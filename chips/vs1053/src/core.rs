//! VLSI VS1053b Datasheet uyumlu SCI register dosyası, WRAM ve SDI FIFO çekirdeği.

use std::collections::VecDeque;

// SCI Register Adresleri (VS1053b Datasheet Bölüm 8.6)
pub const SCI_MODE: u8 = 0x00;
pub const SCI_STATUS: u8 = 0x01;
pub const SCI_BASS: u8 = 0x02;
pub const SCI_CLOCKF: u8 = 0x03;
pub const SCI_DECODE_TIME: u8 = 0x04;
pub const SCI_AUDATA: u8 = 0x05;
pub const SCI_WRAM: u8 = 0x06;
pub const SCI_WRAMADDR: u8 = 0x07;
pub const SCI_HDAT0: u8 = 0x08;
pub const SCI_HDAT1: u8 = 0x09;
pub const SCI_AIADDR: u8 = 0x0A;
pub const SCI_VOL: u8 = 0x0B;
pub const SCI_AICTRL0: u8 = 0x0C;
pub const SCI_AICTRL1: u8 = 0x0D;
pub const SCI_AICTRL2: u8 = 0x0E;
pub const SCI_AICTRL3: u8 = 0x0F;

// SCI_MODE bit maskeleri
pub const SM_DIFF: u16 = 0x0001;
pub const SM_LAYER12: u16 = 0x0002;
pub const SM_RESET: u16 = 0x0004;
pub const SM_CANCEL: u16 = 0x0008;
pub const SM_EARSPEAKER_LO: u16 = 0x0010;
pub const SM_TESTS: u16 = 0x0020;
pub const SM_STREAM: u16 = 0x0040;
pub const SM_EARSPEAKER_HI: u16 = 0x0080;
pub const SM_DACT: u16 = 0x0100;
pub const SM_SDIORD: u16 = 0x0200;
pub const SM_SDISHARE: u16 = 0x0400;
pub const SM_SDINEW: u16 = 0x0800;
pub const SM_ADPCM: u16 = 0x1000;
pub const SM_LINE1: u16 = 0x4000;
pub const SM_CLK_RANGE: u16 = 0x8000;

/// Genişletilmiş SDI FIFO kapasitesi (veri ezilmesini önler, DREQ ile akış kontrol edilir)
pub const SDI_FIFO_CAPACITY: usize = 8192;

/// VS1053b SCI_STATUS: SS_VER = 4 (bits 7:4 => 0x0040), varsayılan 0x0048
pub const DEFAULT_SCI_STATUS: u16 = 0x0048;
/// Varsayılan SCI_MODE: SM_SDINEW (0x0800)
pub const DEFAULT_SCI_MODE: u16 = SM_SDINEW;

pub struct Vs1053Regs {
    pub mode: u16,
    pub status: u16,
    pub bass: u16,
    pub clockf: u16,
    pub decode_time: u16,
    pub audata: u16,
    pub wram_addr: u16,
    pub hdat0: u16,
    pub hdat1: u16,
    pub aiaddr: u16,
    pub vol: u16,
    pub aictrl: [u16; 4],

    // Dahili durum
    pub end_fill_byte: u16,
    pub gpio_ddr: u16,
    pub gpio_odata: u16,
    pub wram_patch: Vec<(u16, u16)>,
    pub sdi_fifo: VecDeque<u8>,
    pub reset_busy_ticks: u8,
    pub cancel_pending: bool,
    pub reset_pending: bool,
    pub last_decode_time_write: bool,
    pub total_sci_reads: u32,
    pub total_sci_writes: u32,
    pub total_sdi_bytes: u64,
}

