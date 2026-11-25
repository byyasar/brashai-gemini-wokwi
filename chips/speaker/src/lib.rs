//! Wokwi Hoparlör Geliştirme Kartı (chip-speaker)
//! - SPK_IN pininden gelen Sigma-Delta PDM ses işaretini mute=0 iken gecikmesiz SPK_OUT'a aktarır
//!   (SPK_OUT doğrudan wokwi-buzzer accurate moduna bağlanır).
//! - LEFT ve RIGHT analog zarf pinlerini (0.0V..3.3V) 30 Hz'de okuyarak
//!   128x128 Framebuffer ekranında canlı diyafram titreşimi + stereo VU-metre çizer.

use core::ffi::c_void;
use wokwi_api::gfx::{rgba, Canvas};
use wokwi_api::*;

struct SpeakerState {
    spk_in: PinT,
    spk_out: PinT,
    left: PinT,
    right: PinT,
    vcc: PinT,
    attr_mute: AttrT,
    fb: BufferT,
    canvas: Canvas,
    level_l: f32,
    level_r: f32,
    peak_l: f32,
    peak_r: f32,
    phase: u32,
    last_out: u32,
}

static mut STATE: Option<SpeakerState> = None;

impl SpeakerState {
    fn render(&mut self) {
        let powered = pin_read(self.vcc) == HIGH;
        let muted = attr_read(self.attr_mute) != 0;

        let raw_l = (pin_adc_read(self.left) / 3.3).clamp(0.0, 1.0);
        let raw_r = (pin_adc_read(self.right) / 3.3).clamp(0.0, 1.0);

        if muted || !powered {
            self.level_l *= 0.6;
            self.level_r *= 0.6;
        } else {
            self.level_l = self.level_l * 0.35 + raw_l * 0.65;
            self.level_r = self.level_r * 0.35 + raw_r * 0.65;
        }
        self.peak_l = (self.peak_l - 0.03).max(self.level_l).clamp(0.0, 1.0);
        self.peak_r = (self.peak_r - 0.03).max(self.level_r).clamp(0.0, 1.0);

        let avg = (self.level_l + self.level_r) * 0.5;
        self.phase = self.phase.wrapping_add(1);

        // Kabin arka planı
        self.canvas.clear(rgba(22, 24, 30, 255));
        self.canvas.draw_rect(0, 0, 128, 128, rgba(70, 75, 90, 255));

        // 4 köşe vida
        for &(vx, vy) in &[(7, 7), (120, 7), (7, 94), (120, 94)] {
            self.canvas.fill_circle(vx, vy, 3, rgba(130, 138, 150, 255));
            self.canvas.draw_circle(vx, vy, 3, rgba(50, 54, 62, 255));
        }

        // Üst etiket
        self.canvas.draw_text(22, 4, "AUDIO SPEAKER", rgba(220, 225, 235, 255), 1);
        self.canvas.draw_led(
            110,
            7,
            2,
            powered && !muted,
            rgba(60, 230, 120, 255),
            rgba(180, 40, 40, 255),
        );

        // Hoparlör merkezi (64, 52)
        let cx = 64;
        let cy = 52;
        let vib = if avg > 0.04 && !muted && powered {
            let dir = if self.phase % 2 == 0 { 1.0 } else { -0.5 };
            (avg * 4.5 * dir) as i32
        } else {
            0
        };

        // Dış şasi halkası
        self.canvas.fill_circle(cx, cy, 38, rgba(38, 42, 50, 255));
        self.canvas.draw_circle(cx, cy, 38, rgba(110, 118, 132, 255));

        // Titreşen süspansiyon ve koni
        let cone_r = (31 + vib).clamp(26, 36);
        self.canvas.fill_circle(cx, cy, cone_r, rgba(52, 58, 70, 255));
        self.canvas.draw_circle(cx, cy, cone_r, rgba(95, 105, 125, 255));

        let mid_r = (21 + vib).clamp(17, 25);
        self.canvas.draw_circle(cx, cy, mid_r, rgba(72, 80, 96, 255));

        // Merkez toz kapağı (aktif seste mavi/turkuaz parlama)
        let cap_r = (11 + vib / 2).clamp(9, 14);
        let cap_col = if muted {
            rgba(140, 45, 45, 255)
        } else if avg > 0.08 {
            let glow = (110.0 + avg * 145.0).min(255.0) as u8;
            rgba(35, glow / 2 + 60, glow, 255)
        } else {
            rgba(45, 50, 62, 255)
        };
        self.canvas.fill_circle(cx, cy, cap_r, cap_col);
        self.canvas.draw_circle(cx, cy, cap_r, rgba(160, 175, 200, 255));

        // Ses dalgası halkaları (ses yüksekken)
        if avg > 0.12 && !muted && powered {
            let wave_r = 39 + ((self.phase % 3) as i32);
            self.canvas.draw_circle(cx, cy, wave_r, rgba(70, 200, 255, 255));
        }

        // Mute uyarısı
        if muted {
            self.canvas.draw_line(40, 28, 88, 76, rgba(245, 70, 70, 255));
            self.canvas.draw_line(88, 28, 40, 76, rgba(245, 70, 70, 255));
            self.canvas.fill_rect(44, 46, 40, 13, rgba(150, 20, 20, 235));
            self.canvas.draw_text(51, 49, "MUTE", rgba(255, 255, 255, 255), 1);
        }

        // Alt Stereo VU-Metre (L ve R)
        self.canvas.fill_rect(4, 98, 120, 26, rgba(12, 14, 18, 255));
        self.canvas.draw_rect(4, 98, 120, 26, rgba(60, 66, 78, 255));

        self.canvas.draw_text(8, 102, "L", rgba(180, 220, 255, 255), 1);
        self.draw_vu_bar(18, 102, 100, 7, self.level_l, self.peak_l);

        self.canvas.draw_text(8, 113, "R", rgba(180, 220, 255, 255), 1);
        self.draw_vu_bar(18, 113, 100, 7, self.level_r, self.peak_r);

        self.canvas.flush(self.fb);
    }

