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
mod port;
mod sync;
mod task;
mod test;
mod theme;
mod util;

use crate::{
    arch::x86_64::{self, *},
    error::Result,
    graphics::{multi_layer, window_manager},
    task::{
        async_task::{self, Priority, TimeoutFuture},
        exec, scheduler, syscall,
    },
    theme::GLOBAL_THEME,
};
use alloc::vec::Vec;
use common::boot_info::BootInfo;
use core::time::Duration;

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
    graphics::init_window_man().unwrap();

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

    device::uart::register_dev().unwrap();

    // initialize urandom
    device::urandom::probe_and_attach().unwrap();

    // initialize power management device
    device::power::probe_and_attach().unwrap();

    // initialize TTY device
    device::tty::probe_and_attach().unwrap();

    // initialize keyboard
    device::keyboard::probe_and_attach().unwrap();

    // initialize mouse
    device::mouse::probe_and_attach().unwrap();

    // initialize PS/2 keyboard and mouse
    device::ps2_keyboard::probe_and_attach().unwrap();
    device::ps2_mouse::probe_and_attach().unwrap();

    // initialize speaker driver
    if let Err(err) = device::speaker::probe_and_attach() {
        kerror!(
            "{}: Failed to probe or attach device: {:?}",
            device::speaker::NAME,
            err
        );
    }

    // initialize my flavor driver
    device::zakki::probe_and_attach().unwrap();

    // initialize pci-bus driver
    device::pci_bus::probe_and_attach().unwrap();

    // initialize usb-bus driver
    device::usb::usb_bus::probe_and_attach().unwrap();

    // probe PCI devices
    if let Err(err) = device::usb::xhc::probe_and_attach() {
        kerror!("{}: Failed to probe or attach device: {:?}", "xhc", err);
    }
    if let Err(err) = device::rtl8139::probe_and_attach() {
        kerror!("{}: Failed to probe or attach device: {:?}", "rtl8139", err);
    }

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
    async_task::spawn_with_priority(poll_loop(device::ps2_mouse::poll_normal), Priority::High)
        .unwrap();
    async_task::spawn(poll_loop(device::ps2_keyboard::poll_normal)).unwrap();
    async_task::spawn(poll_loop(device::keyboard::poll_normal)).unwrap();
    async_task::spawn(poll_loop(device::uart::poll_normal)).unwrap();
    async_task::spawn(poll_loop(device::usb::xhc::poll_normal)).unwrap();
    async_task::spawn(poll_loop(device::usb::usb_bus::poll_normal)).unwrap();
    async_task::spawn_with_priority(poll_loop(device::rtl8139::poll_normal), Priority::Low)
        .unwrap();
    async_task::spawn(async {
        TimeoutFuture::new(Duration::from_secs(5)).await;
        let _ = device::local_apic_timer::self_check();
    })
    .unwrap();
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
    const FRAME_INTERVAL: Duration = Duration::from_millis(20);

    loop {
        let timeout = TimeoutFuture::new(FRAME_INTERVAL);
        let _ = window_manager::flush_components();
        async_task::exec_yield().await;
        let _ = multi_layer::draw_to_frame_buf();
        timeout.await;
    }
}

async fn poll_loop(f: fn() -> Result<()>) {
    loop {
        let _ = f();
        async_task::exec_yield().await;
    }
}