impl Vs1053Regs {
    pub fn new() -> Self {
        Self {
            mode: DEFAULT_SCI_MODE,
            status: DEFAULT_SCI_STATUS,
            bass: 0x0000,
            clockf: 0x0000,
            decode_time: 0x0000,
            audata: 44101,
            wram_addr: 0x0000,
            hdat0: 0x0000,
            hdat1: 0x0000,
            aiaddr: 0x0000,
            vol: 0x0000,
            aictrl: [0; 4],
            end_fill_byte: 0x0000,
            gpio_ddr: 0x0000,
            gpio_odata: 0x0000,
            wram_patch: Vec::new(),
            sdi_fifo: VecDeque::with_capacity(SDI_FIFO_CAPACITY),
            reset_busy_ticks: 0,
            cancel_pending: false,
            reset_pending: false,
            last_decode_time_write: false,
            total_sci_reads: 0,
            total_sci_writes: 0,
            total_sdi_bytes: 0,
        }
    }

    /// Donanım reseti (XRST LOW->HIGH).
    pub fn hard_reset(&mut self) {
        self.mode = DEFAULT_SCI_MODE;
        self.status = DEFAULT_SCI_STATUS;
        self.bass = 0x0000;
        self.clockf = 0x0000;
        self.decode_time = 0x0000;
        self.audata = 44101;
        self.wram_addr = 0x0000;
        self.hdat0 = 0x0000;
        self.hdat1 = 0x0000;
        self.aiaddr = 0x0000;
        self.vol = 0x0000;
        self.aictrl = [0; 4];
        self.end_fill_byte = 0x0000;
        self.gpio_ddr = 0x0000;
        self.gpio_odata = 0x0000;
        self.wram_patch.clear();
        self.sdi_fifo.clear();
        self.reset_busy_ticks = 0;
        self.cancel_pending = false;
        self.reset_pending = true;
        self.last_decode_time_write = false;
    }

    /// Yazılımsal reset (SM_RESET).
    pub fn soft_reset(&mut self) {
        self.mode = (self.mode & !SM_RESET & !SM_CANCEL) | SM_SDINEW;
        self.decode_time = 0;
        self.hdat0 = 0;
        self.hdat1 = 0;
        self.sdi_fifo.clear();
        self.cancel_pending = false;
        self.reset_pending = true;
        self.reset_busy_ticks = 0;
        self.last_decode_time_write = false;
    }

    #[inline]
    pub fn fifo_free(&self) -> usize {
        SDI_FIFO_CAPACITY.saturating_sub(self.sdi_fifo.len())
    }

    /// Datasheet: FIFO'da en az 32 bayt boş yer varsa ve reset meşgul değilse DREQ = HIGH.
    #[inline]
    pub fn is_dreq_ready(&self) -> bool {
        self.reset_busy_ticks == 0 && self.fifo_free() >= 64
    }

    /// Hem SDI FIFO hem de PCM çıkış tamponunu dikkate alarak gerçek zamanlı akış kontrolü yapar.
    /// PCM tamponunda ~1.5 saniyelik ses biriktiğinde DREQ LOW çekilerek ESP32'nin dosyayı
    /// gerçek çalma hızında beslemesi sağlanır.
    #[inline]
    pub fn is_dreq_ready_for_stream(&self, pcm_ring_len: usize) -> bool {
        self.reset_busy_ticks == 0 && self.fifo_free() >= 256 && pcm_ring_len < 33_000
    }

