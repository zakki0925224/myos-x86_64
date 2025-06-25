use crate::{
    arch::{
        addr::VirtualAddress,
        gdt::*,
        iomsg::{IomsgCommand, IomsgHeader},
        register::{model_specific::*, Register},
        task,
    },
    debug,
    device::tty,
    env, error as m_error,
    error::*,
    fs::{
        self,
        vfs::{self, FileDescriptorNumber},
    },
    graphics::{multi_layer::LayerId, simple_window_manager},
    info,
    mem::{bitmap, paging::PAGE_SIZE},
    print, util,
};
use alloc::{boxed::Box, ffi::CString, string::*, vec::Vec};
use common::libc::{Stat, Utsname};
use core::{arch::naked_asm, slice};

#[unsafe(naked)]
extern "sysv64" fn asm_syscall_handler() {
    naked_asm!(
        "push rbp",
        "push rcx",
        "push r11",     // rflags
        "mov rcx, r10", // rcx was updated by syscall instruction
        "mov rbp, rsp",
        "and rsp, -16",
        "pushfq",
        "pop r11",
        "and r11, ~0x100", // clear TF
        "push r11",
        "popfq",
        "call syscall_handler",
        "mov rsp, rbp",
        "pop r11",
        "pop rcx",
        "pop rbp",
        "sysretq"
    );
}

#[no_mangle]
extern "sysv64" fn syscall_handler(
    arg0: u64, // (sysv abi) rdi
    arg1: u64, // (sysv abi) rsi
    arg2: u64, // (sysv abi) rdx
    arg3: u64, // (sysv abi) rcx from r10
    arg4: u64, // (sysv abi) r8
    arg5: u64, // (sysv abi) r9
) -> i64 /* rax */ {
    // let args = [arg0, arg1, arg2, arg3, arg4, arg5];
    // debug!("syscall: Called!(args: {:?})", args);

    match arg0 {
        // read syscall
        0 => {
            let fd = arg1 as i32;
            let buf = arg2 as *mut u8;
            let buf_len = arg3 as usize;
            if let Err(err) = sys_read(fd, buf, buf_len) {
                m_error!("syscall: read: {:?}", err);
                return -1;
            }
        }
        // write syscall
        1 => {
            let fd = arg1 as i32;
            let buf = arg2 as *const u8;
            let buf_len = arg3 as usize;
            if let Err(err) = sys_write(fd, buf, buf_len) {
                m_error!("syscall: write: {:?}", err);
                return -1;
            }
        }
        // open syscall
        2 => {
            let filepath = arg1 as *const u8;
            let flags = arg2 as u32;
            match sys_open(filepath, flags) {
                Ok(fd) => return fd as i64,
                Err(err) => {
                    m_error!("syscall: open: {:?}", err);
                    return -1;
                }
            }
        }
        // close syscall
        3 => {
            let fd = arg1 as i32;
            if let Err(err) = sys_close(fd) {
                m_error!("syscall: close: {:?}", err);
                return -1;
            }
        }
        // exit syscall
        4 => {
            let status = arg1 as i32;
            sys_exit(status);
            unreachable!();
        }
        // sbrk syscall
        5 => {
            let len = arg1 as usize;
            match sys_sbrk(len) {
                Ok(ptr) => return ptr as i64,
                Err(err) => {
                    m_error!("syscall: sbrk: {:?}", err);
                    return -1;
                }
            }
        }
        // uname syscall
        6 => {
            let buf = arg1 as *mut Utsname;
            if let Err(err) = sys_uname(buf) {
                m_error!("syscall: uname: {:?}", err);
                return -1;
            }
        }
        // break syscall
        7 => {
            sys_break();
            unreachable!();
        }
        // stat syscall
        8 => {
            let fd = arg1 as i32;
            let buf = arg2 as *mut Stat;
            if let Err(err) = sys_stat(fd, buf) {
                m_error!("syscall: stat: {:?}", err);
                return -1;
            }
        }
        // uptime syscall
        9 => {
            return sys_uptime();
        }
        // exec syscall
        10 => {
            let args = arg1 as *const u8;
            let flags = arg2 as u32;
            if let Err(err) = sys_exec(args, flags) {
                m_error!("syscall: exec: {:?}", err);
                return -1;
            }
        }
        // getcwd syscall
        11 => {
            let buf = arg1 as *mut u8;
            let buf_len = arg2 as usize;
            if let Err(err) = sys_getcwd(buf, buf_len) {
                m_error!("syscall: getcwd: {:?}", err);
                return -1;
            }
        }
        // chdir syscall
        12 => {
            let path = arg1 as *const u8;
            if let Err(err) = sys_chdir(path) {
                m_error!("syscall: chdir: {:?}", err);
                return -1;
            }
        }
        // sbrksz syscall
        15 => {
            let target = arg1 as *const u8;
            match sys_sbrksz(target) {
                Ok(size) => return size as i64,
                Err(err) => {
                    m_error!("syscall: sbrksz: {:?}, target addr: 0x{:x}", err, arg1);
                    return 0;
                }
            };
        }
        // getenames syscall
        17 => {
            let path = arg1 as *const u8;
            let buf = arg2 as *mut u8;
            let buf_len = arg3 as usize;

            if let Err(err) = sys_getenames(path, buf, buf_len) {
                m_error!("syscall: getenames: {:?}", err);
                return -1;
            }
        }
        // iomsg syscall
        18 => {
            let msgbuf = arg1 as *const u8;
            let replymsgbuf = arg2 as *mut u8;
            let replymsgbuf_len = arg3 as usize;
            if let Err(err) = sys_iomsg(msgbuf, replymsgbuf, replymsgbuf_len) {
                m_error!("syscall: iomsg: {:?}", err);
                return -1;
            }
        }
        num => {
            m_error!("syscall: Syscall number 0x{:x} is not defined", num);
            return -1;
        }
    }

    0
}

