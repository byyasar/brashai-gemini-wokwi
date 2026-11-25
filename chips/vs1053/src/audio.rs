//! VS1053 Ses DSP ve Wokwi Hoparlör Sentez Katmanı:
//! - Geniş Stereo PCM halka tamponu (131.072 örnek ~6 sn @ 22050 Hz, sıfır kare kaybı)
//! - SCI_VOL (0.5 dB/adım L/R zayıflatma) + SCI_BASS raf filtresi + harici kazanç (%)
//! - Gerçek zamanlı 100 Hz (10 ms) çerçeve tüketici + kayan pencereli Otokorelasyon (NSDF)
//!   alt-örnek (parabolik interpolasyonlu) temel frekans / nota çıkarıcı
//! - Wokwi `wokwi-buzzer` WebAudio motoru için nanosaniye hassasiyetli kare dalga periyodu üretimi
//! - Sol / Sağ kanal RMS zarf hesaplayıcı (0.0V .. 3.3V, chip-speaker VU-metre için)

use std::collections::VecDeque;

pub const AUDIO_FRAME_MICROS: u32 = 20_000; // 20 ms (50 Hz DSP çerçeve hızı - düşük WASM yükü)
pub const PDM_SAMPLE_RATE_HZ: u32 = 10_000;
pub const PDM_TIMER_MICROS: u32 = 100;
pub const PCM_RING_CAPACITY: usize = 131_072;
const PITCH_WINDOW: usize = 512;

pub struct PcmRing {
    buf: VecDeque<(i16, i16)>,
    source_rate: u32,
    phase: f32,
    cur_sample: (i16, i16),
    next_sample: (i16, i16),
    has_samples: bool,
    frame_rem: f32,
}

impl PcmRing {
    pub fn new() -> Self {
        Self {
            buf: VecDeque::with_capacity(16384),
            source_rate: 22050,
            phase: 0.0,
            cur_sample: (0, 0),
            next_sample: (0, 0),
            has_samples: false,
            frame_rem: 0.0,
        }
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.phase = 0.0;
        self.cur_sample = (0, 0);
        self.next_sample = (0, 0);
        self.has_samples = false;
        self.frame_rem = 0.0;
    }

    #[inline]
    pub fn set_source_rate(&mut self, sr: u32) {
        self.source_rate = sr.clamp(8000, 48000);
    }

    #[inline]
    pub fn source_rate(&self) -> u32 {
        self.source_rate
    }

    #[inline]
    pub fn free_space(&self) -> usize {
        PCM_RING_CAPACITY.saturating_sub(self.buf.len())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    #[inline]
    pub fn push_sample(&mut self, l: i16, r: i16) {
        if self.buf.len() >= PCM_RING_CAPACITY {
            self.buf.pop_front();
        }
        self.buf.push_back((l, r));
    }

    pub fn drop_oldest(&mut self, n: usize) {
        let k = n.min(self.buf.len());
        for _ in 0..k {
            self.buf.pop_front();
        }
    }

    #[inline]
    pub fn pop_raw(&mut self) -> Option<(i16, i16)> {
        self.buf.pop_front()
    }

    /// Kesirli akümülatör ile tam `dt_sec` sürelik örnek sayısını döndürür.
    pub fn samples_for_duration(&mut self, dt_sec: f32) -> usize {
        let exact = self.source_rate as f32 * dt_sec + self.frame_rem;
        let n = exact.floor() as usize;
        self.frame_rem = exact - (n as f32);
        n
    }

    /// Birim testleri ve örnek bazlı okuma için doğrusal interpolasyonlu okuyucu.
    pub fn next_interpolated(&mut self) -> Option<(f32, f32)> {
        if !self.has_samples {
            let first = self.buf.pop_front()?;
            let second = self.buf.front().copied().unwrap_or(first);
            self.cur_sample = first;
            self.next_sample = second;
            self.phase = 0.0;
            self.has_samples = true;
        }

        let frac = self.phase;
        let l = self.cur_sample.0 as f32 + (self.next_sample.0 - self.cur_sample.0) as f32 * frac;
        let r = self.cur_sample.1 as f32 + (self.next_sample.1 - self.cur_sample.1) as f32 * frac;

        self.phase += self.source_rate as f32 / PDM_SAMPLE_RATE_HZ as f32;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.cur_sample = self.next_sample;
            if let Some(nxt) = self.buf.pop_front() {
                self.next_sample = nxt;
            } else {
                self.has_samples = false;
                break;
            }
        }

        Some((l / 32768.0, r / 32768.0))
    }
}

