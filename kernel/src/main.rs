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
mod test;
mod theme;
mod util;

#[macro_use]
extern crate alloc;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use arch::*;
use common::boot_info::BootInfo;
use fs::{file::bitmap::BitmapImage, vfs};
use graphics::{color::*, frame_buf, multi_layer, simple_window_manager};
use log::*;
use theme::GLOBAL_THEME;
use util::logger;

#[no_mangle]
pub extern "sysv64" fn kernel_entry(boot_info: &BootInfo) -> ! {
    context::switch_kernel_stack(kernel_main, boot_info);
}

pub extern "sysv64" fn kernel_main(boot_info: &BootInfo) -> ! {
    // initialize logger
    device::panic_screen::probe_and_attach(boot_info.graphic_info).unwrap();
    logger::init();

    // attach uart driver
    // do not use .unwrap() here!!
    let _ = device::uart::probe_and_attach();

    // initialize memory management
    mem::init(boot_info.mem_map).unwrap();

    // initialize GDT
    gdt::init().unwrap();
    // initialize PIC and IDT
    idt::init_pic();
    idt::init_idt().unwrap();

    // initialize frame buffer, console
    graphics::init(
        &boot_info.graphic_info,
        GLOBAL_THEME.back_color,
        GLOBAL_THEME.fore_color,
        true,
    )
    .unwrap();

    // initialize graphics shadow buffer and layer manager
    graphics::enable_shadow_buf().unwrap();
    graphics::init_layer_man(&boot_info.graphic_info).unwrap();

    // initialize simple window manager
    graphics::init_simple_wm().unwrap();

    // initialize ACPI
    acpi::init(boot_info.rsdp_virt_addr.unwrap().into()).unwrap();

    // initialize and start local APIC timer
    device::local_apic_timer::probe_and_attach().unwrap();

    // initialize initramfs, VFS
    fs::init(
        boot_info.initramfs_start_virt_addr.into(),
        &boot_info.kernel_config,
    )
    .unwrap();

    // initialize PS/2 keyboard and mouse
    if let Err(err) = device::ps2_keyboard::probe_and_attach() {
        let name = device::ps2_keyboard::get_device_driver_info().unwrap().name;
        error!("{}: Failed to probe or attach device: {:?}", name, err);
    }

    if let Err(err) = device::ps2_mouse::probe_and_attach() {
        let name = device::ps2_mouse::get_device_driver_info().unwrap().name;
        error!("{}: Failed to probe or attach device: {:?}", name, err);
    }

    // initialize my flavor driver
    device::zakki::probe_and_attach().unwrap();

    // initialize pci-bus driver
    device::pci_bus::probe_and_attach().unwrap();

    // initialize xhc driver
    // if let Err(err) = device::usb::xhc::probe_and_attach() {
    //     let name = device::usb::xhc::get_device_driver_info().unwrap().name;
    //     error!("{}: Failed to probe or attach device: {:?}", name, err);
    // }

    // initialize usb-bus driver
    // if let Err(err) = device::usb::bus::probe_and_attach() {
    //     let name = device::usb::bus::get_device_driver_info().unwrap().name;
    //     error!("{}: Failed to probe or attach device: {:?}", name, err);
    // }

    // initalize virtio-net driver
    // if let Err(err) = device::virtio::net::probe_and_attach() {
    //     let name = device::virtio::net::get_device_driver_info().unwrap().name;
    //     error!("{}: Failed to probe or attach device: {:?}", name, err);
    // }

    // initialize speaker driver
    if let Err(err) = device::speaker::probe_and_attach() {
        let name = device::speaker::get_device_driver_info().unwrap().name;
        error!("{}: Failed to probe or attach device: {:?}", name, err);
    }
    // device::speaker::beep();

    // initialize RTL8139 driver
    if let Err(err) = device::rtl8139::probe_and_attach() {
        let name = device::rtl8139::get_device_driver_info().unwrap().name;
        error!("{}: Failed to probe or attach device: {:?}", name, err);
    }

    // enable syscall
    syscall::enable();

    #[cfg(test)]
    test_main();

    env::print_info();
    mem::free();

    // tasks
    let task_graphics = async {
        loop {
            let _ = simple_window_manager::flush_components();
            task::exec_yield().await;
            let _ = multi_layer::draw_to_frame_buf();
            task::exec_yield().await;
            let _ = frame_buf::apply_shadow_buf();
            task::exec_yield().await;
        }
    };

    // let task_poll_virtio_net = async {
    //     loop {
    //         let _ = device::virtio::net::poll_normal();
    //         task::exec_yield().await;
    //     }
    // };

    let task_poll_uart = async {
        loop {
            let _ = device::uart::poll_normal();
            task::exec_yield().await;
        }
    };

    let task_poll_ps2_keyboard = async {
        loop {
            let _ = device::ps2_keyboard::poll_normal();
            task::exec_yield().await;
        }
    };

    let task_poll_rtl8139 = async {
        loop {
            let _ = device::rtl8139::poll_normal();
            task::exec_yield().await;
        }
    };

    task::spawn(task_graphics).unwrap();
    // task::spawn(task_poll_virtio_net).unwrap();
    task::spawn(task_poll_uart).unwrap();
    task::spawn(task_poll_ps2_keyboard).unwrap();
    task::spawn(task_poll_rtl8139).unwrap();
    task::spawn(poll_ps2_mouse(
        boot_info.kernel_config.mouse_pointer_bmp_path.to_string(),
    ))
    .unwrap();
    task::ready().unwrap();

    // execute init app
    let init_app_exec_args = boot_info.kernel_config.init_app_exec_args;
    if let Some(args) = init_app_exec_args {
        let splited: Vec<&str> = args.split(" ").collect();

        loop {
            if splited.len() == 0 || splited[0] == "" {
                error!("Invalid init app exec args: {:?}", args);
                break;
            } else if let Err(err) = fs::exec::exec_elf(&splited[0].into(), &splited[1..], false) {
                error!("{:?}", err);
                break;
            }
        }
    }

    loop {
        arch::hlt();
    }
}