fn sys_read(fd: i32, buf: *mut u8, buf_len: usize) -> Result<()> {
    let fd = FileDescriptorNumber::new_val(fd)?;

    match fd {
        FileDescriptorNumber::STDOUT | FileDescriptorNumber::STDERR => {
            return Err(Error::Failed("fd is not defined"));
        }
        FileDescriptorNumber::STDIN => {
            if buf_len > 1 {
                let mut input_s = None;

                while input_s.is_none() {
                    if let Ok(s) = crate::arch::disabled_int(|| tty::get_line()) {
                        input_s = s;
                    } else {
                        super::hlt();
                    }
                }

                let c_s: Vec<u8> = CString::new(input_s.unwrap())
                    .unwrap()
                    .into_bytes_with_nul();

                if buf_len < c_s.len() {
                    return Err(Error::Failed("buffer is too small"));
                }

                unsafe {
                    buf.copy_from_nonoverlapping(c_s.as_ptr(), c_s.len());
                }
            } else if buf_len == 1 {
                let mut c = None;
                while c.is_none() {
                    if let Ok(ch) = crate::arch::disabled_int(|| tty::get_char()) {
                        c = Some(ch);
                    } else {
                        super::hlt();
                    }
                }

                if buf_len < 1 {
                    return Err(Error::Failed("buffer is too small"));
                }

                unsafe {
                    buf.write(c.unwrap() as u8);
                }
            }
        }
        fd => {
            let data = vfs::read_file(&fd)?;

            if buf_len < data.len() {
                return Err(Error::Failed("buffer is too small"));
            }

            unsafe {
                buf.copy_from_nonoverlapping(data.as_ptr(), data.len());
            }
        }
    }

    Ok(())
}

