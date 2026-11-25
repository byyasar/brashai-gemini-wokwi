//! Wokwi SD Kart Geliştirme Kartı (chip-sd-board)
//! wokwi-microsd-card ile aynı SPI veriyolunu salt-okunur (INPUT) modda izler;
//! MISO/MOSI/SCK/CS hatlarını asla sürmez, yalnızca CD pinini kontrol eder ve
//! 128x80 Framebuffer ekranında canlı SPI aktivitesi / bayt sayacı gösterir.

use core::ffi::c_void;
use wokwi_api::gfx::{rgba, Canvas};
use wokwi_api::*;

struct SdBoardState {
    vcc: PinT,
    cs: PinT,
    sck: PinT,
    cd: PinT,
    attr_card_present: AttrT,
    fb: BufferT,
    canvas: Canvas,
    cs_active: bool,
    bit_count: u8,
    total_bytes: u64,
    window_bytes: u32,
    last_bps: u32,
    act_hold_ticks: u8,
    tick_counter: u32,
}

static mut STATE: Option<SdBoardState> = None;

impl SdBoardState {
    fn render(&mut self) {
        let powered = pin_read(self.vcc) == HIGH;
        let card_ok = attr_read(self.attr_card_present) != 0;
        pin_write(self.cd, if card_ok { HIGH } else { LOW });

        let bg = rgba(16, 46, 36, 255); // Koyu yeşil PCB
        let panel = rgba(10, 28, 22, 255);
        let white = rgba(235, 240, 240, 255);
        let dim = rgba(130, 160, 150, 255);
        let gold = rgba(235, 190, 70, 255);

        self.canvas.clear(bg);
        self.canvas.draw_rect(0, 0, 128, 80, rgba(45, 110, 85, 255));

        // Başlık
        self.canvas.draw_text(4, 4, "SD DEV BOARD", white, 1);

        // PWR ve ACT LED'leri
        self.canvas.draw_led(92, 7, 3, powered, rgba(40, 230, 90, 255), rgba(25, 60, 35, 255));
        self.canvas.draw_text(98, 4, "P", dim, 1);
        let act_on = powered && self.act_hold_ticks > 0;
        self.canvas.draw_led(112, 7, 3, act_on, rgba(255, 150, 30, 255), rgba(70, 40, 15, 255));
        self.canvas.draw_text(118, 4, "A", dim, 1);

        // Sol tarafta microSD yuva ikonu
        self.canvas.fill_rect(5, 16, 34, 42, rgba(165, 175, 185, 255));
        self.canvas.draw_rect(5, 16, 34, 42, white);
        // Kart gövdesi
        if card_ok {
            self.canvas.fill_rect(9, 20, 26, 34, rgba(30, 34, 42, 255));
            self.canvas.fill_rect(12, 22, 20, 7, gold);
            self.canvas.draw_text(13, 36, "SD", white, 1);
            self.canvas.draw_text(10, 45, "FAT", rgba(110, 210, 255, 255), 1);
        } else {
            self.canvas.fill_rect(9, 20, 26, 34, rgba(45, 45, 50, 255));
            self.canvas.draw_text(13, 34, "NO", rgba(240, 80, 80, 255), 1);
        }

        // Sağ bilgi paneli
        self.canvas.fill_rect(43, 16, 80, 42, panel);
        self.canvas.draw_rect(43, 16, 80, 42, rgba(55, 125, 95, 255));

        let cd_str = if card_ok { "CD: TAKILI" } else { "CD: YOK" };
        let cd_col = if card_ok { rgba(80, 235, 130, 255) } else { rgba(240, 90, 90, 255) };
        self.canvas.draw_text(46, 20, cd_str, cd_col, 1);

        let cs_str = if self.cs_active { "CS: AKTIF" } else { "CS: BOSTA" };
        let cs_col = if self.cs_active { rgba(255, 200, 60, 255) } else { dim };
        self.canvas.draw_text(46, 31, cs_str, cs_col, 1);

        let kb = self.total_bytes / 1024;
        let bytes_txt = if kb > 0 {
            format!("RX:{} KB", kb)
        } else {
            format!("RX:{} B", self.total_bytes)
        };
        self.canvas.draw_text(46, 43, &bytes_txt, rgba(120, 220, 255, 255), 1);

        // Alt hız çubuğu
        let rate_ratio = (self.last_bps as f32 / 65536.0).clamp(0.0, 1.0);
        self.canvas.draw_progress_bar(
            5,
            63,
            118,
            11,
            rate_ratio,
            rgba(60, 220, 140, 255),
            panel,
            rgba(60, 130, 100, 255),
        );
        let rate_txt = format!("{} B/S", self.last_bps);
        self.canvas.draw_text(10, 65, &rate_txt, white, 1);

        self.canvas.flush(self.fb);
    }
}

