//! Wokwi Framebuffer API için hafif RGBA32 çizim ve 5x7 piksel ASCII metin motoru.

use crate::{buffer_write, BufferT};

/// RGBA renk sabiti üretir (little-endian Wokwi framebuffer: R, G, B, A).
#[inline]
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    [r, g, b, a]
}

pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        let w = width as usize;
        let h = height as usize;
        Self {
            width: w,
            height: h,
            pixels: vec![0u8; w * h * 4],
        }
    }

    pub fn clear(&mut self, color: [u8; 4]) {
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&color);
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x < 0 || y < 0 {
            return;
        }
        let ux = x as usize;
        let uy = y as usize;
        if ux >= self.width || uy >= self.height {
            return;
        }
        let idx = (uy * self.width + ux) * 4;
        self.pixels[idx..idx + 4].copy_from_slice(&color);
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: [u8; 4]) {
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.width as i32);
        let y1 = (y + h).min(self.height as i32);
        for py in y0..y1 {
            for px in x0..x1 {
                self.set_pixel(px, py, color);
            }
        }
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: [u8; 4]) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.fill_rect(x, y, w, 1, color);
        self.fill_rect(x, y + h - 1, w, 1, color);
        self.fill_rect(x, y, 1, h, color);
        self.fill_rect(x + w - 1, y, 1, h, color);
    }

    pub fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, color: [u8; 4]) {
        if r <= 0 {
            self.set_pixel(cx, cy, color);
            return;
        }
        let r2 = r * r;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r2 {
                    self.set_pixel(cx + dx, cy + dy, color);
                }
            }
        }
    }

    pub fn draw_circle(&mut self, cx: i32, cy: i32, r: i32, color: [u8; 4]) {
        if r <= 0 {
            self.set_pixel(cx, cy, color);
            return;
        }
        let mut x = r;
        let mut y = 0;
        let mut err = 1 - x;
        while x >= y {
            self.set_pixel(cx + x, cy + y, color);
            self.set_pixel(cx + y, cy + x, color);
            self.set_pixel(cx - y, cy + x, color);
            self.set_pixel(cx - x, cy + y, color);
            self.set_pixel(cx - x, cy - y, color);
            self.set_pixel(cx - y, cy - x, color);
            self.set_pixel(cx + y, cy - x, color);
            self.set_pixel(cx + x, cy - y, color);
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    pub fn draw_line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: [u8; 4]) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.set_pixel(x0, y0, color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    pub fn draw_led(&mut self, cx: i32, cy: i32, r: i32, on: bool, on_color: [u8; 4], off_color: [u8; 4]) {
        let c = if on { on_color } else { off_color };
        self.fill_circle(cx, cy, r, c);
        self.draw_circle(cx, cy, r, rgba(200, 200, 210, 255));
    }

    pub fn draw_progress_bar(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        ratio: f32,
        fg: [u8; 4],
        bg: [u8; 4],
        border: [u8; 4],
    ) {
        self.fill_rect(x, y, w, h, bg);
        let clamped = ratio.clamp(0.0, 1.0);
        let fill_w = ((w - 2) as f32 * clamped) as i32;
        if fill_w > 0 {
            self.fill_rect(x + 1, y + 1, fill_w, h - 2, fg);
        }
        self.draw_rect(x, y, w, h, border);
    }

    pub fn draw_text(&mut self, mut x: i32, y: i32, text: &str, color: [u8; 4], scale: i32) {
        let s = scale.max(1);
        for ch in text.bytes() {
            let glyph = glyph_5x7(ch);
            for (col_idx, &col_bits) in glyph.iter().enumerate() {
                for row_idx in 0..7 {
                    if (col_bits >> row_idx) & 1 != 0 {
                        self.fill_rect(
                            x + (col_idx as i32) * s,
                            y + (row_idx as i32) * s,
                            s,
                            s,
                            color,
                        );
                    }
                }
            }
            x += 6 * s;
        }
    }

    #[inline]
    pub fn flush(&self, fb: BufferT) {
        buffer_write(fb, 0, &self.pixels);
    }
}

/// 5x7 piksel temel ASCII bitmap yazı tipi (sütun-majör, alt bit = üst satır).
fn glyph_5x7(ch: u8) -> [u8; 5] {
    let c = if ch.is_ascii_lowercase() {
        ch.to_ascii_uppercase()
    } else {
        ch
    };
    match c {
        b' ' => [0x00, 0x00, 0x00, 0x00, 0x00],
        b'!' => [0x00, 0x00, 0x5F, 0x00, 0x00],
        b'%' => [0x23, 0x13, 0x08, 0x64, 0x62],
        b'(' => [0x00, 0x1C, 0x22, 0x41, 0x00],
        b')' => [0x00, 0x41, 0x22, 0x1C, 0x00],
        b'+' => [0x08, 0x08, 0x3E, 0x08, 0x08],
        b',' => [0x00, 0x50, 0x30, 0x00, 0x00],
        b'-' => [0x08, 0x08, 0x08, 0x08, 0x08],
        b'.' => [0x00, 0x60, 0x60, 0x00, 0x00],
        b'/' => [0x20, 0x10, 0x08, 0x04, 0x02],
        b':' => [0x00, 0x36, 0x36, 0x00, 0x00],
        b'=' => [0x14, 0x14, 0x14, 0x14, 0x14],
        b'[' => [0x00, 0x7F, 0x41, 0x41, 0x00],
        b']' => [0x00, 0x41, 0x41, 0x7F, 0x00],
        b'_' => [0x40, 0x40, 0x40, 0x40, 0x40],
        b'0' => [0x3E, 0x51, 0x49, 0x45, 0x3E],
        b'1' => [0x00, 0x42, 0x7F, 0x40, 0x00],
        b'2' => [0x42, 0x61, 0x51, 0x49, 0x46],
        b'3' => [0x21, 0x41, 0x45, 0x4B, 0x31],
        b'4' => [0x18, 0x14, 0x12, 0x7F, 0x10],
        b'5' => [0x27, 0x45, 0x45, 0x45, 0x39],
        b'6' => [0x3C, 0x4A, 0x49, 0x49, 0x30],
        b'7' => [0x01, 0x71, 0x09, 0x05, 0x03],
        b'8' => [0x36, 0x49, 0x49, 0x49, 0x36],
        b'9' => [0x06, 0x49, 0x49, 0x29, 0x1E],
        b'A' => [0x7E, 0x11, 0x11, 0x11, 0x7E],
        b'B' => [0x7F, 0x49, 0x49, 0x49, 0x36],
        b'C' => [0x3E, 0x41, 0x41, 0x41, 0x22],
        b'D' => [0x7F, 0x41, 0x41, 0x22, 0x1C],
        b'E' => [0x7F, 0x49, 0x49, 0x49, 0x41],
        b'F' => [0x7F, 0x09, 0x09, 0x09, 0x01],
        b'G' => [0x3E, 0x41, 0x49, 0x49, 0x7A],
        b'H' => [0x7F, 0x08, 0x08, 0x08, 0x7F],
        b'I' => [0x00, 0x41, 0x7F, 0x41, 0x00],
        b'J' => [0x20, 0x40, 0x41, 0x3F, 0x01],
        b'K' => [0x7F, 0x08, 0x14, 0x22, 0x41],
        b'L' => [0x7F, 0x40, 0x40, 0x40, 0x40],
        b'M' => [0x7F, 0x02, 0x0C, 0x02, 0x7F],
        b'N' => [0x7F, 0x04, 0x08, 0x10, 0x7F],
        b'O' => [0x3E, 0x41, 0x41, 0x41, 0x3E],
        b'P' => [0x7F, 0x09, 0x09, 0x09, 0x06],
        b'Q' => [0x3E, 0x41, 0x51, 0x21, 0x5E],
        b'R' => [0x7F, 0x09, 0x19, 0x29, 0x46],
        b'S' => [0x46, 0x49, 0x49, 0x49, 0x31],
        b'T' => [0x01, 0x01, 0x7F, 0x01, 0x01],
        b'U' => [0x3F, 0x40, 0x40, 0x40, 0x3F],
        b'V' => [0x1F, 0x20, 0x40, 0x20, 0x1F],
        b'W' => [0x3F, 0x40, 0x38, 0x40, 0x3F],
        b'X' => [0x63, 0x14, 0x08, 0x14, 0x63],
        b'Y' => [0x07, 0x08, 0x70, 0x08, 0x07],
        b'Z' => [0x61, 0x51, 0x49, 0x45, 0x43],
        _ => [0x7F, 0x41, 0x41, 0x41, 0x7F],
    }
}
