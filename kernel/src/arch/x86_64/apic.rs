use crate::{arch::VirtualAddress, sync::volatile::Volatile, util::mmio::Mmio};

pub fn local_apic_id() -> u8 {
    let reg: Mmio<Volatile<u32>> =
        unsafe { Mmio::from_raw(VirtualAddress::new(0xfee00020).as_ptr_mut()) };
    (reg.as_ref().read() >> 24) as u8
}

pub fn notify_end_of_int() {
    let mut reg: Mmio<Volatile<u32>> =
        unsafe { Mmio::from_raw(VirtualAddress::new(0xfee000b0).as_ptr_mut()) };
    unsafe {
        reg.get_unchecked_mut().write(0);
    }
}
