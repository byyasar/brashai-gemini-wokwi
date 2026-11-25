//! Wokwi Custom Chips API (v1) ABI katmanı ve test stub'ları.
//! Resmi wokwi-api.h ile birebir uyumlu #[repr(C)] yapılar ve FFI imzaları içerir.

pub mod gfx;

use core::ffi::c_void;

pub type PinT = i32;
pub type SpiDevT = u32;
pub type TimerT = u32;
pub type BufferT = u32;
pub type AttrT = u32;

pub const NO_PIN: PinT = -1;

// Pin mantıksal değerleri
pub const LOW: u32 = 0;
pub const HIGH: u32 = 1;

// Pin modları
pub const INPUT: u32 = 0;
pub const OUTPUT: u32 = 1;
pub const INPUT_PULLUP: u32 = 2;
pub const INPUT_PULLDOWN: u32 = 3;
pub const ANALOG: u32 = 4;
pub const OUTPUT_LOW: u32 = 16;
pub const OUTPUT_HIGH: u32 = 17;

// Pin izleme kenarları
pub const RISING: u32 = 1;
pub const FALLING: u32 = 2;
pub const BOTH: u32 = 3;

#[repr(C)]
pub struct PinWatchConfig {
    pub user_data: *mut c_void,
    pub edge: u32,
    pub pin_change: Option<extern "C" fn(user_data: *mut c_void, pin: PinT, value: u32)>,
}

#[repr(C)]
pub struct SpiConfig {
    pub user_data: *mut c_void,
    pub sck: PinT,
    pub mosi: PinT,
    pub miso: PinT,
    pub mode: u32,
    pub done: Option<extern "C" fn(user_data: *mut c_void, buffer: *mut u8, count: u32)>,
    pub reserved: [u32; 8],
}

#[repr(C)]
pub struct TimerConfig {
    pub user_data: *mut c_void,
    pub callback: Option<extern "C" fn(user_data: *mut c_void)>,
    pub reserved: [u32; 8],
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn __wokwi_api_version_1() -> i32 {
    1
}

#[cfg(target_arch = "wasm32")]
mod ffi {
    use super::*;

    #[link(wasm_import_module = "env")]
    extern "C" {
        pub fn pinInit(name: *const u8, mode: u32) -> PinT;
        pub fn pinRead(pin: PinT) -> u32;
        pub fn pinWrite(pin: PinT, value: u32);
        pub fn pinWatch(pin: PinT, config: *const PinWatchConfig) -> bool;
        pub fn pinWatchStop(pin: PinT);
        pub fn pinMode(pin: PinT, value: u32);
        pub fn pinADCRead(pin: PinT) -> f32;
        pub fn pinDACWrite(pin: PinT, voltage: f32);

        pub fn attrInit(name: *const u8, default_value: f64) -> AttrT;
        pub fn attrRead(attr_id: AttrT) -> u32;
        pub fn attrReadFloat(attr_id: AttrT) -> f64;

        pub fn spiInit(spi_config: *const SpiConfig) -> SpiDevT;
        pub fn spiStart(spi: SpiDevT, buffer: *mut u8, count: u32);
        pub fn spiStop(spi: SpiDevT);

        pub fn timerInit(config: *const TimerConfig) -> TimerT;
        pub fn timerStart(timer: TimerT, micros: u32, repeat: bool);
        pub fn timerStartNanos(timer: TimerT, nanos: f64, repeat: bool);
        pub fn timerStop(timer: TimerT);
        pub fn getSimNanos() -> f64;

        pub fn framebufferInit(pixel_width: *mut u32, pixel_height: *mut u32) -> BufferT;
        pub fn bufferRead(buffer: BufferT, offset: u32, data: *mut u8, data_len: u32) -> u32;
        pub fn bufferWrite(buffer: BufferT, offset: u32, data: *const u8, data_len: u32) -> u32;
        pub fn debugPrint(message: *const u8);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(non_snake_case)]
mod ffi {
    use super::*;

    pub unsafe fn pinInit(_name: *const u8, _mode: u32) -> PinT { 1 }
    pub unsafe fn pinRead(_pin: PinT) -> u32 { 0 }
    pub unsafe fn pinWrite(_pin: PinT, _value: u32) {}
    pub unsafe fn pinWatch(_pin: PinT, _config: *const PinWatchConfig) -> bool { true }
    pub unsafe fn pinWatchStop(_pin: PinT) {}
    pub unsafe fn pinMode(_pin: PinT, _value: u32) {}
    pub unsafe fn pinADCRead(_pin: PinT) -> f32 { 0.0 }
    pub unsafe fn pinDACWrite(_pin: PinT, _voltage: f32) {}

    pub unsafe fn attrInit(_name: *const u8, _default_value: f64) -> AttrT { 1 }
    pub unsafe fn attrRead(_attr_id: AttrT) -> u32 { 0 }
    pub unsafe fn attrReadFloat(_attr_id: AttrT) -> f64 { 0.0 }

    pub unsafe fn spiInit(_spi_config: *const SpiConfig) -> SpiDevT { 1 }
    pub unsafe fn spiStart(_spi: SpiDevT, _buffer: *mut u8, _count: u32) {}
    pub unsafe fn spiStop(_spi: SpiDevT) {}

    pub unsafe fn timerInit(_config: *const TimerConfig) -> TimerT { 1 }
    pub unsafe fn timerStart(_timer: TimerT, _micros: u32, _repeat: bool) {}
    pub unsafe fn timerStartNanos(_timer: TimerT, _nanos: f64, _repeat: bool) {}
    pub unsafe fn timerStop(_timer: TimerT) {}
    pub unsafe fn getSimNanos() -> f64 { 0.0 }