fn sys_write(fd: i32, buf: *const u8, buf_len: usize) -> Result<()> {
    let fd = FileDescriptorNumber::new_val(fd)?;
    let buf_slice = unsafe { slice::from_raw_parts(buf, buf_len) };

    match fd {
        FileDescriptorNumber::STDOUT => {
            let s = String::from_utf8_lossy(buf_slice).to_string();
            print!("{}", s);
        }
        FileDescriptorNumber::STDIN | FileDescriptorNumber::STDERR => {
            return Err(Error::Failed("fd is not defined"));
        }
        fd => {
            vfs::write_file(&fd, buf_slice)?;
        }
    }

    Ok(())
}

fn sys_open(filepath: *const u8, flags: u32) -> Result<i32> {
    let filepath = unsafe { util::cstring::from_cstring_ptr(filepath) }
        .as_str()
        .into();
    let create = flags & 0x1 != 0;
    let fd = vfs::open_file(&filepath, create)?;
    task::push_fd(fd);

    Ok(fd.get() as i32)
}

fn sys_close(fd: i32) -> Result<()> {
    let fd = FileDescriptorNumber::new_val(fd)?;
    vfs::close_file(&fd)?;
    task::remove_fd(&fd);

    Ok(())
}

fn sys_exit(status: i32) {
    task::return_task(status);
}

fn sys_sbrk(len: usize) -> Result<*const u8> {
    assert!(len > 0);
    let mem_frame_info = bitmap::alloc_mem_frame((len + PAGE_SIZE).div_ceil(PAGE_SIZE))?;
    mem_frame_info.set_permissions_to_user()?;
    let virt_addr = mem_frame_info.frame_start_virt_addr()?;
    debug!(
        "syscall: sbrk: allocated {} bytes at 0x{:x}",
        mem_frame_info.frame_size,
        virt_addr.get()
    );
    task::push_allocated_mem_frame_info_for_user_task(mem_frame_info)?;
    Ok(virt_addr.as_ptr())
}

fn sys_uname(buf: *mut Utsname) -> Result<()> {
    let sysname = env::OS_NAME.as_bytes();
    let nodename = "nodename".as_bytes();
    let release = "release".as_bytes();
    let version = env::ENV_VERSION.as_bytes();
    let machine = "x86_64".as_bytes();
    let domainname = "domainname".as_bytes();

    let utsname_mut = unsafe { &mut *buf };
    utsname_mut.sysname[..sysname.len()].copy_from_slice(sysname);
    utsname_mut.nodename[..nodename.len()].copy_from_slice(nodename);
    utsname_mut.release[..release.len()].copy_from_slice(release);
    utsname_mut.version[..version.len()].copy_from_slice(version);
    utsname_mut.machine[..machine.len()].copy_from_slice(machine);
    utsname_mut.domainname[..domainname.len()].copy_from_slice(domainname);
    Ok(())
}

fn sys_break() {
    task::debug_user_task();
    super::int3();
}

fn sys_stat(fd: i32, buf: *mut Stat) -> Result<()> {
    let fd = FileDescriptorNumber::new_val(fd)?;
    let stat_mut = unsafe { &mut *buf };

    let size = match fd {
        FileDescriptorNumber::STDIN
        | FileDescriptorNumber::STDOUT
        | FileDescriptorNumber::STDERR => 0,
        fd => vfs::read_file(&fd)?.len(),
    };
    stat_mut.size = size;
    Ok(())
}

fn sys_uptime() -> i64 {
    util::time::global_uptime().as_millis() as i64
}

fn sys_exec(args: *const u8, flags: u32) -> Result<()> {
    let args = unsafe { util::cstring::from_cstring_ptr(args) };
    let args: Vec<&str> = args.split(' ').collect();

    let enable_debug = flags & 0x1 != 0;
    fs::exec::exec_elf(&args[0].into(), &args[1..], enable_debug)?;

    Ok(())
}

fn sys_getcwd(buf: *mut u8, buf_len: usize) -> Result<()> {
    let cwd = vfs::cwd_path()?;
    let cwd_s: Vec<u8> = CString::new(cwd.to_string().as_str())
        .unwrap()
        .into_bytes_with_nul();

    if buf_len < cwd_s.len() {
        return Err(Error::Failed("Buffer is too small"));
    }

    unsafe {
        buf.copy_from_nonoverlapping(cwd_s.as_ptr(), cwd_s.len());
    }

    Ok(())
}

