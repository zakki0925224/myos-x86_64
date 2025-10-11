use crate::{
    arch::x86_64,
    debug::qemu::{self, EXIT_FAILURE},
    device::panic_screen,
    kerror,
};
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kerror!("{:?}", info.message());
    kerror!("{:?}", info.location());

    // prevent overwriting by graphics::frame_buf
    x86_64::disabled_int(|| {
        panic_screen::write_fmt(format_args!("{:?}\n", info.message())).unwrap();
        panic_screen::write_fmt(format_args!("{:?}\n", info.location())).unwrap();

        qemu::exit(EXIT_FAILURE);
        loop {}
    });

    unreachable!();
}
