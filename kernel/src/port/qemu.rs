use crate::arch::IoPortAddress;

pub const EXIT_SUCCESS: u32 = 0x10;
pub const EXIT_FAILURE: u32 = 0x11;

pub fn exit(exit_code: u32) -> ! {
    // ISA debug exit
    IoPortAddress::new(0xf4).out32(exit_code);
    unreachable!()
}

pub fn poweroff() {
    IoPortAddress::new(0x604).out16(0x2000);

    // for old QEMU and bochs
    IoPortAddress::new(0xb004).out16(0x2000);
}