fn sys_chdir(path: *const u8) -> Result<()> {
    let path = unsafe { util::cstring::from_cstring_ptr(path) }
        .as_str()
        .into();
    vfs::chdir(&path)?;
    Ok(())
}

fn sys_sbrksz(target: *const u8) -> Result<usize> {
    let target_virt_addr: VirtualAddress = (target as u64).into();
    let size = task::get_memory_frame_size_by_virt_addr(target_virt_addr)?
        .ok_or(Error::Failed("Failed to get memory frame size"))?;
    Ok(size)
}

fn sys_getenames(path: *const u8, buf: *mut u8, buf_len: usize) -> Result<()> {
    let path = unsafe { util::cstring::from_cstring_ptr(path) }
        .as_str()
        .into();

    let entry_names = fs::vfs::entry_names(&path)?;
    let entry_names_s: Vec<u8> = entry_names
        .iter()
        .map(|n| CString::new(n.as_str()).unwrap().into_bytes_with_nul())
        .flatten()
        .collect();

    if buf_len < entry_names_s.len() {
        return Err(Error::Failed("Buffer is too small"));
    }

    unsafe {
        buf.copy_from_nonoverlapping(entry_names_s.as_ptr(), entry_names_s.len());
    }

    Ok(())
}

fn sys_iomsg(msgbuf: *const u8, replymsgbuf: *mut u8, replymsgbuf_len: usize) -> Result<()> {
    let mut offset = 0;
    let header: &IomsgHeader = unsafe { &*(msgbuf as *const IomsgHeader) };
    offset += size_of::<IomsgHeader>();
    debug!("{:?}", header);

    match header.cmd()? {
        IomsgCommand::RemoveComponent => {
            let layer_id: i32 = unsafe { *(msgbuf.offset(offset as isize) as *const i32) };
            offset += size_of::<i32>();

            if (offset - size_of::<IomsgHeader>()) != header.payload_size as usize {
                return Err(Error::Failed("Invalid payload size for RemoveComponent"));
            }

            let layer_id = LayerId::new_val(layer_id)?;
            simple_window_manager::remove_component(&layer_id)?;
            task::remove_layer_id(&layer_id);

            // reply
            let reply_header = IomsgHeader::new(IomsgCommand::RemoveComponent, 0);
            if replymsgbuf_len < size_of::<IomsgHeader>() {
                return Err(Error::Failed("Reply buffer is too small"));
            }

            unsafe {
                let reply_header_ptr = replymsgbuf as *mut IomsgHeader;
                reply_header_ptr.write(reply_header);
            }
        }
        IomsgCommand::CreateComponentWindow => {
            let x_pos: usize = unsafe { *(msgbuf.offset(offset as isize) as *const usize) };
            offset += size_of::<usize>();
            let y_pos: usize = unsafe { *(msgbuf.offset(offset as isize) as *const usize) };
            offset += size_of::<usize>();
            let width: usize = unsafe { *(msgbuf.offset(offset as isize) as *const usize) };
            offset += size_of::<usize>();
            let height: usize = unsafe { *(msgbuf.offset(offset as isize) as *const usize) };
            offset += size_of::<usize>();
            let title_ptr = unsafe { msgbuf.offset(offset as isize) as *const u8 };

            let xy = (x_pos as usize, y_pos as usize);
            let wh = (width as usize, height as usize);
            let title = unsafe { util::cstring::from_cstring_ptr(title_ptr) };
            offset += title.len() + 1; // null terminator

            if (offset - size_of::<IomsgHeader>()) != header.payload_size as usize {
                return Err(Error::Failed(
                    "Invalid payload size for CreateComponentWindow",
                ));
            }

            let layer_id = simple_window_manager::create_window(title, xy, wh)?;
            task::push_layer_id(layer_id.clone());

            // reply
            let reply_header =
                IomsgHeader::new(IomsgCommand::CreateComponentWindow, size_of::<u64>() as u32);
            if replymsgbuf_len < size_of::<IomsgHeader>() + reply_header.payload_size as usize {
                return Err(Error::Failed("Reply buffer is too small"));
            }

            unsafe {
                let reply_header_ptr = replymsgbuf as *mut IomsgHeader;
                reply_header_ptr.write(reply_header);
                let reply_wd = layer_id.get() as u64;
                (replymsgbuf.offset(size_of::<IomsgHeader>() as isize) as *mut u64).write(reply_wd);
            }
        }
        IomsgCommand::CreateComponentImage => {
            let layer_id: i32 = unsafe { *(msgbuf.offset(offset as isize) as *const i32) };
            offset += size_of::<i32>();
            offset += 4; // padding
            let image_width: usize = unsafe { *(msgbuf.offset(offset as isize) as *const usize) };
            offset += size_of::<usize>();
            let image_height: usize = unsafe { *(msgbuf.offset(offset as isize) as *const usize) };
            offset += size_of::<usize>();
            let pixel_format: u8 = unsafe { *(msgbuf.offset(offset as isize) as *const u8) };
            offset += size_of::<u8>();
            offset += 7; // padding
            let framebuf_ptr =
                unsafe { *(msgbuf.offset(offset as isize) as *const usize) } as *const u8;
            offset += size_of::<usize>();

            if (offset - size_of::<IomsgHeader>()) != header.payload_size as usize {
                return Err(Error::Failed(
                    "Invalid payload size for CreateComponentImage",
                ));
            }

            let layer_id = LayerId::new_val(layer_id)?;
            let wh = (image_width, image_height);
            let framebuf_virt_addr: VirtualAddress = (framebuf_ptr as u64).into();

            let image = simple_window_manager::components::Image::create_and_push_from_framebuf(
                (0, 0),
                wh,
                framebuf_virt_addr,
                pixel_format.into(),
            )?;
            let new_layer_id =
                simple_window_manager::add_component_to_window(&layer_id, Box::new(image))?;

            // reply
            let reply_header =
                IomsgHeader::new(IomsgCommand::CreateComponentImage, size_of::<i32>() as u32);
            if replymsgbuf_len < size_of::<IomsgHeader>() + reply_header.payload_size as usize {
                return Err(Error::Failed("Reply buffer is too small"));
            }

            unsafe {
                let reply_header_ptr = replymsgbuf as *mut IomsgHeader;
                reply_header_ptr.write(reply_header);
                (replymsgbuf.offset(size_of::<IomsgHeader>() as isize) as *mut i32)
                    .write(new_layer_id.get() as i32);
            }
        }
    }

    Ok(())
}