extern "C" fn on_cs_change(_user_data: *mut c_void, _pin: PinT, value: u32) {
    unsafe {
        if let Some(st) = STATE.as_mut() {
            st.cs_active = value == LOW;
            if value == HIGH {
                st.bit_count = 0;
            }
        }
    }
}

extern "C" fn on_sck_rising(_user_data: *mut c_void, _pin: PinT, _value: u32) {
    unsafe {
        if let Some(st) = STATE.as_mut() {
            if st.cs_active || pin_read(st.cs) == LOW {
                st.bit_count += 1;
                if st.bit_count >= 8 {
                    st.bit_count = 0;
                    st.total_bytes = st.total_bytes.saturating_add(1);
                    st.window_bytes = st.window_bytes.saturating_add(1);
                    st.act_hold_ticks = 4;
                }
            }
        }
    }
}

extern "C" fn on_ui_timer(_user_data: *mut c_void) {
    unsafe {
        if let Some(st) = STATE.as_mut() {
            st.tick_counter = st.tick_counter.wrapping_add(1);
            if st.act_hold_ticks > 0 {
                st.act_hold_ticks -= 1;
            }
            // 20 Hz timer -> her 5 tikte (250ms) bayt/sn tahminini güncelle
            if st.tick_counter % 5 == 0 {
                st.last_bps = st.window_bytes.saturating_mul(4);
                st.window_bytes = 0;
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
    let vcc = pin_init(b"VCC\0", INPUT);
    let _gnd = pin_init(b"GND\0", INPUT);
    let cs = pin_init(b"CS\0", INPUT_PULLUP);
    let sck = pin_init(b"SCK\0", INPUT);
    let _mosi = pin_init(b"MOSI\0", INPUT);
    let _miso = pin_init(b"MISO\0", INPUT);
    let cd = pin_init(b"CD\0", OUTPUT_HIGH);

    let attr_card_present = attr_init(b"card_present\0", 1);
    let (fb, w, h) = framebuffer_init();
    let canvas = Canvas::new(w.max(128), h.max(80));

    unsafe {
        STATE = Some(SdBoardState {
            vcc,
            cs,
            sck,
            cd,
            attr_card_present,
            fb,
            canvas,
            cs_active: false,
            bit_count: 0,
            total_bytes: 0,
            window_bytes: 0,
            last_bps: 0,
            act_hold_ticks: 0,
            tick_counter: 0,
        });
    }

    let cs_watch = PinWatchConfig {
        user_data: core::ptr::null_mut(),
        edge: BOTH,
        pin_change: Some(on_cs_change),
    };
    pin_watch(cs, &cs_watch);

    let sck_watch = PinWatchConfig {
        user_data: core::ptr::null_mut(),
        edge: RISING,
        pin_change: Some(on_sck_rising),
    };
    pin_watch(sck, &sck_watch);

    let timer_cfg = TimerConfig {
        user_data: core::ptr::null_mut(),
        callback: Some(on_ui_timer),
        reserved: [0; 8],
    };
    let t = timer_init(&timer_cfg);
    timer_start(t, 50_000, true); // 20 Hz UI yenileme
}