pub struct SigmaDeltaAudio {
    lp_state: f32,
    hp_prev_in: f32,
    hp_prev_out: f32,
    history: [f32; PITCH_WINDOW],
    hist_filled: usize,
    pub env_l: f32,
    pub env_r: f32,
    pub current_freq_hz: f32,
    pub active: bool,
    pub pdm_transitions: u64,
}

impl SigmaDeltaAudio {
    pub fn new() -> Self {
        Self {
            lp_state: 0.0,
            hp_prev_in: 0.0,
            hp_prev_out: 0.0,
            history: [0.0; PITCH_WINDOW],
            hist_filled: 0,
            env_l: 0.0,
            env_r: 0.0,
            current_freq_hz: 0.0,
            active: false,
            pdm_transitions: 0,
        }
    }

    pub fn reset(&mut self) {
        self.lp_state = 0.0;
        self.hp_prev_in = 0.0;
        self.hp_prev_out = 0.0;
        self.history = [0.0; PITCH_WINDOW];
        self.hist_filled = 0;
        self.env_l = 0.0;
        self.env_r = 0.0;
        self.current_freq_hz = 0.0;
        self.active = false;
    }

    /// Her 10 ms'de gerçek zamanlı PCM örneklerini tüketir, ses zarfını günceller ve
    /// Wokwi hoparlörünün çalacağı baskın müzik frekansını (Hz) döndürür.
    pub fn process_frame_10ms(
        &mut self,
        ring: &mut PcmRing,
        gain_l: f32,
        gain_r: f32,
        bass_reg: u16,
        user_gain_pct: f32,
    ) -> Option<f32> {
        if ring.is_empty() {
            self.active = false;
            self.current_freq_hz = 0.0;
            self.env_l *= 0.75;
            self.env_r *= 0.75;
            if self.env_l < 0.005 {
                self.env_l = 0.0;
            }
            if self.env_r < 0.005 {
                self.env_r = 0.0;
            }
            self.hist_filled = 0;
            return None;
        }

        let user_gain = if user_gain_pct <= 0.0 {
            1.0
        } else {
            (user_gain_pct / 100.0).clamp(0.1, 2.0)
        };

        let sr = ring.source_rate();
        let want = ring.samples_for_duration(0.020).max(1);

        let sb_amp = ((bass_reg >> 4) & 0x0F) as f32;
        let st_amp = ((bass_reg >> 12) & 0x0F) as i8;
        let st_signed = if st_amp >= 8 { st_amp - 16 } else { st_amp } as f32;

        let mut sum_sq_l = 0.0_f32;
        let mut sum_sq_r = 0.0_f32;
        let mut count = 0usize;
        let mut frame_mono = Vec::with_capacity(want);

        for _ in 0..want {
            let Some((il, ir)) = ring.pop_raw() else {
                break;
            };
            let sl = (il as f32 / 32768.0) * gain_l * user_gain;
            let sr_ch = (ir as f32 / 32768.0) * gain_r * user_gain;
            sum_sq_l += sl * sl;
            sum_sq_r += sr_ch * sr_ch;
            count += 1;

            let mut mono = 0.5 * (sl + sr_ch);
            if sb_amp > 0.0 {
                self.lp_state += 0.06 * (mono - self.lp_state);
                mono += self.lp_state * (sb_amp * 0.04);
            }
            if st_signed != 0.0 {
                let hp = 0.85 * (self.hp_prev_out + mono - self.hp_prev_in);
                self.hp_prev_in = mono;
                self.hp_prev_out = hp;
                mono += hp * (st_signed * 0.04);
            }
            frame_mono.push(mono.clamp(-1.0, 1.0));
        }

        if count == 0 {
            self.active = false;
            self.current_freq_hz = 0.0;
            return None;
        }

        // Kayan 512-örneklik pencereyi güncelle
        let n = frame_mono.len();
        if n >= PITCH_WINDOW {
            self.history.copy_from_slice(&frame_mono[n - PITCH_WINDOW..]);
            self.hist_filled = PITCH_WINDOW;
        } else {
            self.history.copy_within(n..PITCH_WINDOW, 0);
            self.history[PITCH_WINDOW - n..].copy_from_slice(&frame_mono);
            self.hist_filled = (self.hist_filled + n).min(PITCH_WINDOW);
        }

        let rms_l = (sum_sq_l / count as f32).sqrt();
        let rms_r = (sum_sq_r / count as f32).sqrt();
        self.env_l = (self.env_l * 0.35 + rms_l * 1.45).clamp(0.0, 1.0);
        self.env_r = (self.env_r * 0.35 + rms_r * 1.45).clamp(0.0, 1.0);

        let muted_or_silent = (gain_l <= 0.001 && gain_r <= 0.001) || (rms_l + rms_r) * 0.5 < 0.006;
        if muted_or_silent {
            self.active = false;
            self.current_freq_hz = 0.0;
            return None;
        }

        let win_slice = &self.history[PITCH_WINDOW - self.hist_filled..];
        if let Some(freq) = detect_fundamental_hz(win_slice, sr) {
            self.active = true;
            self.current_freq_hz = freq;
            Some(freq)
        } else if self.current_freq_hz > 60.0 {
            self.active = true;
            Some(self.current_freq_hz)
        } else {
            self.active = false;
            None
        }
    }

