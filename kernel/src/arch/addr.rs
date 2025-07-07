use crate::{arch, error::Result, mem::paging};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    pub fn new(addr: u64) -> Self {
        Self(addr)
    }

    pub fn get(&self) -> u64 {
        self.0
    }

    pub fn set(&mut self, addr: u64) {
        self.0 = addr;
    }

    pub fn offset(&self, offset: usize) -> Self {
        Self::new(self.0 + offset as u64)
    }

    pub fn get_virt_addr(&self) -> Result<VirtualAddress> {
        paging::calc_virt_addr(*self)
    }
}

impl From<u64> for PhysicalAddress {
    fn from(addr: u64) -> Self {
        Self::new(addr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct VirtualAddress(u64);

impl VirtualAddress {
    pub fn new(addr: u64) -> Self {
        Self(addr)
    }

    pub fn get(&self) -> u64 {
        self.0
    }

    pub fn set(&mut self, addr: u64) {
        self.0 = addr;
    }

    pub fn offset(&self, offset: usize) -> Self {
        Self::new(self.0 + offset as u64)
    }

    pub fn get_phys_addr(&self) -> Result<PhysicalAddress> {
        paging::calc_phys_addr(*self)
    }

    pub fn get_pml4_entry_index(&self) -> usize {
        ((self.0 >> 39) & 0x1ff) as usize
    }

    pub fn get_pml3_entry_index(&self) -> usize {
        ((self.0 >> 30) & 0x1ff) as usize
    }

    pub fn get_pml2_entry_index(&self) -> usize {
        ((self.0 >> 21) & 0x1ff) as usize
    }

    pub fn get_pml1_entry_index(&self) -> usize {
        ((self.0 >> 12) & 0x1ff) as usize
    }

    pub fn get_page_offset(&self) -> usize {
        (self.0 & 0xfff) as usize
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.get() as *const T
    }

    pub fn as_ptr_mut<T>(&self) -> *mut T {
        self.get() as *mut T
    }

    pub fn copy_from_nonoverlapping<T>(&self, src: *const T, count: usize) {
        unsafe { self.as_ptr_mut::<T>().copy_from_nonoverlapping(src, count) }
    }
}

impl From<u64> for VirtualAddress {
    fn from(addr: u64) -> Self {
        Self::new(addr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct IoPortAddress(u32);

impl IoPortAddress {
    pub const fn new(addr: u32) -> Self {
        Self(addr)
    }

    pub fn offset(&self, offset: usize) -> Self {
        Self::new(self.0 + offset as u32)
    }

    pub fn out8(&self, value: u8) {
        assert!(self.0 <= u16::MAX as u32);
        arch::out8(self.0 as u16, value);
    }

    pub fn in8(&self) -> u8 {
        assert!(self.0 <= u16::MAX as u32);
        arch::in8(self.0 as u16)
    }

    pub fn out16(&self, value: u16) {
        assert!(self.0 <= u16::MAX as u32);
        arch::out16(self.0 as u16, value);
    }

    pub fn in16(&self) -> u16 {
        assert!(self.0 <= u16::MAX as u32);
        arch::in16(self.0 as u16)
    }

    pub fn out32(&self, value: u32) {
        arch::out32(self.0, value);
    }

    pub fn in32(&self) -> u32 {
        arch::in32(self.0)
    }
}

impl From<u16> for IoPortAddress {
    fn from(addr: u16) -> Self {
        Self::new(addr as u32)
    }
}

impl From<u32> for IoPortAddress {
    fn from(addr: u32) -> Self {
        Self::new(addr)
    }
}
