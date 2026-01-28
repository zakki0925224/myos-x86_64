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
    graphics::{
        color::ColorCode,
        frame_buf, multi_layer,
        simple_window_manager::{self, MouseEvent},
    },
    task::{
        async_task::{self, Priority},
        syscall,
    },
    theme::GLOBAL_THEME,
};
use alloc::{string::ToString, vec::Vec};
use common::boot_info::BootInfo;

#[macro_use]
extern crate alloc;

#[no_mangle]
pub extern "sysv64" fn kernel_entry(boot_info: &BootInfo) -> ! {
    context::switch_kernel_stack(kernel_main, boot_info);
}

pub extern "sysv64" fn kernel_main(boot_info: &BootInfo) -> ! {
    let graphic_info = boot_info.graphic_info;

    device::panic_screen::probe_and_attach(graphic_info.clone()).unwrap();

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
        GLOBAL_THEME.console.back,
        GLOBAL_THEME.console.fore,
    )
    .unwrap();

    // initialize graphics shadow buffer and layer manager
    graphics::enable_shadow_buf().unwrap();
    graphics::init_layer_man(&boot_info.graphic_info).unwrap();

    // initialize simple window manager
    graphics::init_simple_wm(boot_info.kernel_config.mouse_pointer_bmp_path.to_string()).unwrap();

    // initialize ACPI
    acpi::init(boot_info.rsdp_virt_addr.unwrap().into()).unwrap();

    // check TSC
    tsc::check_available();

    // initialize and start local APIC timer
    device::local_apic_timer::probe_and_attach().unwrap();

    // initialize initramfs, VFS
    fs::init(
        boot_info.initramfs_start_virt_addr.into(),
        &boot_info.kernel_config,
    )
    .unwrap();

    // initialize urandom
    device::urandom::probe_and_attach().unwrap();

    // initialize TTY device
    device::tty::probe_and_attach().unwrap();

    // initialize PS/2 keyboard and mouse
    device::ps2_keyboard::probe_and_attach().unwrap();
    device::ps2_mouse::probe_and_attach().unwrap();

    // initialize speaker driver
    if let Err(err) = device::speaker::probe_and_attach() {
        let name = device::speaker::get_device_driver_info().unwrap().name;
        kerror!("{}: Failed to probe or attach device: {:?}", name, err);
    }

    // initialize my flavor driver
    device::zakki::probe_and_attach().unwrap();

    // initialize pci-bus driver
    device::pci_bus::probe_and_attach().unwrap();

    // initialize usb-bus driver
    device::usb::usb_bus::probe_and_attach().unwrap();

    // initialize xHC driver
    if let Err(err) = device::usb::xhc::probe_and_attach() {
        let name = device::usb::xhc::get_device_driver_info().unwrap().name;
        kerror!("{}: Failed to probe or attach device: {:?}", name, err);
    }

    // initialize RTL8139 driver
    if let Err(err) = device::rtl8139::probe_and_attach() {
        let name = device::rtl8139::get_device_driver_info().unwrap().name;
        kerror!("{}: Failed to probe or attach device: {:?}", name, err);
    }

    // enable syscall
    syscall::enable();

    #[cfg(test)]
    test_main();

    env::print_info();
    mem::free();

    async_task::spawn_with_priority(graphics(), Priority::High).unwrap();
    async_task::spawn_with_priority(poll_ps2_mouse(), Priority::High).unwrap();
    async_task::spawn(poll_ps2_keyboard()).unwrap();
    async_task::spawn(poll_usb_bus()).unwrap();
    async_task::spawn(poll_xhc()).unwrap();
    async_task::spawn_with_priority(poll_uart(), Priority::Low).unwrap();
    async_task::spawn_with_priority(poll_rtl8139(), Priority::Low).unwrap();
    async_task::spawn_with_priority(mem_monitor(), Priority::Low).unwrap();
    async_task::ready().unwrap();

    // execute init app
    let init_app_exec_args = boot_info.kernel_config.init_app_exec_args;
    if let Some(args) = init_app_exec_args {
        let splited: Vec<&str> = args.split(" ").collect();

        loop {
            if splited.len() == 0 || splited[0] == "" {
                kerror!("Invalid init app exec args: {:?}", args);
                break;
            } else if let Err(err) = fs::exec::exec_elf(&splited[0].into(), &splited[1..], false) {
                kerror!("{:?}", err);
                break;
            }
        }
    }

    loop {
        x86_64::stihlt();
    }
}

