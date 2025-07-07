use crate::{
    arch,
    debug::qemu::{self, EXIT_FAILURE},
    device::panic_screen,
    error_,
};
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error_!("{:?}", info.message());
    error_!("{:?}", info.location());

    // prevent overwriting by graphics::frame_buf
    arch::disabled_int(|| {
        panic_screen::write_fmt(format_args!("{:?}\n", info.message())).unwrap();
        panic_screen::write_fmt(format_args!("{:?}\n", info.location())).unwrap();

        qemu::exit(EXIT_FAILURE);
        loop {}
    });

    unreachable!();
}
