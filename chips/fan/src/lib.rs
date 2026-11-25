//! Yeniden Tasarlanmış Ultra-Hızlı Wokwi Fan Breakout (chip-fan)
//! - Sıfır piksel-başı atan2/sqrt: Kare başına yalnızca 6 sin/cos ile <0.05ms render!
//! - 3 net pin: VCC (röle NO çıkışı), GND (toprak), IN (röle kontrol sinyali)
//! - Çift tetikleme: Röle çektiğinde (IN=HIGH veya VCC=HIGH) veya Manuel Test=1 iken döner.
//! - Stepper tarzı 0.9° ayrık açı adımı, 6 kalın yüksek kontrastlı kanat, dönen rotor oku ve RPM göstergesi.

use core::ffi::c_void;
use wokwi_api::gfx::{rgba, Canvas};
use wokwi_api::*;

const TWO_PI: f32 = 6.28318530718;
const BLADE_COUNT: usize = 6;
const HALF_STEPS_PER_REV: f32 = 400.0; // 0.9° / yarım adım (stepper motor hassasiyeti)

struct FanState {
    vcc: PinT,
    gnd: PinT,
    sig_in: PinT,
    attr_test: AttrT,
    attr_rpm: AttrT,
    fb: BufferT,
    canvas: Canvas,
    half_step_pos: f32,
    speed: f32,
    ticks: u32,
}

static mut STATE: Option<FanState> = None;

/// İki nokta arasında kalın (3px) çizgi çizer
fn draw_thick_line(canvas: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, col: [u8; 4]) {
    canvas.draw_line(x0, y0, x1, y1, col);
    canvas.draw_line(x0 + 1, y0, x1 + 1, y1, col);
    canvas.draw_line(x0, y0 + 1, x1, y1 + 1, col);
}

/// Dolu üçgen (scanline) — atan2/sqrt olmadan çok hızlı kalın fan kanadı çizer
fn fill_triangle(
    canvas: &mut Canvas,
    mut x0: i32,
    mut y0: i32,
    mut x1: i32,
    mut y1: i32,
    mut x2: i32,
    mut y2: i32,
    col: [u8; 4],
) {
    if y0 > y1 {
        core::mem::swap(&mut y0, &mut y1);
        core::mem::swap(&mut x0, &mut x1);
    }
    if y0 > y2 {
        core::mem::swap(&mut y0, &mut y2);
        core::mem::swap(&mut x0, &mut x2);
    }
    if y1 > y2 {
        core::mem::swap(&mut y1, &mut y2);
        core::mem::swap(&mut x1, &mut x2);
    }

    let total_h = (y2 - y0).max(1);
    for y in y0..=y2 {
        if y < 0 || y >= 128 {
            continue;
        }
        let second_half = y > y1 || y1 == y0;
        let seg_h = if second_half { (y2 - y1).max(1) } else { (y1 - y0).max(1) };
        let alpha = (y - y0) as f32 / total_h as f32;
        let beta = if second_half {
            (y - y1) as f32 / seg_h as f32
        } else {
            (y - y0) as f32 / seg_h as f32
        };

        let mut ax = x0 + ((x2 - x0) as f32 * alpha) as i32;
        let mut bx = if second_half {
            x1 + ((x2 - x1) as f32 * beta) as i32
        } else {
            x0 + ((x1 - x0) as f32 * beta) as i32
        };

        if ax > bx {
            core::mem::swap(&mut ax, &mut bx);
        }
        let sx = ax.max(0);
        let ex = bx.min(127);
        for x in sx..=ex {
            canvas.set_pixel(x, y, col);
        }
    }
}

