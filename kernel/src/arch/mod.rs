use crate::{arch::x86_64::paging, error::Result, mem::paging::PageError};
use core::fmt::Debug;

pub mod x86_64;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct VirtualAddress(u64);

impl Debug for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VirtualAddress({:#x})", self.0)
    }
}

impl From<u64> for VirtualAddress {
    fn from(addr: u64) -> Self {
        Self::new(addr)
    }
}

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

    pub unsafe fn phys_addr(&self) -> Result<u64> {
        let page_table = &*paging::kernel_page_table();
        paging::calc_phys_addr(page_table, *self)
            .ok_or(PageError::AddressNotMapped(self.get()).into())
    }

    pub fn pml4_entry_index(&self) -> usize {
        ((self.0 >> 39) & 0x1ff) as usize
    }

    pub fn pml3_entry_index(&self) -> usize {
        ((self.0 >> 30) & 0x1ff) as usize
    }

    pub fn pml2_entry_index(&self) -> usize {
        ((self.0 >> 21) & 0x1ff) as usize
    }

    pub fn pml1_entry_index(&self) -> usize {
        ((self.0 >> 12) & 0x1ff) as usize
    }

    pub fn page_offset(&self) -> usize {
        (self.0 & 0xfff) as usize
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.get() as *const T
    }

    pub fn as_ptr_mut<T>(&self) -> *mut T {
        self.get() as *mut T
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct IoPortAddress(u32);

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

impl Debug for IoPortAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "IoPortAddress({:#x})", self.0)
    }
}

impl IoPortAddress {
    pub const fn new(addr: u32) -> Self {
        Self(addr)
    }

    pub fn offset(&self, offset: usize) -> Self {
        Self::new(self.0 + offset as u32)
    }

    pub fn out8(&self, value: u8) {
        assert!(self.0 <= u16::MAX as u32);
        x86_64::out8(self.0 as u16, value);
    }

    pub fn in8(&self) -> u8 {
        assert!(self.0 <= u16::MAX as u32);
        x86_64::in8(self.0 as u16)
    }

    pub fn out16(&self, value: u16) {
        assert!(self.0 <= u16::MAX as u32);
        x86_64::out16(self.0 as u16, value);
    }

    pub fn in16(&self) -> u16 {
        assert!(self.0 <= u16::MAX as u32);
        x86_64::in16(self.0 as u16)
    }

    pub fn out32(&self, value: u32) {
        x86_64::out32(self.0, value);
    }

    pub fn in32(&self) -> u32 {
        x86_64::in32(self.0)
    }
}
