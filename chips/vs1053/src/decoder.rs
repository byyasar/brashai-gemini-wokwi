//! VS1053b Dahili Ses Kod Çözücü:
//! - ID3v2 etiketi otomatik atlama
//! - MP3 (MPEG-1/2/2.5 Layer I/II/III) frame decode (nanomp3-core)
//! - WAV (RIFF PCM 8/16-bit mono/stereo) decode
//! - VS1053 Donanım Sine Testi (SM_TESTS: 53 EF 6E <idx> 00 00 00 00 / 45 78 69 74)

use nanomp3_core::{DecodeError, Decoder, MAX_SAMPLES_PER_FRAME};

use crate::audio::PcmRing;
use crate::core::{Vs1053Regs, SM_TESTS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFormat {
    Idle,
    Mp3,
    Wav,
    SineTest,
}

impl StreamFormat {
    pub fn label(self) -> &'static str {
        match self {
            StreamFormat::Idle => "IDLE",
            StreamFormat::Mp3 => "MP3",
            StreamFormat::Wav => "WAV",
            StreamFormat::SineTest => "TEST",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct WavHeaderInfo {
    sample_rate: u32,
    channels: u8,
    bits_per_sample: u16,
    data_remaining: u32,
}

pub struct AudioDecoder {
    mp3: Decoder,
    stream_buf: Vec<u8>,
    pcm_scratch: Vec<i16>,
    id3_skip_remaining: usize,
    wav: Option<WavHeaderInfo>,
    sine_freq_hz: Option<f32>,
    sine_phase: f32,
    idle_ticks: u8,
    pub format: StreamFormat,
    pub sample_rate: u32,
    pub channels: u8,
    pub bitrate_kbps: u32,
    pub decoded_samples_total: u64,
}

impl AudioDecoder {
    pub fn new() -> Self {
        Self {
            mp3: Decoder::new(),
            stream_buf: Vec::with_capacity(16384),
            pcm_scratch: vec![0i16; MAX_SAMPLES_PER_FRAME],
            id3_skip_remaining: 0,
            wav: None,
            sine_freq_hz: None,
            sine_phase: 0.0,
            idle_ticks: 0,
            format: StreamFormat::Idle,
            sample_rate: 44100,
            channels: 2,
            bitrate_kbps: 0,
            decoded_samples_total: 0,
        }
    }

    pub fn reset(&mut self) {
        self.mp3 = Decoder::new();
        self.stream_buf.clear();
        self.id3_skip_remaining = 0;
        self.wav = None;
        self.sine_freq_hz = None;
        self.sine_phase = 0.0;
        self.idle_ticks = 0;
        self.format = StreamFormat::Idle;
        self.sample_rate = 44100;
        self.channels = 2;
        self.bitrate_kbps = 0;
        self.decoded_samples_total = 0;
    }

    /// FIFO'dan veri çekip MP3/WAV/SineTest karelerini çözer ve `pcm_ring` içine yazar.
    pub fn step(&mut self, regs: &mut Vs1053Regs, ring: &mut PcmRing) {
        if regs.reset_pending {
            regs.reset_pending = false;
            self.reset();
            ring.clear();
        }

        let flushing_cancel = regs.cancel_pending;

        // SDI FIFO'dan akış tamponuna baytları aktar
        let mut pulled = false;
        let max_stream = if flushing_cancel { 32768 } else { 16384 };
        while ring.free_space() >= MAX_SAMPLES_PER_FRAME && self.stream_buf.len() < max_stream {
            if let Some(b) = regs.sdi_fifo.pop_front() {
                self.stream_buf.push(b);
                pulled = true;
            } else {
                break;
            }
        }

        if flushing_cancel {
            self.idle_ticks = 10;
        } else if pulled || !regs.sdi_fifo.is_empty() {
            self.idle_ticks = 0;
        } else {
            self.idle_ticks = self.idle_ticks.saturating_add(1);
        }

        // 1. Sine test komutu kontrolü (SM_TESTS aktifken)
        if (regs.mode & SM_TESTS) != 0 {
            self.check_sine_test_cmds(regs);
        } else if self.sine_freq_hz.is_some() {
            self.sine_freq_hz = None;
            self.format = StreamFormat::Idle;
        }

        if let Some(freq) = self.sine_freq_hz {
            if flushing_cancel {
                regs.cancel_pending = false;
                self.sine_freq_hz = None;
                self.format = StreamFormat::Idle;
                regs.sdi_fifo.clear();
                self.stream_buf.clear();
                return;
            }
            self.generate_sine_samples(freq, ring);
            return;
        }

        // 2. ID3v2 atlama devam ediyorsa kalan baytları düş
        if self.id3_skip_remaining > 0 {
            let drop_n = self.id3_skip_remaining.min(self.stream_buf.len());
            self.stream_buf.drain(..drop_n);
            self.id3_skip_remaining -= drop_n;
            if self.id3_skip_remaining > 0 {
                if flushing_cancel {
                    regs.cancel_pending = false;
                    regs.sdi_fifo.clear();
                    self.stream_buf.clear();
                    self.id3_skip_remaining = 0;
                }
                return;
            }
        }

        // 3. ID3v2 başlığı algılama ("ID3" + 2 byte sürüm + 1 byte flag + 4 byte syncsafe boyut)
        if self.wav.is_none() && self.stream_buf.len() >= 10 && &self.stream_buf[0..3] == b"ID3" {
            let tag_size = (((self.stream_buf[6] as usize) & 0x7F) << 21)
                | (((self.stream_buf[7] as usize) & 0x7F) << 14)
                | (((self.stream_buf[8] as usize) & 0x7F) << 7)
                | ((self.stream_buf[9] as usize) & 0x7F);
            let total_skip = 10 + tag_size;
            let drop_n = total_skip.min(self.stream_buf.len());
            self.stream_buf.drain(..drop_n);
            self.id3_skip_remaining = total_skip - drop_n;
            if self.id3_skip_remaining > 0 {
                return;
            }
        }

        // 4. WAV (RIFF....WAVE) başlığı algılama
        if self.wav.is_none()
            && self.stream_buf.len() >= 44
            && &self.stream_buf[0..4] == b"RIFF"
            && &self.stream_buf[8..12] == b"WAVE"
        {
            if let Some((hdr, header_len)) = parse_wav_header(&self.stream_buf) {
                self.stream_buf.drain(..header_len);
                self.wav = Some(hdr);
                self.format = StreamFormat::Wav;
                self.sample_rate = hdr.sample_rate;
                self.channels = hdr.channels;
                self.bitrate_kbps = (hdr.sample_rate * hdr.channels as u32 * hdr.bits_per_sample as u32) / 1000;
                regs.audata = ((hdr.sample_rate as u16) & 0xFFFE) | if hdr.channels > 1 { 1 } else { 0 };
                regs.hdat1 = 0x7665;
                regs.hdat0 = hdr.bits_per_sample;
                ring.set_source_rate(hdr.sample_rate);
            }
        }

        // 5. WAV PCM çözme
        if let Some(mut wav_info) = self.wav {
            self.decode_wav_chunk(&mut wav_info, regs, ring);
            if wav_info.data_remaining == 0 || flushing_cancel {
                self.wav = None;
                if flushing_cancel {
                    regs.cancel_pending = false;
                    regs.sdi_fifo.clear();
                    self.stream_buf.clear();
                }
            } else {
                self.wav = Some(wav_info);
            }
            return;
        }

        // 6. MP3 frame çözme (nanomp3-core)
        self.decode_mp3_frames(regs, ring, flushing_cancel);

        if flushing_cancel {
            regs.cancel_pending = false;
            regs.sdi_fifo.clear();
            self.stream_buf.clear();
            self.mp3 = Decoder::new();
            self.idle_ticks = 0;
        }
    }

    fn check_sine_test_cmds(&mut self, regs: &mut Vs1053Regs) {
        while self.stream_buf.len() >= 8 {
            if self.stream_buf[0..3] == [0x53, 0xEF, 0x6E] {
                let idx = self.stream_buf[3];
                self.stream_buf.drain(..8);
                let f = sine_freq_from_index(idx);
                self.sine_freq_hz = Some(f);
                self.format = StreamFormat::SineTest;
                self.sample_rate = 44100;
                self.channels = 2;
                regs.audata = 44101;
                return;
            } else if self.stream_buf[0..4] == [0x45, 0x78, 0x69, 0x74] {
                self.stream_buf.drain(..8);
                self.sine_freq_hz = None;
                self.format = StreamFormat::Idle;
                return;
            } else {
                self.stream_buf.remove(0);
            }
        }
    }

    fn generate_sine_samples(&mut self, freq_hz: f32, ring: &mut PcmRing) {
        let sr = 44100.0_f32;
        ring.set_source_rate(44100);
        let step = 2.0 * core::f32::consts::PI * freq_hz / sr;
        let to_gen = ring.free_space().min(1024);
        for _ in 0..to_gen {
            let s = (self.sine_phase.sin() * 16000.0) as i16;
            self.sine_phase += step;
            if self.sine_phase >= 2.0 * core::f32::consts::PI {
                self.sine_phase -= 2.0 * core::f32::consts::PI;
            }
            ring.push_sample(s, s);
        }
    }

    fn decode_wav_chunk(&mut self, wav: &mut WavHeaderInfo, regs: &mut Vs1053Regs, ring: &mut PcmRing) {
        let bytes_per_sample = (wav.bits_per_sample / 8).max(1) as usize;
        let frame_bytes = bytes_per_sample * wav.channels.max(1) as usize;
        while self.stream_buf.len() >= frame_bytes && ring.free_space() > 0 && wav.data_remaining >= frame_bytes as u32 {
            let (l, r) = if wav.bits_per_sample == 16 {
                let l = i16::from_le_bytes([self.stream_buf[0], self.stream_buf[1]]);
                let r = if wav.channels > 1 {
                    i16::from_le_bytes([self.stream_buf[2], self.stream_buf[3]])
                } else {
                    l
                };
                (l, r)
            } else {
                let l = ((self.stream_buf[0] as i16) - 128) * 256;
                let r = if wav.channels > 1 {
                    ((self.stream_buf[1] as i16) - 128) * 256
                } else {
                    l
                };
                (l, r)
            };
            self.stream_buf.drain(..frame_bytes);
            wav.data_remaining -= frame_bytes as u32;
            ring.push_sample(l, r);
            self.decoded_samples_total += 1;
            regs.decode_time = (self.decoded_samples_total / wav.sample_rate.max(1) as u64) as u16;
        }
    }

    fn decode_mp3_frames(&mut self, regs: &mut Vs1053Regs, ring: &mut PcmRing, flush_all: bool) {
        let min_buf = if flush_all || self.idle_ticks >= 4 { 64 } else { 1536 };
        // Her 20 ms'lik tikte en fazla 3 MP3 karesi (~72 ms ses) çöz; simülasyonu asla dondurma
        let max_iters = if flush_all { 64 } else { 3 };
        let mut guard = 0;

        while ring.free_space() >= MAX_SAMPLES_PER_FRAME && self.stream_buf.len() >= min_buf && guard < max_iters {
            guard += 1;

            let (consumed, res) = self.mp3.decode(&self.stream_buf, &mut self.pcm_scratch);

            match res {
                Ok(info) => {
                    if consumed > 0 {
                        let c = consumed.min(self.stream_buf.len());
                        self.stream_buf.drain(..c);
                    }
                    if info.samples_produced > 0 {
                        let ch = info.channels.num();
                        let sr = info.sample_rate.max(8000);

                        // Dosya sonu filler (0x00) kaynaklı sahte MP3 başlıklarını (false sync) filtrele:
                        // Akış zaten MP3 olarak kilitlendiyse farklı sample_rate veya kanal sayısına sahip sahte kareyi yoksay.
                        if self.format == StreamFormat::Mp3 && (sr != self.sample_rate || ch != self.channels) {
                            continue;
                        }

                        self.format = StreamFormat::Mp3;
                        self.sample_rate = sr;
                        self.channels = ch;
                        self.bitrate_kbps = info.bitrate;
                        ring.set_source_rate(self.sample_rate);

                        regs.audata = ((self.sample_rate as u16) & 0xFFFE) | if ch > 1 { 1 } else { 0 };
                        regs.hdat1 = 0xFFE0 | ((info.layer as u16) << 1);
                        regs.hdat0 = (info.bitrate as u16) & 0xFFFF;

                        for i in 0..info.samples_produced {
                            let (l, r) = if ch == 1 {
                                let s = self.pcm_scratch[i];
                                (s, s)
                            } else {
                                (self.pcm_scratch[i * 2], self.pcm_scratch[i * 2 + 1])
                            };
                            ring.push_sample(l, r);
                        }
                        self.decoded_samples_total += info.samples_produced as u64;
                        regs.decode_time = (self.decoded_samples_total / self.sample_rate as u64) as u16;
                    } else if consumed == 0 {
                        break;
                    }
                }
                Err(DecodeError::NoFrame) => {
                    if (flush_all || self.idle_ticks >= 4) && self.stream_buf.len() < 1024 {
                        self.stream_buf.clear();
                        break;
                    }
                    let c = consumed.min(self.stream_buf.len()).max(1);
                    self.stream_buf.drain(..c);
                }
                Err(DecodeError::ReservoirUnavailable(_))
                | Err(DecodeError::Corrupt(_))
                | Err(DecodeError::UnsupportedLayer(_))
                | Err(_) => {
                    let c = consumed.min(self.stream_buf.len()).max(1);
                    self.stream_buf.drain(..c);
                }
            }
        }
    }
}

/// VS1053 datasheet Bölüm 9.12.1 sine test frekans formülü.
fn sine_freq_from_index(idx: u8) -> f32 {
    let fs_idx = ((idx >> 5) & 0x07) as usize;
    let skip = (idx & 0x1F) as f32;
    const FS_TABLE: [f32; 8] = [44100.0, 48000.0, 32000.0, 22050.0, 24000.0, 16000.0, 11025.0, 12000.0];
    let fs = FS_TABLE[fs_idx];
    let f = fs * skip / 128.0;
    if f < 80.0 { 1000.0 } else { f }
}

fn parse_wav_header(buf: &[u8]) -> Option<(WavHeaderInfo, usize)> {
    if buf.len() < 44 || &buf[0..4] != b"RIFF" || &buf[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12usize;
    let mut channels = 1u8;
    let mut sample_rate = 22050u32;
    let mut bits_per_sample = 16u16;
    let mut fmt_found = false;

    while pos + 8 <= buf.len() {
        let chunk_id = &buf[pos..pos + 4];
        let chunk_sz = u32::from_le_bytes([buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]]) as usize;
        pos += 8;

        if chunk_id == b"fmt " {
            if pos + 16 > buf.len() {
                return None;
            }
            let audio_fmt = u16::from_le_bytes([buf[pos], buf[pos + 1]]);
            if audio_fmt != 1 {
                return None;
            }
            channels = u16::from_le_bytes([buf[pos + 2], buf[pos + 3]]).clamp(1, 2) as u8;
            sample_rate = u32::from_le_bytes([buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]]).clamp(8000, 48000);
            bits_per_sample = u16::from_le_bytes([buf[pos + 14], buf[pos + 15]]);
            if bits_per_sample != 8 && bits_per_sample != 16 {
                bits_per_sample = 16;
            }
            fmt_found = true;
        } else if chunk_id == b"data" && fmt_found {
            let info = WavHeaderInfo {
                sample_rate,
                channels,
                bits_per_sample,
                data_remaining: chunk_sz as u32,
            };
            return Some((info, pos));
        }

        pos = pos.saturating_add(chunk_sz + (chunk_sz & 1));
    }
    None
}