// async tasks

async fn graphics() {
    loop {
        let _ = simple_window_manager::flush_components();
        async_task::exec_yield().await;
        let _ = multi_layer::draw_to_frame_buf();
        async_task::exec_yield().await;
        let _ = frame_buf::apply_shadow_buf();
        async_task::exec_yield().await;
    }
}

async fn poll_ps2_mouse() {
    loop {
        let mouse_event = match device::ps2_mouse::poll_normal() {
            Ok(Some(e)) => e,
            _ => {
                async_task::exec_yield().await;
                continue;
            }
        };

        let _ = simple_window_manager::mouse_pointer_event(MouseEvent::Ps2Mouse(mouse_event));
        async_task::exec_yield().await;
    }
}

async fn poll_ps2_keyboard() {
    loop {
        let _ = device::ps2_keyboard::poll_normal();
        async_task::exec_yield().await;
    }
}

async fn poll_usb_bus() {
    loop {
        let _ = device::usb::usb_bus::poll_normal();
        async_task::exec_yield().await;
    }
}

async fn poll_xhc() {
    loop {
        let _ = device::usb::xhc::poll_normal();
        async_task::exec_yield().await;
    }
}

async fn poll_uart() {
    loop {
        let _ = device::uart::poll_normal();
        async_task::exec_yield().await;
    }
}

async fn poll_rtl8139() {
    loop {
        let _ = device::rtl8139::poll_normal();
        async_task::exec_yield().await;
    }
}

async fn mem_monitor() {
    const W: usize = 400;
    const H: usize = 300;

    // create bitmap reference
    let (addr, size) = match mem::bitmap::get_bitmap_region() {
        Ok((addr, size)) => (addr, size),
        Err(_) => {
            kwarn!("Memory manager unavailable");
            return;
        }
    };
    let bitmap_ptr = addr.as_ptr();

    let mut prev_bitmap = vec![0u8; size];
    let mut diff_intensity = vec![0u8; W * H]; // 0~255

    // create layer
    let mut layer = multi_layer::create_layer((0, 0), (W, H)).unwrap();
    layer.always_on_top = true;
    let layer_id = layer.id;
    multi_layer::push_layer(layer).unwrap();

    loop {
        let current_bitmap: &[u8] = unsafe { core::slice::from_raw_parts(bitmap_ptr, size) };

        let _ = multi_layer::draw_layer(layer_id, |l| {
            l.fill(ColorCode::new_rgb(50, 50, 50))?;

            'outer: for i in 0..size {
                let curr_val = current_bitmap[i];
                let prev_val = prev_bitmap[i];

                if i * 8 >= W * H {
                    break 'outer;
                }

                for bit in 0..8 {
                    let page_idx = i * 8 + bit;
                    if page_idx >= W * H {
                        break 'outer;
                    }

                    let curr_bit = (curr_val >> bit) & 1;
                    let prev_bit = (prev_val >> bit) & 1;

                    if curr_bit != prev_bit {
                        diff_intensity[page_idx] = 255;
                    } else {
                        diff_intensity[page_idx] = diff_intensity[page_idx].saturating_sub(10);
                    }

                    let intensity = diff_intensity[page_idx];
                    let is_used = curr_bit != 0;

                    let color = if intensity > 0 {
                        let white_rate = intensity as u32;

                        if is_used {
                            let gb = 100 + (155 * white_rate / 255) as u8;
                            ColorCode::new_rgb(255, gb, gb)
                        } else {
                            let r = (255 * white_rate / 255) as u8;
                            let b = 100 + (155 * white_rate / 255) as u8;
                            ColorCode::new_rgb(r, 255, b)
                        }
                    } else {
                        if is_used {
                            ColorCode::new_rgb(255, 0, 0) // red
                        } else {
                            ColorCode::new_rgb(0, 255, 0) // green
                        }
                    };

                    let px = page_idx % W;
                    let py = page_idx / W;
                    l.draw_pixel((px, py), color)?;
                }
            }

            Ok(())
        });

        prev_bitmap.copy_from_slice(current_bitmap);

        async_task::exec_yield().await;
    }
}