    /// Birim testleri için tek adımlı sıfır geçişli kare dalga okuyucu.
    pub fn tick(
        &mut self,
        ring: &mut PcmRing,
        gain_l: f32,
        gain_r: f32,
        _bass_reg: u16,
        user_gain_pct: f32,
    ) -> u32 {
        let user_gain = if user_gain_pct <= 0.0 {
            1.0
        } else {
            (user_gain_pct / 100.0).clamp(0.1, 2.0)
        };
        let Some((raw_l, raw_r)) = ring.next_interpolated() else {
            self.active = false;
            return 0;
        };
        let sl = raw_l * gain_l * user_gain;
        let sr = raw_r * gain_r * user_gain;
        self.env_l = (self.env_l * 0.99 + sl.abs() * 0.02).clamp(0.0, 1.0);
        self.env_r = (self.env_r * 0.99 + sr.abs() * 0.02).clamp(0.0, 1.0);
        let mono = 0.5 * (sl + sr);
        self.active = (gain_l > 0.001 || gain_r > 0.001) && mono.abs() > 0.002;
        if self.active && mono > 0.003 { 1 } else { 0 }
    }

    #[inline]
    pub fn left_dac_voltage(&self) -> f32 {
        (self.env_l * 3.3).clamp(0.0, 3.3)
    }

    #[inline]
    pub fn right_dac_voltage(&self) -> f32 {
        (self.env_r * 3.3).clamp(0.0, 3.3)
    }
}