    pub unsafe fn framebufferInit(pixel_width: *mut u32, pixel_height: *mut u32) -> BufferT {
        if !pixel_width.is_null() { *pixel_width = 160; }
        if !pixel_height.is_null() { *pixel_height = 96; }
        1
    }
    pub unsafe fn bufferRead(_buffer: BufferT, _offset: u32, _data: *mut u8, _data_len: u32) -> u32 { 0 }
    pub unsafe fn bufferWrite(_buffer: BufferT, _offset: u32, _data: *const u8, _data_len: u32) -> u32 { 0 }
    pub unsafe fn debugPrint(_message: *const u8) {}
}

/// Null-sonlu bayt dilimi (`b"PIN\0"`) ile pin başlatır.
#[inline]
pub fn pin_init(name_cstr: &[u8], mode: u32) -> PinT {
    debug_assert!(name_cstr.ends_with(&[0]));
    unsafe { ffi::pinInit(name_cstr.as_ptr(), mode) }
}

#[inline]
pub fn pin_read(pin: PinT) -> u32 {
    unsafe { ffi::pinRead(pin) }
}

#[inline]
pub fn pin_write(pin: PinT, value: u32) {
    unsafe { ffi::pinWrite(pin, value) }
}

#[inline]
pub fn pin_watch(pin: PinT, config: &PinWatchConfig) -> bool {
    unsafe { ffi::pinWatch(pin, config as *const PinWatchConfig) }
}

#[inline]
pub fn pin_watch_stop(pin: PinT) {
    unsafe { ffi::pinWatchStop(pin) }
}

#[inline]
pub fn pin_mode(pin: PinT, mode: u32) {
    unsafe { ffi::pinMode(pin, mode) }
}

#[inline]
pub fn pin_adc_read(pin: PinT) -> f32 {
    unsafe { ffi::pinADCRead(pin) }
}

#[inline]
pub fn pin_dac_write(pin: PinT, voltage: f32) {
    unsafe { ffi::pinDACWrite(pin, voltage) }
}

#[inline]
pub fn attr_init(name_cstr: &[u8], default_value: u32) -> AttrT {
    debug_assert!(name_cstr.ends_with(&[0]));
    unsafe { ffi::attrInit(name_cstr.as_ptr(), default_value as f64) }
}

#[inline]
pub fn attr_init_float(name_cstr: &[u8], default_value: f32) -> AttrT {
    debug_assert!(name_cstr.ends_with(&[0]));
    unsafe { ffi::attrInit(name_cstr.as_ptr(), default_value as f64) }
}

#[inline]
pub fn attr_read(attr_id: AttrT) -> u32 {
    unsafe { ffi::attrRead(attr_id) }
}

#[inline]
pub fn attr_read_float(attr_id: AttrT) -> f32 {
    unsafe { ffi::attrReadFloat(attr_id) as f32 }
}

#[inline]
pub fn spi_init(config: &SpiConfig) -> SpiDevT {
    unsafe { ffi::spiInit(config as *const SpiConfig) }
}

#[inline]
pub fn spi_start(spi: SpiDevT, buffer: &mut [u8]) {
    unsafe { ffi::spiStart(spi, buffer.as_mut_ptr(), buffer.len() as u32) }
}

#[inline]
pub fn spi_start_len(spi: SpiDevT, buffer_ptr: *mut u8, count: u32) {
    unsafe { ffi::spiStart(spi, buffer_ptr, count) }
}

#[inline]
pub fn spi_stop(spi: SpiDevT) {
    unsafe { ffi::spiStop(spi) }
}

#[inline]
pub fn timer_init(config: &TimerConfig) -> TimerT {
    unsafe { ffi::timerInit(config as *const TimerConfig) }
}

#[inline]
pub fn timer_start(timer: TimerT, micros: u32, repeat: bool) {
    unsafe { ffi::timerStart(timer, micros, repeat) }
}

#[inline]
pub fn timer_start_ns(timer: TimerT, nanos: u64, repeat: bool) {
    unsafe { ffi::timerStartNanos(timer, nanos as f64, repeat) }
}

#[inline]
pub fn timer_stop(timer: TimerT) {
    unsafe { ffi::timerStop(timer) }
}

#[inline]
pub fn get_sim_nanos() -> u64 {
    unsafe { ffi::getSimNanos() as u64 }
}

#[inline]
pub fn framebuffer_init() -> (BufferT, u32, u32) {
    let mut w: u32 = 0;
    let mut h: u32 = 0;
    let buf = unsafe { ffi::framebufferInit(&mut w as *mut u32, &mut h as *mut u32) };
    (buf, w, h)
}

#[inline]
pub fn buffer_write(buffer: BufferT, offset: u32, data: &[u8]) {
    unsafe {
        let _ = ffi::bufferWrite(buffer, offset, data.as_ptr(), data.len() as u32);
    }
}

#[inline]
pub fn buffer_read(buffer: BufferT, offset: u32, data: &mut [u8]) {
    unsafe {
        let _ = ffi::bufferRead(buffer, offset, data.as_mut_ptr(), data.len() as u32);
    }
}

#[inline]
pub fn debug_print(msg_cstr: &[u8]) {
    debug_assert!(msg_cstr.ends_with(&[0]));
    unsafe { ffi::debugPrint(msg_cstr.as_ptr()) }
}
