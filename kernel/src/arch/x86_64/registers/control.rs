use super::Register;
use core::arch::asm;

// https://en.wikipedia.org/wiki/Control_register
#[derive(Debug, Clone, Copy)]
pub struct Cr0(u64);
impl Register<u64> for Cr0 {
    fn read() -> Self {
        let cr0;

        unsafe {
            asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack));
        }

        Self(cr0)
    }

    fn write(&self) {
        unsafe {
            asm!("mov cr0, {}", in(reg) self.0, options(nomem, nostack));
        }
    }

    fn raw(&self) -> u64 {
        self.0
    }

    fn set_raw(&mut self, value: u64) {
        self.0 = value;
    }
}

impl Cr0 {
    pub fn set_paging(&mut self, value: bool) {
        self.0 = (self.0 & !0x8000_0000) | ((value as u64) << 31);
    }

    pub fn set_emulation(&mut self, value: bool) {
        self.0 = (self.0 & !0x04) | ((value as u64) << 2);
    }

    pub fn set_monitor_coprocessor(&mut self, value: bool) {
        self.0 = (self.0 & !0x02) | ((value as u64) << 1);
    }

    pub fn paging(&self) -> bool {
        (self.0 & 0x8000_0000) != 0
    }

    pub fn emulation(&self) -> bool {
        (self.0 & 0x04) != 0
    }

    pub fn monitor_coprocessor(&self) -> bool {
        (self.0 & 0x02) != 0
    }
}

pub struct Cr2(u64);

impl Register<u64> for Cr2 {
    fn read() -> Self {
        let cr2;
        unsafe {
            asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack));
        }

        Self(cr2)
    }

    fn write(&self) {
        unsafe {
            asm!("mov cr2, {}", in(reg) self.0, options(nomem, nostack));
        }
    }

    fn raw(&self) -> u64 {
        self.0
    }

    fn set_raw(&mut self, value: u64) {
        self.0 = value;
    }
}

#[derive(Debug)]
pub struct Cr3(u64);

impl Register<u64> for Cr3 {
    fn read() -> Self {
        let cr3;
        unsafe {
            asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack));
        }

        Self(cr3)
    }

    fn write(&self) {
        unsafe {
            asm!("mov cr3, {}", in(reg) self.0, options(nomem, nostack));
        }
    }

    fn raw(&self) -> u64 {
        self.0
    }

    fn set_raw(&mut self, value: u64) {
        self.0 = value;
    }
}

#[derive(Debug)]
pub struct Cr4(u64);

impl Register<u64> for Cr4 {
    fn read() -> Self {
        let cr4;
        unsafe {
            asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack));
        }

        Self(cr4)
    }

    fn write(&self) {
        unsafe {
            asm!("mov cr4, {}", in(reg) self.0, options(nomem, nostack));
        }
    }

    fn raw(&self) -> u64 {
        self.0
    }

    fn set_raw(&mut self, value: u64) {
        self.0 = value;
    }
}

impl Cr4 {
    pub fn set_osfxsr(&mut self, value: bool) {
        self.0 = (self.0 & !0x200) | ((value as u64) << 9);
    }

    pub fn set_osxmmexcept(&mut self, value: bool) {
        self.0 = (self.0 & !0x400) | ((value as u64) << 10);
    }

    pub fn osfxsr(&self) -> bool {
        (self.0 & 0x200) != 0
    }

    pub fn osxmmexcept(&self) -> bool {
        (self.0 & 0x400) != 0
    }
}
