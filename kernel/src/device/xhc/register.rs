use alloc::{boxed::Box, vec::Vec};
use crate::{arch::volatile::Volatile, device::xhc::context::OutputContext};
use core::{marker::PhantomPinned, mem::MaybeUninit, pin::Pin};

#[repr(C)]
pub struct CapabilityRegisters {
    pub cap_reg_len: Volatile<u8>,
    reserved: Volatile<u8>,
    pub interface_ver_num: Volatile<u16>,
    hcs_params1: Volatile<u32>,
    pub hcs_params2: Volatile<u32>,
    pub hcs_params3: Volatile<u32>,
    pub hcc_params1: Volatile<u32>,
    db_offset: Volatile<u32>,
    rts_offset: Volatile<u32>,
    pub hcc_params2: Volatile<u32>,
}

impl CapabilityRegisters {
    pub fn cap_reg_len(&self) -> usize {
        self.cap_reg_len.read() as usize
    }

    pub fn db_offset(&self) -> usize {
        self.db_offset.read() as usize
    }

    pub fn rts_offset(&self) -> usize {
        self.rts_offset.read() as usize
    }

    pub fn num_of_device_slots(&self) -> usize {
        let hcs_params1 = self.hcs_params1.read();
        (hcs_params1 & 0xff) as usize
    }

    pub fn num_of_ports(&self) -> usize {
        let hcs_params1 = self.hcs_params1.read();
        ((hcs_params1 >> 16) & 0xff) as usize
    }

    pub fn num_scratchpad_bufs(&self) -> usize {
        let hcs_params2 = self.hcs_params2.read();
        (((hcs_params2 & 0xf_8000) >> 16 ) | ((hcs_params2 & 0x7c00_0000) >> 26)) as usize
    }
}

#[repr(C, align(64))]
pub struct DeviceContextBaseAddressArrayInner {
    scratchpad_table_ptr: *const *const u8,
    context: [u64; 255],
    _pinned: PhantomPinned,
}

impl DeviceContextBaseAddressArrayInner {
    pub fn new() -> Self {
        unsafe { MaybeUninit::zeroed().assume_init() }
    }
}

pub struct DeviceContextBaseAddressArray {
    inner: Pin<Box<DeviceContextBaseAddressArrayInner>>,
    context: [Option<Pin<Box<OutputContext>>>; 255],
    scratchpad_bufs: ScratchpadBuffers,
}

impl DeviceContextBaseAddressArray {
    pub fn new(scratchpad_bufs: ScratchpadBuffers) -> Self {
        let mut inner = DeviceContextBaseAddressArrayInner::new();
        inner.scratchpad_table_ptr = scratchpad_bufs.table.as_ref().as_ptr();

        Self {
            inner: Box::pin(inner),
            context: unsafe { MaybeUninit::uninit().assume_init() },
            scratchpad_bufs,
        }
    }

    pub fn inner_mut_ptr(&self) -> *const DeviceContextBaseAddressArrayInner {
        self.inner.as_ref().get_ref()
    }
}

pub struct UsbCommandRegister(Volatile<u32>);

impl UsbCommandRegister {
    fn read(&self) -> u32 {
        self.0.read()
    }

    fn write(&mut self, value: u32) {
        self.0.write(value);
    }

    pub fn set_run_stop(&mut self, value: bool) {
        self.write((self.read() & !0x1) | (value as u32));
    }

    pub fn host_controller_reset(&self) -> bool {
        (self.read() & 0x2) != 0
    }

    pub fn set_host_controller_reset(&mut self, value: bool) {
        self.write((self.read() & !0x2) | ((value as u32) << 1));
    }

    pub fn set_intr_enable(&mut self, value: bool) {
        self.write((self.read() & !0x4) | ((value as u32) << 2));
    }
}

pub struct UsbStatusRegister(Volatile<u32>);

impl UsbStatusRegister {
    fn read(&self) -> u32 {
        self.0.read()
    }

    fn write(&mut self, value: u32) {
        self.0.write(value);
    }

    pub fn hchalted(&self) -> bool {
        (self.read() & 0x1) != 0
    }

    pub fn host_system_err(&self) -> bool {
        (self.read() & 0x4) != 0
    }

    pub fn set_host_system_err(&mut self, value: bool) {
        self.write((self.read() & !0x4) | ((value as u32) << 2));
    }

    pub fn event_int(&self) -> bool {
        (self.read() & 0x8) != 0
    }

    pub fn set_event_int(&mut self, value: bool) {
        self.write((self.read() & !0x8) | ((value as u32) << 3));
    }

    pub fn port_change_detect(&self) -> bool {
        (self.read() & 0x10) != 0
    }

    pub fn set_port_change_detect(&mut self, value: bool) {
        self.write((self.read() & !0x10) | ((value as u32) << 4));
    }

    pub fn save_restore_err(&self) -> bool {
        (self.read() & 0x400) != 0
    }

    pub fn set_save_restore_err(&mut self, value: bool) {
        self.write((self.read() & !0x400) | ((value as u32) << 10));
    }

    pub fn controller_not_ready(&self) -> bool {
        (self.read() & 0x800) != 0
    }

    pub fn host_controller_err(&self) -> bool {
        (self.read() & 0x1000) != 0
    }
}

#[repr(C)]
pub struct OperationalRegisters {
    pub usb_cmd: UsbCommandRegister,
    pub usb_status: UsbStatusRegister,
    pub page_size: Volatile<u32>,
    reserved0: [u32; 2],
    pub dn_ctrl: Volatile<u32>,
    pub cmd_ring_ctrl: Volatile<u64>,
    reserved1: [u64; 2],
    pub dcbaa_ptr: Volatile<*const DeviceContextBaseAddressArrayInner>,
    pub config: Volatile<u32>,
}

impl OperationalRegisters {
    pub fn set_max_device_slots_enabled(&mut self, value: u8) {
        self.config.write((self.config.read() & !0xff) | (value as u32));
    }
}

#[repr(C)]
struct InterrupterRegisterSet([u64; 4]);

#[repr(C)]
pub struct RuntimeRegisters {
    pub mfindex: Volatile<u32>,
    reserved: [u32; 7],
    pub int_reg_set: [InterrupterRegisterSet; 1024],
}

pub struct ScratchpadBuffers {
    pub table: Pin<Box<[*const u8]>>,
    pub bufs: Vec<Pin<Box<[u8]>>>,
}

#[repr(C, align(4096))]
pub struct EventRingSegmentTableEntry {
    pub ring_seg_base_addr: u64,
    pub ring_seg_size: u16,
    reserved: [u16; 3],
}
