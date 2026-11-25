//! VLSI VS1053b Audio Codec Geliştirme Kartı (chip-vs1053)
//! Wokwi Custom Chips API (WASM) giriş noktası, 1-bayt kayıpsız SPI SCI/SDI köprüsü,
//! nanosaniye hassasiyetli hoparlör kare dalga sentezleyici ve 160x96 PCB durum ekranı.

pub mod audio;
pub mod core;
pub mod decoder;

use std::ffi::c_void;

use wokwi_api::*;

use crate::audio::{PcmRing, SigmaDeltaAudio, AUDIO_FRAME_MICROS};
use crate::core::Vs1053Regs;
use crate::decoder::AudioDecoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpiPhase {
    Idle,
    SciHeader,
    SciReadData,
    SciWriteData,
    SdiStream,
}

struct Vs1053Chip {
    regs: Vs1053Regs,
    decoder: AudioDecoder,
    ring: PcmRing,
    sd_audio: SigmaDeltaAudio,

    // Pinler
    vcc: PinT,
    sck: PinT,
    miso: PinT,
    _xcs: PinT,
    xdcs: PinT,
    dreq: PinT,
    xrst: PinT,
    left: PinT,
    right: PinT,
    spk_out: PinT,

    // Özellikler, SPI ve Zamanlayıcılar
    attr_gain: AttrT,
    spi: SpiDevT,
    tone_timer: TimerT,
    spi_phase: SpiPhase,
    sci_cmd_addr: u8,
    sci_buf: [u8; 4],
    sdi_byte: [u8; 1],
    sdi_chunk_cnt: u16,

    // SCI Read (0x03) sırasında MISO bit-shift durumu (paylaşılan SPI'da MISO çakışmasını %100 önler)
    sci_read_val: u16,
    sci_read_bit: u8,
    sci_skip_first_fall: bool,
    sck_watching: bool,

    // Ses & Durum
    tone_running: bool,
    active_tone_hz: f32,
    last_dreq: u32,
    dreq_low_ticks: u8,
    last_spk: u32,
}

static mut CHIP: Option<Vs1053Chip> = None;

impl Vs1053Chip {
    fn release_miso(&mut self) {
        if self.sck_watching {
            pin_watch_stop(self.sck);
            self.sck_watching = false;
        }
        pin_mode(self.miso, INPUT);
    }

    fn stop_tone(&mut self) {
        if self.tone_running {
            timer_stop(self.tone_timer);
            self.tone_running = false;
            self.active_tone_hz = 0.0;
        }
        if self.last_spk != LOW {
            self.last_spk = LOW;
            pin_write(self.spk_out, LOW);
        }
    }

    fn update_tone(&mut self, target_hz: Option<f32>) {
        match target_hz {
            Some(freq) if (60.0..=4000.0).contains(&freq) => {
                let need_update = !self.tone_running
                    || self.active_tone_hz <= 1.0
                    || ((freq - self.active_tone_hz).abs() / self.active_tone_hz) > 0.012;
                if need_update {
                    let half_period_ns = ((500_000_000.0_f64 / freq as f64).round() as u64)
                        .clamp(125_000, 8_500_000);
                    timer_start_ns(self.tone_timer, half_period_ns, true);
                    self.tone_running = true;
                    self.active_tone_hz = freq;
                }
            }
            _ => {
                self.stop_tone();
            }
        }
    }

    fn start_sci_read_shift(&mut self, val: u16) {
        self.sci_read_val = val;
        self.sci_read_bit = 15;
        self.sci_skip_first_fall = pin_read(self.sck) == HIGH;
        pin_mode(self.miso, OUTPUT);
        pin_write(self.miso, if (val & 0x8000) != 0 { HIGH } else { LOW });

        let watch = PinWatchConfig {
            user_data: std::ptr::null_mut(),
            edge: FALLING,
            pin_change: Some(on_sci_read_sck_falling),
        };
        pin_watch(self.sck, &watch);
        self.sck_watching = true;
    }

    fn sync_dreq(&mut self) {
        let ready = if pin_read(self.xrst) == LOW {
            LOW
        } else if self.regs.is_dreq_ready_for_stream(self.ring.len()) {
            HIGH
        } else {
            LOW
        };
        if ready == HIGH {
            self.dreq_low_ticks = 0;
        }
        if ready != self.last_dreq {
            self.last_dreq = ready;
            pin_write(self.dreq, ready);
        }
    }
}

