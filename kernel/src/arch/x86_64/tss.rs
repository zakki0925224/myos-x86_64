use crate::{
    error::Result,
    mem::{self, paging::PAGE_SIZE},
};
use core::mem::size_of;

static mut TSS: TaskStateSegment = TaskStateSegment::new();

#[derive(Debug, Clone, Copy)]
#[repr(packed)]
pub struct TaskStateSegmentDescriptor {
    limit_low: u16,
    base_low: u16,
    base_mid_low: u8,
    attr: u16,
    base_mid_high: u8,
    base_high: u32,
    _reserved: u32,
}

impl TaskStateSegmentDescriptor {
    pub const fn new() -> Self {
        Self {
            limit_low: 0,
            base_low: 0,
            base_mid_low: 0,
            attr: 0,
            base_mid_high: 0,
            base_high: 0,
            _reserved: 0,
        }
    }

    pub fn set(&mut self, base: u64) {
        self.set_base(base);
        self.limit_low = size_of::<Self>() as u16 - 1;
        self.attr = 0b1000_0000_1000_1001;
    }

    pub fn set_base(&mut self, base: u64) {
        self.base_low = base as u16;
        self.base_mid_low = (base >> 16) as u8;
        self.base_mid_high = (base >> 24) as u8;
        self.base_high = (base >> 32) as u32;
    }
}

#[repr(packed)]
struct TaskStateSegment {
    reserved0: u32,
    rsp: [u64; 3],
    ist: [u64; 8],
    reserved1: [u16; 5],
    io_map_base_addr: u16,
}

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp: [0; 3],
            ist: [0; 8],
            reserved1: [0; 5],
            io_map_base_addr: 0,
        }
    }

    pub fn init(&mut self) -> Result<()> {
        let frame_len = 8;

        let rsp0 = mem::bitmap::alloc_mem_frame(frame_len)?
            .frame_start_virt_addr()?
            .offset(frame_len * PAGE_SIZE)
            .get();
        self.rsp[0] = rsp0;

        Ok(())
    }
}

// return tss addr
pub fn init() -> Result<u64> {
    unsafe {
        TSS.init()?;
        Ok((&TSS as *const _) as u64)
    }
}
