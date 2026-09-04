#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(sync_unsafe_cell)]
#![feature(custom_test_frameworks)]
#![test_runner(test::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod arch;
mod debug;
mod device;
mod env;
mod error;
mod fs;
mod graphics;
mod mem;
mod net;
mod panic;
mod sync;
mod task;
mod test;
mod theme;
mod util;

use crate::{
    arch::x86_64::{self, *},
    graphics::{multi_layer, window_manager},
    task::{
        async_task::{self, Priority},
        exec, scheduler, syscall,
    },
    theme::GLOBAL_THEME,
};
use alloc::{string::ToString, sync::Arc, vec::Vec};
use common::boot_info::BootInfo;

#[macro_use]
extern crate alloc;

#[no_mangle]
pub extern "sysv64" fn kernel_entry(boot_info: &BootInfo) -> ! {
    context::switch_kernel_stack(kernel_main, boot_info);
}

#[no_mangle]
pub extern "sysv64" fn kernel_main(boot_info: &BootInfo) -> ! {
    let graphic_info = boot_info.graphic_info;

    device::panic_screen::probe_and_attach(graphic_info).unwrap();

    // attach uart driver
    // do not use .unwrap() here!!
    let _ = device::uart::probe_and_attach();

    // initialize memory management
    mem::init(boot_info.mem_map, &graphic_info).unwrap();

    // initialize GDT
    gdt::init();
    // initialize PIC and IDT
    idt::init_pic();
    idt::init();

    // initialize frame buffer, console
    graphics::init(
        &boot_info.graphic_info,
        GLOBAL_THEME.console.back,
        GLOBAL_THEME.console.fore,
    )
    .unwrap();

    // initialize graphics shadow buffer and layer manager
    graphics::enable_shadow_buf().unwrap();
    graphics::init_layer_man(&boot_info.graphic_info).unwrap();

    // initialize window manager
    graphics::init_window_man(boot_info.kernel_config.mouse_pointer_bmp_path.to_string()).unwrap();

    // initialize ACPI
    acpi::init(boot_info.rsdp_virt_addr.unwrap().into()).unwrap();

    // initialize TSC
    tsc::init();

    // initialize and start local APIC timer
    device::local_apic_timer::probe_and_attach().unwrap();

    // initialize initramfs, VFS
    fs::init(
        boot_info.initramfs_start_virt_addr.into(),
        &boot_info.kernel_config,
    )
    .unwrap();

    device::uart::register().unwrap();

    // initialize urandom
    device::urandom::probe_and_attach().unwrap();

    // initialize TTY device
    device::tty::probe_and_attach().unwrap();

    // initialize keyboard
    device::keyboard::probe_and_attach().unwrap();

    // initialize PS/2 keyboard and mouse
    device::ps2_keyboard::probe_and_attach().unwrap();
    device::ps2_mouse::probe_and_attach().unwrap();

    // initialize speaker driver
    if let Err(err) = device::speaker::probe_and_attach() {
        let name = device::speaker::device_info().unwrap().name;
        kerror!("{}: Failed to probe or attach device: {:?}", name, err);
    }

    // initialize my flavor driver
    device::zakki::probe_and_attach().unwrap();

    // initialize pci-bus driver
    device::pci_bus::probe_and_attach().unwrap();

    // initialize usb-bus driver
    device::usb::usb_bus::probe_and_attach().unwrap();

    // register USB device drivers
    device::usb::usb_bus::register_driver(Arc::new(
        device::usb::hid_keyboard::UsbHidKeyboardDriver,
    ))
    .unwrap();
    device::usb::usb_bus::register_driver(Arc::new(device::usb::hid_tablet::UsbHidTabletDriver))
        .unwrap();

    // probe PCI devices
    device::pci_bus::register_driver(Arc::new(device::usb::xhc::XhciDriver)).unwrap();
    device::pci_bus::register_driver(Arc::new(device::rtl8139::Rtl8139Driver)).unwrap();
    device::pci_bus::probe_all().unwrap();

    // enable syscall
    syscall::enable();

    #[cfg(test)]
    test_main();

    env::print_info();
    mem::debug_usage();

    // initialize scheduler
    scheduler::init().unwrap();

    // do not spawn async tasks before initialize scheduler
    // because kernel task id must be 0
    async_task::spawn_with_priority(graphics(), Priority::High).unwrap();
    async_task::spawn_with_priority(poll_devices(Priority::High), Priority::High).unwrap();
    async_task::spawn(poll_devices(Priority::Normal)).unwrap();
    async_task::spawn_with_priority(poll_devices(Priority::Low), Priority::Low).unwrap();
    async_task::ready().unwrap();

    // execute init app
    let init_app_exec_args = boot_info.kernel_config.init_app_exec_args;

    if let Some(args) = init_app_exec_args {
        let splited: Vec<&str> = args.split(" ").collect();

        if splited.is_empty() || splited[0].is_empty() {
            panic!("Invalid init app exec args: {:?}", args);
        } else if let Err(err) =
            exec::exec_elf(&splited[0].into(), &splited[1..], [None, None, None])
        {
            panic!("{:?}", err);
        }
    } else {
        panic!("Init app exec args not found");
    }

    loop {
        x86_64::sti();
        let _ = async_task::poll();
    }
}

// async tasks

async fn graphics() {
    loop {
        let _ = window_manager::flush_components();
        async_task::exec_yield().await;
        let _ = multi_layer::draw_to_frame_buf();
        async_task::exec_yield().await;
    }
}

async fn poll_devices(priority: Priority) {
    loop {
        let _ = device::poll_devices(priority);
        async_task::exec_yield().await;
    }
}