extern "C" fn on_sci_read_sck_falling(_user_data: *mut c_void, _pin: PinT, _value: u32) {
    unsafe {
        let Some(chip) = CHIP.as_mut() else { return };
        if chip.spi_phase != SpiPhase::SciReadData {
            chip.release_miso();
            return;
        }
        if chip.sci_skip_first_fall {
            chip.sci_skip_first_fall = false;
            return;
        }
        if chip.sci_read_bit == 0 {
            chip.spi_phase = SpiPhase::Idle;
            chip.release_miso();
        } else {
            chip.sci_read_bit -= 1;
            let bit = if ((chip.sci_read_val >> chip.sci_read_bit) & 1) != 0 { HIGH } else { LOW };
            pin_write(chip.miso, bit);
        }
    }
}

extern "C" fn on_xcs_change(_user_data: *mut c_void, _pin: PinT, value: u32) {
    unsafe {
        let Some(chip) = CHIP.as_mut() else { return };
        if value == LOW {
            chip.release_miso();
            chip.spi_phase = SpiPhase::SciHeader;
            chip.sci_buf = [0; 4];
            spi_start_len(chip.spi, chip.sci_buf.as_mut_ptr(), 2);
        } else {
            if chip.spi_phase != SpiPhase::Idle {
                spi_stop(chip.spi);
                chip.spi_phase = SpiPhase::Idle;
            }
            chip.release_miso();
            if chip.regs.reset_pending {
                chip.decoder.step(&mut chip.regs, &mut chip.ring);
                if chip.ring.is_empty() {
                    chip.stop_tone();
                }
            } else if chip.regs.cancel_pending {
                chip.ring.clear();
                chip.stop_tone();
                chip.regs.cancel_pending = false;
            }
            chip.sync_dreq();
        }
    }
}

extern "C" fn on_xdcs_change(_user_data: *mut c_void, _pin: PinT, value: u32) {
    unsafe {
        let Some(chip) = CHIP.as_mut() else { return };
        if value == LOW {
            // 1-baytlık kayıpsız SDI akışı: 1..32+ bayt arası her paket eksiksiz yakalanır
            chip.release_miso();
            chip.spi_phase = SpiPhase::SdiStream;
            chip.sdi_byte = [0; 1];
            chip.sdi_chunk_cnt = 0;
            spi_start_len(chip.spi, chip.sdi_byte.as_mut_ptr(), 1);
        } else {
            if chip.spi_phase == SpiPhase::SdiStream {
                spi_stop(chip.spi);
                chip.spi_phase = SpiPhase::Idle;
            }
            chip.release_miso();
            chip.sync_dreq();
        }
    }
}

extern "C" fn on_xrst_change(_user_data: *mut c_void, _pin: PinT, value: u32) {
    unsafe {
        let Some(chip) = CHIP.as_mut() else { return };
        if value == LOW {
            chip.regs.hard_reset();
            chip.decoder.reset();
            chip.ring.clear();
            chip.sd_audio.reset();
            chip.stop_tone();
        } else {
            chip.regs.reset_pending = false;
        }
        chip.sync_dreq();
    }
}

