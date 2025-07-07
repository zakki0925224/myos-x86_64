use crate::{arch::addr::IoPortAddress, warn};

pub const EXIT_SUCCESS: u32 = 0x10;
pub const EXIT_FAILURE: u32 = 0x11;

pub fn exit(exit_code: u32) {
    // ISA debug exit
    IoPortAddress::new(0xf4).out32(exit_code);

    // if QEMU, unreachable
    warn!("Failed to exit QEMU");
}