async fn poll_ps2_mouse(mouse_pointer_bmp_path: String) {
    let mut is_created_mouse_pointer_layer = false;
    let mouse_pointer_bmp_fd = loop {
        match vfs::open_file(&((&mouse_pointer_bmp_path).into())) {
            Ok(fd) => break fd,
            Err(e) => {
                warn!("Failed to open mouse pointer bitmap, Retrying...: {:?}", e);
                task::exec_yield().await;
            }
        }
    };

    let bmp_data = loop {
        match vfs::read_file(&mouse_pointer_bmp_fd) {
            Ok(data) => break data,
            Err(e) => {
                warn!("Failed to read mouse pointer bitmap, Retrying...: {:?}", e);
                task::exec_yield().await;
            }
        }
    };

    let pointer_bmp = BitmapImage::new(&bmp_data);
    loop {
        match vfs::close_file(&mouse_pointer_bmp_fd) {
            Ok(()) => break,
            Err(e) => {
                warn!("Failed to close mouse pointer bitmap, Retrying...: {:?}", e);
                task::exec_yield().await;
            }
        }
    }

    loop {
        let mouse_event = match device::ps2_mouse::poll_normal() {
            Ok(Some(e)) => e,
            _ => {
                task::exec_yield().await;
                continue;
            }
        };

        if !is_created_mouse_pointer_layer
            && simple_window_manager::create_mouse_pointer(&pointer_bmp).is_ok()
        {
            is_created_mouse_pointer_layer = true;
        }

        if is_created_mouse_pointer_layer {
            let _ = simple_window_manager::mouse_pointer_event(mouse_event);
        }
        task::exec_yield().await;
    }
}