extern "C" fn on_spi_done(_user_data: *mut c_void, _buffer: *mut u8, count: u32) {
    unsafe {
        let Some(chip) = CHIP.as_mut() else { return };
        let n = count as usize;

        match chip.spi_phase {
            SpiPhase::SciHeader => {
                if n >= 2 {
                    let opcode = chip.sci_buf[0];
                    let addr = chip.sci_buf[1];
                    chip.sci_cmd_addr = addr;
                    if opcode == 0x03 {
                        let val = chip.regs.read_reg(addr);
                        chip.spi_phase = SpiPhase::SciReadData;
                        chip.start_sci_read_shift(val);
                    } else if opcode == 0x02 {
                        chip.sci_buf = [0; 4];
                        chip.spi_phase = SpiPhase::SciWriteData;
                        spi_start_len(chip.spi, chip.sci_buf.as_mut_ptr(), 2);
                    } else {
                        chip.spi_phase = SpiPhase::Idle;
                        chip.release_miso();
                    }
                }
            }
            SpiPhase::SciReadData => {
                chip.release_miso();
                chip.spi_phase = SpiPhase::Idle;
                chip.sync_dreq();
            }
            SpiPhase::SciWriteData => {
                if n >= 2 {
                    let val = ((chip.sci_buf[0] as u16) << 8) | (chip.sci_buf[1] as u16);
                    chip.regs.write_reg(chip.sci_cmd_addr, val);
                }
                chip.spi_phase = SpiPhase::Idle;
                chip.sync_dreq();
            }
            SpiPhase::SdiStream => {
                if n > 0 {
                    // SPI callback'i içinde ağır MP3 decode YAPMA (ESP32 TG1WDT koruma):
                    // Yalnızca baytı FIFO'ya koy; decode 20 ms timer'da küçük adımlarla yapılır.
                    let b = chip.sdi_byte[0];
                    chip.regs.push_sdi(&[b]);
                    chip.sync_dreq();
                }
                if pin_read(chip.xdcs) == LOW {
                    chip.sdi_byte = [0; 1];
                    spi_start_len(chip.spi, chip.sdi_byte.as_mut_ptr(), 1);
                } else {
                    chip.spi_phase = SpiPhase::Idle;
                }
            }
            SpiPhase::Idle => {}
        }
    }
}

/// Nanosaniye hassasiyetli simetrik kare dalga üreteci (`wokwi-buzzer` WebAudio sürücüsü)
extern "C" fn on_tone_timer(_user_data: *mut c_void) {
    unsafe {
        let Some(chip) = CHIP.as_mut() else { return };
        if pin_read(chip.xrst) == LOW || !chip.tone_running {
            if chip.last_spk != LOW {
                chip.last_spk = LOW;
                pin_write(chip.spk_out, LOW);
            }
            return;
        }
        let next = if chip.last_spk == LOW { HIGH } else { LOW };
        chip.last_spk = next;
        chip.sd_audio.pdm_transitions = chip.sd_audio.pdm_transitions.wrapping_add(1);
        pin_write(chip.spk_out, next);
    }
}

/// 100 Hz (10 ms) gerçek zamanlı MP3 çözme, otokorelasyon frekans takibi, DAC zarf ve 25 Hz ekran zamanlayıcısı
extern "C" fn on_frame_timer(_user_data: *mut c_void) {
    unsafe {
        let Some(chip) = CHIP.as_mut() else { return };
        if chip.regs.reset_busy_ticks > 0 {
            chip.regs.reset_busy_ticks -= 1;
        }

        if pin_read(chip.xrst) != LOW {
            chip.decoder.step(&mut chip.regs, &mut chip.ring);

            let (gl, gr) = chip.regs.volume_gains();
            let gain_raw = attr_read(chip.attr_gain);
            let gain_pct = if gain_raw == 0 { 100.0 } else { gain_raw as f32 };
            let target_freq = chip.sd_audio.process_frame_10ms(
                &mut chip.ring,
                gl,
                gr,
                chip.regs.bass,
                gain_pct,
            );
            chip.update_tone(target_freq);

            // ESP32 watchdog koruması: DREQ 60 ms'den uzun LOW kalırsa tamponda yer aç
            if chip.last_dreq == LOW {
                chip.dreq_low_ticks = chip.dreq_low_ticks.saturating_add(1);
                if chip.dreq_low_ticks >= 3 {
                    chip.ring.drop_oldest(16384);
                    chip.dreq_low_ticks = 0;
                }
            }
        } else {
            chip.stop_tone();
        }

        chip.sync_dreq();

        // Analog çıkış zarf voltajlarını güncelle
        pin_dac_write(chip.left, chip.sd_audio.left_dac_voltage());
        pin_dac_write(chip.right, chip.sd_audio.right_dac_voltage());
    }
}

#[no_mangle]
pub extern "C" fn chip_init() {
    chipInit();
}

