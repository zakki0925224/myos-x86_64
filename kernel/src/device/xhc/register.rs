use crate::arch::volatile::Volatile;
use core::marker::PhantomPinned;

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
}

#[repr(C, align(64))]
pub struct DeviceContextBaseAddressArray {
    ctx: [u64; 256],
    _pinned: PhantomPinned,
}

#[repr(C)]
pub struct OperationalRegisters {
    pub usb_cmd: Volatile<u32>,
    pub usb_status: Volatile<u32>,
    pub page_size: Volatile<u32>,
    reserved0: [u32; 2],
    pub dn_ctrl: Volatile<u32>,
    pub cmd_ring_ctrl: Volatile<u64>,
    reserved1: [u64; 2],
    pub dev_ctx_baa_ptr: Volatile<*mut DeviceContextBaseAddressArray>,
    pub config: Volatile<u64>,
}

#[repr(C)]
struct InterrupterRegisterSet([u64; 4]);

#[repr(C)]
pub struct RuntimeRegisters {
    pub mfindex: Volatile<u32>,
    reserved: [u32; 7],
    pub int_reg_set: [InterrupterRegisterSet; 1024],
}