/// Normalize Otokorelasyon (NSDF / McLeod benzeri) ile alt-örnek hassasiyetli temel frekans (f0) tespiti.
/// Harmonik içerikli müzik sinyallerinde oktav hatası yapmadan ilk güçlü periyot tepesini yakalar.
pub fn detect_fundamental_hz(samples: &[f32], sample_rate: u32) -> Option<f32> {
    let n = samples.len();
    if n < 64 {
        return None;
    }

    let mean = samples.iter().copied().sum::<f32>() / n as f32;
    let mut energy = 0.0_f32;
    for &s in samples {
        let d = s - mean;
        energy += d * d;
    }
    if energy < 1e-6 {
        return None;
    }

    let min_lag = ((sample_rate / 3200) as usize).max(4);
    let max_lag = ((sample_rate / 85) as usize).min(n / 2);
    if min_lag + 2 >= max_lag {
        return None;
    }

    let mut nsdf = vec![0.0_f32; max_lag + 1];
    for lag in 1..=max_lag {
        let mut acf = 0.0_f32;
        let mut norm = 0.0_f32;
        let limit = n - lag;
        for i in 0..limit {
            let a = samples[i] - mean;
            let b = samples[i + lag] - mean;
            acf += a * b;
            norm += a * a + b * b;
        }
        if norm > 1e-8 {
            nsdf[lag] = 2.0 * acf / norm;
        }
    }

    // İlk negatif geçişi bul (DC tepesinden çıkmak için)
    let mut start = min_lag;
    while start < max_lag && nsdf[start] > 0.0 {
        start += 1;
    }
    while start < max_lag && nsdf[start] <= 0.0 {
        start += 1;
    }
    if start >= max_lag - 1 {
        // Yedek: yükselen sıfır geçişlerinden ortalama periyot
        return zero_crossing_hz(samples, sample_rate, mean);
    }

    // Pozitif bölgedeki tüm yerel tepeleri bul ve genel maksimumu belirle
    let mut peaks: Vec<(usize, f32)> = Vec::new();
    let mut global_max = 0.0_f32;
    let mut cur_lag = start;
    while cur_lag < max_lag {
        if nsdf[cur_lag] > 0.0 {
            let mut best_l = cur_lag;
            let mut best_v = nsdf[cur_lag];
            while cur_lag <= max_lag && nsdf[cur_lag] > 0.0 {
                if nsdf[cur_lag] > best_v {
                    best_v = nsdf[cur_lag];
                    best_l = cur_lag;
                }
                cur_lag += 1;
            }
            if best_v > global_max {
                global_max = best_v;
            }
            peaks.push((best_l, best_v));
        } else {
            cur_lag += 1;
        }
    }

    if global_max < 0.25 || peaks.is_empty() {
        return zero_crossing_hz(samples, sample_rate, mean);
    }

    // Oktav hatasını önlemek için global_max * 0.82 eşiğini aşan İLK tepeyi seç (McLeod kuralı)
    let threshold = global_max * 0.82;
    let (best_lag, _) = peaks
        .iter()
        .copied()
        .find(|&(_, v)| v >= threshold)
        .unwrap_or(peaks[0]);

    // Parabolik interpolasyon ile alt-örnek (sub-sample) periyot hassasiyeti
    let refined_lag = if best_lag > 1 && best_lag < max_lag {
        let y0 = nsdf[best_lag - 1];
        let y1 = nsdf[best_lag];
        let y2 = nsdf[best_lag + 1];
        let denom = 2.0 * y1 - y0 - y2;
        if denom.abs() > 1e-6 {
            let delta = 0.5 * (y0 - y2) / denom;
            (best_lag as f32 + delta.clamp(-0.5, 0.5)).max(1.0)
        } else {
            best_lag as f32
        }
    } else {
        best_lag as f32
    };

    let freq = sample_rate as f32 / refined_lag;
    if (75.0..=3600.0).contains(&freq) {
        Some(freq)
    } else {
        None
    }
}

fn zero_crossing_hz(samples: &[f32], sample_rate: u32, mean: f32) -> Option<f32> {
    let mut crossings = Vec::new();
    for i in 1..samples.len() {
        let prev = samples[i - 1] - mean;
        let cur = samples[i] - mean;
        if prev <= 0.0 && cur > 0.0 {
            let frac = if (cur - prev).abs() > 1e-6 {
                -prev / (cur - prev)
            } else {
                0.0
            };
            crossings.push((i - 1) as f32 + frac);
        }
    }
    if crossings.len() < 2 {
        return None;
    }
    let total_span = crossings.last().unwrap() - crossings.first().unwrap();
    let periods = (crossings.len() - 1) as f32;
    if total_span <= 1.0 {
        return None;
    }
    let avg_period = total_span / periods;
    let freq = sample_rate as f32 / avg_period;
    if (75.0..=3600.0).contains(&freq) {
        Some(freq)
    } else {
        None
    }
}