pub fn enable() {
    let mut efer = ExtendedFeatureEnableRegister::read();
    efer.set_syscall_enable(true);
    efer.write();
    assert_eq!(ExtendedFeatureEnableRegister::read().syscall_enable(), true);

    let asm_syscall_handler_addr = asm_syscall_handler as *const () as u64;
    let mut lstar = LongModeSystemCallTargetAddressRegister::read();
    lstar.set_target_addr(asm_syscall_handler_addr);
    lstar.write();
    assert_eq!(
        LongModeSystemCallTargetAddressRegister::read().target_addr(),
        asm_syscall_handler_addr
    );

    let target_addr =
        ((KERNEL_MODE_CS_VALUE as u64) << 32) | ((KERNEL_MODE_SS_VALUE as u64 | 3) << 48);
    let mut star = SystemCallTargetAddressRegister::read();
    star.set_target_addr(target_addr); // set CS and SS to kernel segment
    star.write();
    assert_eq!(
        SystemCallTargetAddressRegister::read().target_addr(),
        target_addr
    );

    let mut fmask = SystemCallFlagMaskRegister::read();
    fmask.set_value(0);
    fmask.write();
    assert_eq!(SystemCallFlagMaskRegister::read().value(), 0);

    info!("syscall: Enabled syscall");
}