    fn draw_vu_bar(&mut self, x: i32, y: i32, w: i32, h: i32, level: f32, peak: f32) {
        self.canvas.fill_rect(x, y, w, h, rgba(28, 32, 38, 255));
        let segs = 20;
        let seg_w = w / segs;
        let lit = (level * segs as f32).round() as i32;
        for i in 0..segs {
            if i < lit {
                let col = if i < 12 {
                    rgba(50, 220, 100, 255)
                } else if i < 17 {
                    rgba(245, 200, 45, 255)
                } else {
                    rgba(245, 65, 65, 255)
                };
                self.canvas.fill_rect(x + i * seg_w + 1, y + 1, seg_w - 1, h - 2, col);
            }
        }
        let peak_x = x + ((peak * (w - 2) as f32) as i32).clamp(0, w - 2);
        if peak > 0.03 {
            self.canvas.fill_rect(peak_x, y, 2, h, rgba(255, 255, 255, 255));
        }
    }
}

extern "C" fn on_spk_in_change(_user_data: *mut c_void, _pin: PinT, value: u32) {
    unsafe {
        if let Some(st) = STATE.as_mut() {
            let muted = attr_read(st.attr_mute) != 0;
            let out = if muted { LOW } else { value };
            if out != st.last_out {
                st.last_out = out;
                pin_write(st.spk_out, out);
            }
        }
    }
}

extern "C" fn on_ui_timer(_user_data: *mut c_void) {
    unsafe {
        if let Some(st) = STATE.as_mut() {
            // Mute durumunda SPK_OUT'u mutlaka LOW tut
            if attr_read(st.attr_mute) != 0 && st.last_out != LOW {
                st.last_out = LOW;
                pin_write(st.spk_out, LOW);
            }
            st.render();
        }
    }
}

#[no_mangle]
pub extern "C" fn chip_init() {
    chipInit();
}

#[no_mangle]
pub extern "C" fn chipInit() {
    let spk_in = pin_init(b"SPK_IN\0", INPUT);
    let spk_out = pin_init(b"SPK_OUT\0", OUTPUT_LOW);
    let left = pin_init(b"LEFT\0", ANALOG);
    let right = pin_init(b"RIGHT\0", ANALOG);
    let vcc = pin_init(b"VCC\0", INPUT);
    let _gnd = pin_init(b"GND\0", INPUT);

    let attr_mute = attr_init(b"mute\0", 0);
    let (fb, w, h) = framebuffer_init();
    let canvas = Canvas::new(w.max(128), h.max(128));

    unsafe {
        STATE = Some(SpeakerState {
            spk_in,
            spk_out,
            left,
            right,
            vcc,
            attr_mute,
            fb,
            canvas,
            level_l: 0.0,
            level_r: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
            phase: 0,
            last_out: LOW,
        });
    }

    let watch = PinWatchConfig {
        user_data: core::ptr::null_mut(),
        edge: BOTH,
        pin_change: Some(on_spk_in_change),
    };
    pin_watch(spk_in, &watch);

    let timer_cfg = TimerConfig {
        user_data: core::ptr::null_mut(),
        callback: Some(on_ui_timer),
        reserved: [0; 8],
    };
    let t = timer_init(&timer_cfg);
    timer_start(t, 33_333, true); // 30 Hz hoparlör diyafram & VU-metre yenileme
}