#[no_mangle]
pub extern "C" fn chipInit() {
    let vcc = pin_init(b"VCC\0", INPUT);
    let _gnd = pin_init(b"GND\0", INPUT);
    let sck = pin_init(b"SCK\0", INPUT);
    let mosi = pin_init(b"MOSI\0", INPUT);
    let miso = pin_init(b"MISO\0", INPUT);
    let xcs = pin_init(b"XCS\0", INPUT_PULLUP);
    let xdcs = pin_init(b"XDCS\0", INPUT_PULLUP);
    let dreq = pin_init(b"DREQ\0", OUTPUT_HIGH);
    let xrst = pin_init(b"XRST\0", INPUT_PULLUP);
    let left = pin_init(b"LEFT\0", ANALOG);
    let right = pin_init(b"RIGHT\0", ANALOG);
    let spk_out = pin_init(b"SPK_OUT\0", OUTPUT_LOW);
    let _gbuf = pin_init(b"GBUF\0", OUTPUT_LOW);
    let _gpio0 = pin_init(b"GPIO0\0", INPUT);
    let _gpio1 = pin_init(b"GPIO1\0", INPUT);
    let _gpio2 = pin_init(b"GPIO2\0", INPUT);
    let _gpio3 = pin_init(b"GPIO3\0", INPUT);

    let attr_gain = attr_init(b"gain\0", 100);

    // Paylaşılan SPI veriyolunda MISO çakışmasını önlemek için miso = NO_PIN
    let spi_cfg = SpiConfig {
        user_data: std::ptr::null_mut(),
        sck,
        mosi,
        miso: NO_PIN,
        mode: 0,
        done: Some(on_spi_done),
        reserved: [0; 8],
    };
    let spi = spi_init(&spi_cfg);

    let tone_cfg = TimerConfig {
        user_data: std::ptr::null_mut(),
        callback: Some(on_tone_timer),
        reserved: [0; 8],
    };
    let tone_timer = timer_init(&tone_cfg);

    unsafe {
        CHIP = Some(Vs1053Chip {
            regs: Vs1053Regs::new(),
            decoder: AudioDecoder::new(),
            ring: PcmRing::new(),
            sd_audio: SigmaDeltaAudio::new(),
            vcc,
            sck,
            miso,
            _xcs: xcs,
            xdcs,
            dreq,
            xrst,
            left,
            right,
            spk_out,
            attr_gain,
            spi,
            tone_timer,
            spi_phase: SpiPhase::Idle,
            sci_cmd_addr: 0,
            sci_buf: [0; 4],
            sdi_byte: [0; 1],
            sdi_chunk_cnt: 0,
            sci_read_val: 0,
            sci_read_bit: 0,
            sci_skip_first_fall: false,
            sck_watching: false,
            tone_running: false,
            active_tone_hz: 0.0,
            last_dreq: HIGH,
            dreq_low_ticks: 0,
            last_spk: LOW,
        });
    }

    let xcs_watch = PinWatchConfig {
        user_data: std::ptr::null_mut(),
        edge: BOTH,
        pin_change: Some(on_xcs_change),
    };
    pin_watch(xcs, &xcs_watch);

    let xdcs_watch = PinWatchConfig {
        user_data: std::ptr::null_mut(),
        edge: BOTH,
        pin_change: Some(on_xdcs_change),
    };
    pin_watch(xdcs, &xdcs_watch);

    let xrst_watch = PinWatchConfig {
        user_data: std::ptr::null_mut(),
        edge: BOTH,
        pin_change: Some(on_xrst_change),
    };
    pin_watch(xrst, &xrst_watch);

    // 100 Hz (10 ms) gerçek zamanlı decode, DSP pitch ve ekran zamanlayıcısı
    let frame_cfg = TimerConfig {
        user_data: std::ptr::null_mut(),
        callback: Some(on_frame_timer),
        reserved: [0; 8],
    };
    let frame_timer = timer_init(&frame_cfg);
    timer_start(frame_timer, AUDIO_FRAME_MICROS, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::*;
    use crate::decoder::StreamFormat;

    #[test]
    fn test_sci_reset_values_and_status_version() {
        let mut regs = Vs1053Regs::new();
        assert_eq!(regs.read_reg(SCI_MODE), SM_SDINEW);
        let status = regs.read_reg(SCI_STATUS);
        assert_eq!((status >> 4) & 0x0F, 4, "SS_VER mutlaka 4 olmali");
        assert!(regs.is_dreq_ready());
    }

    #[test]
    fn test_esp32_vs1053_ext_begin_sequence() {
        let mut regs = Vs1053Regs::new();
        let mut dec = AudioDecoder::new();
        let mut ring = PcmRing::new();

        regs.write_reg(SCI_WRAMADDR, 0xC017);
        regs.write_reg(SCI_WRAM, 3);
        regs.write_reg(SCI_WRAMADDR, 0xC019);
        regs.write_reg(SCI_WRAM, 0);
        assert_eq!(regs.gpio_ddr, 3);
        assert_eq!(regs.gpio_odata, 0);

        regs.write_reg(SCI_MODE, SM_SDINEW | SM_RESET);
        assert!(regs.is_dreq_ready());

        regs.write_reg(SCI_CLOCKF, 6 << 12);
        regs.write_reg(SCI_MODE, SM_SDINEW | SM_LINE1);
        regs.write_reg(SCI_VOL, 0x1010);
        assert_eq!(regs.read_reg(SCI_CLOCKF), 0x6000);
        assert_eq!(regs.read_reg(SCI_MODE), SM_SDINEW | SM_LINE1);

        regs.write_reg(SCI_WRAMADDR, 0x1E06);
        assert_eq!(regs.read_reg(SCI_WRAM), 0x0000);

        regs.push_sdi(&[0xAA; 64]);
        regs.write_reg(SCI_MODE, SM_SDINEW | SM_CANCEL);
        assert_eq!(regs.read_reg(SCI_MODE) & SM_CANCEL, 0, "SM_CANCEL hemen temizlenmeli");
        dec.step(&mut regs, &mut ring);
        assert_eq!(regs.sdi_fifo.len(), 0);
    }

    #[test]
    fn test_sci_vol_and_decode_time_double_write() {
        let mut regs = Vs1053Regs::new();
        regs.write_reg(SCI_VOL, 0x0000);
        let (l, r) = regs.volume_gains();
        assert!((l - 1.0).abs() < 1e-4 && (r - 1.0).abs() < 1e-4);

        regs.write_reg(SCI_VOL, 0xFEFE);
        let (l, r) = regs.volume_gains();
        assert_eq!(l, 0.0);
        assert_eq!(r, 0.0);

        regs.decode_time = 42;
        regs.write_reg(SCI_DECODE_TIME, 10);
        assert_eq!(regs.read_reg(SCI_DECODE_TIME), 10);
        regs.write_reg(SCI_DECODE_TIME, 1);
        regs.write_reg(SCI_DECODE_TIME, 1);
        assert_eq!(regs.read_reg(SCI_DECODE_TIME), 0);
    }

    #[test]
    fn test_real_48khz_firca1_mp3_with_98kb_id3_tag() {
        // Kullanıcının gerçek 4.96 MB, 48 kHz, 256 kbps stereo MP3 dosyası (98 KB ID3v2 etiketli)
        let mp3_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdcard/hosgeldiniz/Firca1.mp3");
        let full_data = std::fs::read(mp3_path).expect("Firca1.mp3 okunamadi");
        // İlk 260 KB (~98 KB ID3v2 + ~5 saniye 256 kbps müzik) test et
        let slice_len = full_data.len().min(260_000);
        let data = &full_data[..slice_len];

        let mut regs = Vs1053Regs::new();
        let mut dec = AudioDecoder::new();
        let mut ring = PcmRing::new();
        let mut audio = SigmaDeltaAudio::new();

        let mut active_frames = 0usize;
        for chunk in data.chunks(32) {
            regs.push_sdi(chunk);
            if regs.sdi_fifo.len() >= 64 {
                dec.step(&mut regs, &mut ring);
            }
            while ring.len() >= 24_000 {
                if audio.process_frame_10ms(&mut ring, 1.0, 1.0, 0, 100.0).is_some() {
                    active_frames += 1;
                }
            }
        }
        dec.step(&mut regs, &mut ring);
        while !ring.is_empty() {
            if audio.process_frame_10ms(&mut ring, 1.0, 1.0, 0, 100.0).is_some() {
                active_frames += 1;
            }
        }

        assert_eq!(dec.format, StreamFormat::Mp3);
        assert_eq!(dec.sample_rate, 48000);
        assert_eq!(dec.channels, 2);
        assert!(dec.decoded_samples_total > 100_000, "En az 2 sn 48kHz PCM cozulmeli: {}", dec.decoded_samples_total);
        assert!(active_frames > 50, "Gercek MP3 sarkisindan aktif ses cerceveleri cikmali: {}", active_frames);
    }

    #[test]
    fn test_firca2_burst_plus_cancel_preserves_all_arpeggio_notes() {
        // ESP32-vs1053_ext'in gerçek davranışı: 17 baytlık parçalar + 2052 filler + SM_CANCEL
        let mp3_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdcard/hosgeldiniz/Firca2.mp3");
        let data = std::fs::read(mp3_path).expect("Firca2.mp3 okunamadi");

        let mut regs = Vs1053Regs::new();
        let mut dec = AudioDecoder::new();
        let mut ring = PcmRing::new();
        let mut audio = SigmaDeltaAudio::new();

        for chunk in data.chunks(17) {
            regs.push_sdi(chunk);
            if regs.sdi_fifo.len() >= 64 {
                dec.step(&mut regs, &mut ring);
            }
        }
        for chunk in [0u8; 2052].chunks(32) {
            regs.push_sdi(chunk);
            dec.step(&mut regs, &mut ring);
        }
        regs.write_reg(SCI_MODE, SM_SDINEW | SM_CANCEL);
        dec.step(&mut regs, &mut ring);

        assert_eq!(dec.format, StreamFormat::Mp3);
        assert_eq!(dec.sample_rate, 22050);
        assert!(ring.len() > 30_000, "1.7 sn sesin tamami korunmali, bulunan: {}", ring.len());

        // Firca2 Re majör arpeji: Re5 (~587.33 Hz), Fa#5 (~739.99 Hz), La5 (~880.0 Hz)
        let mut saw_d5 = false;
        let mut saw_fs5 = false;
        let mut saw_a5 = false;
        while !ring.is_empty() {
            if let Some(f) = audio.process_frame_10ms(&mut ring, 1.0, 1.0, 0, 100.0) {
                if (f - 587.33).abs() < 25.0 {
                    saw_d5 = true;
                }
                if (f - 739.99).abs() < 25.0 {
                    saw_fs5 = true;
                }
                if (f - 880.00).abs() < 30.0 {
                    saw_a5 = true;
                }
            }
        }
        assert!(saw_d5, "Re5 (587 Hz) notasi duyulmali!");
        assert!(saw_fs5, "Fa#5 (740 Hz) notasi duyulmali!");
        assert!(saw_a5, "La5 (880 Hz) notasi duyulmali!");
    }

    #[test]
    fn test_sarki1_and_tada_mp3_pitch_and_envelope() {
        let mp3_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdcard/efekt/tada.mp3");
        let data = std::fs::read(mp3_path).expect("tada.mp3 okunamadi");

        let mut regs = Vs1053Regs::new();
        let mut dec = AudioDecoder::new();
        let mut ring = PcmRing::new();
        let mut audio = SigmaDeltaAudio::new();

        for chunk in data.chunks(32) {
            regs.push_sdi(chunk);
            dec.step(&mut regs, &mut ring);
        }
        regs.write_reg(SCI_MODE, SM_SDINEW | SM_CANCEL);
        dec.step(&mut regs, &mut ring);

        let mut active_frames = 0usize;
        let mut saw_c6 = false;
        while !ring.is_empty() {
            if let Some(f) = audio.process_frame_10ms(&mut ring, 1.0, 1.0, 0, 100.0) {
                active_frames += 1;
                if (f - 1046.5).abs() < 35.0 {
                    saw_c6 = true;
                }
            }
        }
        assert!(active_frames > 50, "En az 1 sn aktif ses cercevesi uretilmeli: {}", active_frames);
        assert!(saw_c6, "Tada fanfar bitis notasi C6 (1046 Hz) yakalanmali!");
    }

    #[test]
    fn test_sine_test_start_and_exit() {
        let mut regs = Vs1053Regs::new();
        let mut dec = AudioDecoder::new();
        let mut ring = PcmRing::new();

        regs.write_reg(SCI_MODE, SM_SDINEW | SM_TESTS);
        regs.push_sdi(&[0x53, 0xEF, 0x6E, 0x44, 0x00, 0x00, 0x00, 0x00]);
        dec.step(&mut regs, &mut ring);
        assert_eq!(dec.format, StreamFormat::SineTest);
        assert!(ring.len() > 0);

        ring.clear();
        regs.push_sdi(&[0x45, 0x78, 0x69, 0x74, 0x00, 0x00, 0x00, 0x00]);
        dec.step(&mut regs, &mut ring);
        assert_eq!(dec.format, StreamFormat::Idle);
    }
}