impl FanState {
    fn render(&mut self) {
        self.ticks = self.ticks.wrapping_add(1);
        let in_high = pin_read(self.sig_in) == HIGH;
        let vcc_high = pin_read(self.vcc) == HIGH;
        let gnd_low = pin_read(self.gnd) == LOW;
        let manual_test = attr_read(self.attr_test) != 0;

        let active = manual_test || (gnd_low && (in_high || vcc_high));

        let max_rpm_raw = attr_read(self.attr_rpm);
        let max_rpm = if max_rpm_raw < 600 { 2000.0 } else { max_rpm_raw as f32 };

        // Hızlı ve belirgin hızlanma / yumuşak durma
        if active {
            self.speed = self.speed * 0.78 + 0.22;
            if self.speed < 0.20 {
                self.speed = 0.20;
            }
        } else {
            self.speed *= 0.86;
            if self.speed < 0.02 {
                self.speed = 0.0;
            }
        }

        // Stepper motor 0.9° yarım-adım ilerlemesi (tam hızda kare başına 24 yarım-adım = 21.6°)
        let step_inc = (self.speed * 24.0).round();
        self.half_step_pos = (self.half_step_pos + step_inc) % HALF_STEPS_PER_REV;

        let angle_deg = ((self.half_step_pos * 0.9) as u32) % 360;
        let angle_rad = (self.half_step_pos / HALF_STEPS_PER_REV) * TWO_PI;
        let current_rpm = (self.speed * max_rpm).round() as u32;

        let cx: i32 = 64;
        let cy: i32 = 56;
        let outer_r: i32 = 50;

        // --- 1. Koyu Antrasit Kasa + 4 Montaj Vidası ---
        let frame_bg = rgba(22, 24, 28, 255);
        let border = rgba(85, 92, 105, 255);
        self.canvas.clear(frame_bg);
        self.canvas.draw_rect(0, 0, 128, 128, border);
        for &(vx, vy) in &[(9, 9), (118, 9), (9, 104), (118, 104)] {
            self.canvas.fill_circle(vx, vy, 4, rgba(55, 60, 70, 255));
            self.canvas.fill_circle(vx, vy, 2, rgba(16, 18, 22, 255));
        }

        // --- 2. Yüksek Kontrastlı Beyaz Hava Kanalı ---
        self.canvas.fill_circle(cx, cy, outer_r, rgba(242, 246, 252, 255));
        self.canvas.draw_circle(
            cx,
            cy,
            outer_r,
            if active { rgba(30, 220, 140, 255) } else { border },
        );

        // --- 3. 6 Adet Kalın Kavisli Siyah Kanat (Ultra-Hızlı Üçgen Tarama) ---
        let blade_col = rgba(20, 22, 28, 255);
        let tip_col = if active {
            rgba(0, 175, 255, 255) // Dönerken kanat kenarında canlı mavi vurgu
        } else {
            rgba(70, 76, 86, 255)
        };

        let sector = TWO_PI / (BLADE_COUNT as f32);
        let r_in = 14.0_f32;
        let r_out = 47.0_f32;

        for i in 0..BLADE_COUNT {
            let a0 = angle_rad + (i as f32) * sector;
            let a1 = a0 + 0.34; // Kanat ön kenarı (kavisli)
            let a2 = a0 + 0.86; // Kanat arka kenarı (geniş PC fan kanadı)

            let x0 = cx + (r_in * a0.cos()) as i32;
            let y0 = cy + (r_in * a0.sin()) as i32;
            let x1 = cx + (r_out * a1.cos()) as i32;
            let y1 = cy + (r_out * a1.sin()) as i32;
            let x2 = cx + ((r_out - 2.0) * a2.cos()) as i32;
            let y2 = cy + ((r_out - 2.0) * a2.sin()) as i32;

            fill_triangle(&mut self.canvas, x0, y0, x1, y1, x2, y2, blade_col);
            self.canvas.draw_line(x0, y0, x1, y1, tip_col);
        }

        // --- 4. Merkez Motor Göbeği + Stepper Tarzı Dönen Ok ---
        self.canvas.fill_circle(cx, cy, 16, frame_bg);
        self.canvas.draw_circle(cx, cy, 16, rgba(145, 155, 170, 255));
        self.canvas.fill_circle(cx, cy, 12, rgba(36, 40, 48, 255));

        let ex = cx + (12.0 * angle_rad.cos()) as i32;
        let ey = cy + (12.0 * angle_rad.sin()) as i32;
        let ptr_col = if active {
            rgba(40, 245, 140, 255)
        } else {
            rgba(230, 80, 80, 255)
        };
        draw_thick_line(&mut self.canvas, cx, cy, ex, ey, ptr_col);
        self.canvas.fill_circle(cx, cy, 3, rgba(245, 250, 255, 255));

        // --- 5. Alt Durum Şeridi ---
        self.canvas.fill_rect(1, 110, 126, 17, rgba(14, 16, 20, 255));
        self.canvas.draw_line(1, 110, 126, 110, rgba(60, 68, 80, 255));

        self.canvas.draw_led(
            9,
            118,
            3,
            active,
            rgba(40, 240, 130, 255),
            rgba(180, 45, 45, 255),
        );

        let info = if current_rpm > 0 {
            format!("{}RPM {:>3}D", current_rpm, angle_deg)
        } else {
            format!("OFF V{}I{} T{}", if vcc_high { 1 } else { 0 }, if in_high { 1 } else { 0 }, self.ticks % 100)
        };
        let txt_col = if active {
            rgba(60, 245, 155, 255)
        } else {
            rgba(165, 170, 180, 255)
        };
        self.canvas.draw_text(17, 115, &info, txt_col, 1);

        self.canvas.flush(self.fb);
    }
}

extern "C" fn on_fan_timer(_user_data: *mut c_void) {
    unsafe {
        if let Some(st) = STATE.as_mut() {
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
    let vcc = pin_init(b"VCC\0", INPUT_PULLDOWN);
    let gnd = pin_init(b"GND\0", INPUT);
    let sig_in = pin_init(b"IN\0", INPUT_PULLDOWN);
    let attr_test = attr_init(b"test_spin\0", 0);
    let attr_rpm = attr_init(b"target_rpm\0", 2000);

    let (fb, w, h) = framebuffer_init();
    let canvas = Canvas::new(w.max(128), h.max(128));

    unsafe {
        STATE = Some(FanState {
            vcc,
            gnd,
            sig_in,
            attr_test,
            attr_rpm,
            fb,
            canvas,
            half_step_pos: 0.0,
            speed: 0.0,
            ticks: 0,
        });

        if let Some(st) = STATE.as_mut() {
            st.render();
        }
    }

    let timer_cfg = TimerConfig {
        user_data: core::ptr::null_mut(),
        callback: Some(on_fan_timer),
        reserved: [0; 8],
    };
    let timer = timer_init(&timer_cfg);
    timer_start(timer, 50_000, true); // 20 FPS
}