    pub fn read_reg(&mut self, addr: u8) -> u16 {
        self.total_sci_reads = self.total_sci_reads.saturating_add(1);
        self.last_decode_time_write = false;
        match addr & 0x0F {
            SCI_MODE => self.mode,
            SCI_STATUS => (self.status & !0x00F0) | 0x0040, // SS_VER = 4 garanti
            SCI_BASS => self.bass,
            SCI_CLOCKF => self.clockf,
            SCI_DECODE_TIME => self.decode_time,
            SCI_AUDATA => self.audata,
            SCI_WRAM => {
                let val = self.read_wram(self.wram_addr);
                self.wram_addr = self.wram_addr.wrapping_add(1);
                val
            }
            SCI_WRAMADDR => self.wram_addr,
            SCI_HDAT0 => self.hdat0,
            SCI_HDAT1 => self.hdat1,
            SCI_AIADDR => self.aiaddr,
            SCI_VOL => self.vol,
            SCI_AICTRL0 => self.aictrl[0],
            SCI_AICTRL1 => self.aictrl[1],
            SCI_AICTRL2 => self.aictrl[2],
            SCI_AICTRL3 => self.aictrl[3],
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, addr: u8, val: u16) {
        self.total_sci_writes = self.total_sci_writes.saturating_add(1);
        let reg = addr & 0x0F;
        if reg != SCI_DECODE_TIME {
            self.last_decode_time_write = false;
        }
        match reg {
            SCI_MODE => {
                let want_reset = (val & SM_RESET) != 0;
                let want_cancel = (val & SM_CANCEL) != 0;
                self.mode = val & !SM_RESET;
                if want_reset {
                    self.soft_reset();
                } else if want_cancel {
                    // SM_CANCEL: biti hemen temizle ve FIFO'yu boşalt ki ESP32 stopSong() beklemesin
                    self.hdat0 = 0;
                    self.hdat1 = 0;
                    self.cancel_pending = false;
                    self.mode &= !SM_CANCEL;
                    self.sdi_fifo.clear();
                }
            }
            SCI_STATUS => {
                self.status = (val & !0x00F0) | 0x0040;
            }
            SCI_BASS => self.bass = val,
            SCI_CLOCKF => self.clockf = val,
            SCI_DECODE_TIME => {
                if self.last_decode_time_write {
                    self.decode_time = 0;
                    self.last_decode_time_write = false;
                } else {
                    self.decode_time = val;
                    self.last_decode_time_write = true;
                }
            }
            SCI_AUDATA => self.audata = val,
            SCI_WRAM => {
                self.write_wram(self.wram_addr, val);
                self.wram_addr = self.wram_addr.wrapping_add(1);
            }
            SCI_WRAMADDR => self.wram_addr = val,
            SCI_AIADDR => self.aiaddr = val,
            SCI_VOL => self.vol = val,
            SCI_AICTRL0 => self.aictrl[0] = val,
            SCI_AICTRL1 => self.aictrl[1] = val,
            SCI_AICTRL2 => self.aictrl[2] = val,
            SCI_AICTRL3 => self.aictrl[3] = val,
            _ => {}
        }
    }

    fn read_wram(&self, addr: u16) -> u16 {
        match addr {
            0x1E06 => self.end_fill_byte,
            0xC017 => self.gpio_ddr,
            0xC019 => self.gpio_odata,
            _ => {
                for &(a, v) in self.wram_patch.iter().rev() {
                    if a == addr {
                        return v;
                    }
                }
                0x0000
            }
        }
    }

    fn write_wram(&mut self, addr: u16, val: u16) {
        match addr {
            0x1E06 => self.end_fill_byte = val,
            0xC017 => self.gpio_ddr = val,
            0xC019 => self.gpio_odata = val,
            _ => {
                if let Some(entry) = self.wram_patch.iter_mut().find(|(a, _)| *a == addr) {
                    entry.1 = val;
                } else if self.wram_patch.len() < 256 {
                    self.wram_patch.push((addr, val));
                }
            }
        }
    }

    pub fn push_sdi(&mut self, data: &[u8]) {
        self.total_sdi_bytes = self.total_sdi_bytes.saturating_add(data.len() as u64);
        for &b in data {
            self.sdi_fifo.push_back(b);
        }
    }

    /// SCI_VOL (0.5 dB/adım) değerinden (sol_kazanç, sağ_kazanç) [0.0..1.0] üretir.
    pub fn volume_gains(&self) -> (f32, f32) {
        let left_att = (self.vol >> 8) as u8;
        let right_att = (self.vol & 0xFF) as u8;
        (att_to_gain(left_att), att_to_gain(right_att))
    }
}

#[inline]
pub fn att_to_gain(att: u8) -> f32 {
    if att >= 0xFE {
        0.0
    } else if att == 0 {
        1.0
    } else {
        10.0_f32.powf(-(att as f32) / 40.0)
    }
}
