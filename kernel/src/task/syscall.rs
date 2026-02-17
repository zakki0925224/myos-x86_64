use crate::{
    arch::{
        x86_64::{self, gdt::*, registers::*},
        VirtualAddress,
    },
    device::tty,
    env,
    error::{Error, Result},
    fs::{
        self,
        vfs::{self, FileDescriptorNumber},
    },
    graphics::{multi_layer::LayerId, window_manager},
    kdebug, kerror, kinfo,
    mem::{bitmap, paging::PAGE_SIZE},
    net::{self, socket::*},
    print,
    task::{self, TaskRequest, TaskResult},
    util,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use common::geometry::{Point, Size};
use core::{arch::naked_asm, net::Ipv4Addr, slice};
use libc_rs::*;

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
enum IomsgCommand {
    RemoveComponent = IOMSG_CMD_REMOVE_COMPONENT,
    CreateComponentWindow = IOMSG_CMD_CREATE_COMPONENT_WINDOW,
    CreateComponentImage = IOMSG_CMD_CREATE_COMPONENT_IMAGE,
}

trait IomsgHeaderExt {
    fn new(cmd: IomsgCommand, payload_size: u32) -> Self;
    fn is_valid(&self) -> bool;
    fn cmd(&self) -> Result<IomsgCommand>;
}

impl IomsgHeaderExt for iomsg_header {
    fn new(cmd: IomsgCommand, payload_size: u32) -> Self {
        Self {
            cmd_id: cmd as u32,
            payload_size,
        }
    }

    fn is_valid(&self) -> bool {
        (self.cmd_id & 0x80000000) != 0 && self.payload_size > 0
    }

    fn cmd(&self) -> Result<IomsgCommand> {
        match self.cmd_id {
            IOMSG_CMD_REMOVE_COMPONENT => Ok(IomsgCommand::RemoveComponent),
            IOMSG_CMD_CREATE_COMPONENT_WINDOW => Ok(IomsgCommand::CreateComponentWindow),
            IOMSG_CMD_CREATE_COMPONENT_IMAGE => Ok(IomsgCommand::CreateComponentImage),
            _ => Err("Invalid command ID".into()),
        }
    }
}

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
        "push r9",      // save r9 (arg6) temporarily
        "mov r9, r8",   // 6th arg (was r8/arg5)
        "mov r8, rcx",  // 5th arg (was rcx/r10/arg4)
        "mov rcx, rdx", // 4th arg (was rdx/arg3)
        "mov rdx, rsi", // 3rd arg (was rsi/arg2)
        "mov rsi, rdi", // 2nd arg (was rdi/arg1)
        "mov rdi, rax", // 1st arg (syscall number from rax)
        "call syscall_handler",
        "pop r9", // restore r9
        "mov rsp, rbp",
        "pop r11",
        "pop rcx",
        "pop rbp",
        "sysretq"
    );
}

#[no_mangle]
extern "sysv64" fn syscall_handler(
    syscall_num: u64, // (sysv abi) rax - syscall number
    arg0: u64,        // (sysv abi) rdi
    arg1: u64,        // (sysv abi) rsi
    arg2: u64,        // (sysv abi) rdx
    arg3: u64,        // (sysv abi) rcx from r10
    arg4: u64,        // (sysv abi) r8
    arg5: u64,        // (sysv abi) r9
) -> i64 /* rax */ {
    tty::check_sigint();

    // let args = [arg0, arg1, arg2, arg3, arg4, arg5];
    // kdebug!(
    //     "syscall: Called!(syscall num: {}, args: {:?})",
    //     syscall_num,
    //     args
    // )

    match syscall_num as u32 {
        SN_READ => {
            let fd_num = arg0 as i32;
            let buf = arg1 as *mut u8;
            let buf_len = arg2 as usize;
            match sys_read(fd_num, buf, buf_len) {
                Ok(len) => return len as i64,
                Err(err) => {
                    kerror!("syscall: read: {:?}", err);
                    return -1;
                }
            }
        }
        SN_WRITE => {
            let fd_num = arg0 as i32;
            let buf = arg1 as *const u8;
            let buf_len = arg2 as usize;
            match sys_write(fd_num, buf, buf_len) {
                Ok(len) => return len as i64,
                Err(err) => {
                    kerror!("syscall: write: {:?}", err);
                    return -1;
                }
            }
        }
        SN_OPEN => {
            let filepath = arg0 as *const u8;
            let flags = arg1 as i32;
            match sys_open(filepath, flags) {
                Ok(fd) => return fd as i64,
                Err(err) => {
                    kerror!("syscall: open: {:?}", err);
                    return -1;
                }
            }
        }
        SN_CLOSE => {
            let fd_num = arg0 as i32;
            if let Err(err) = sys_close(fd_num) {
                kerror!("syscall: close: {:?}", err);
                return -1;
            }
        }
        SN_EXIT => {
            let status = arg0 as i32;
            sys_exit(status);
            unreachable!();
        }
        SN_SBRK => {
            let len = arg0 as usize;
            match sys_sbrk(len) {
                Ok(ptr) => return ptr as i64,
                Err(err) => {
                    kerror!("syscall: sbrk: {:?}", err);
                    return -1;
                }
            }
        }
        SN_UNAME => {
            let buf = arg0 as *mut utsname;
            if let Err(err) = sys_uname(buf) {
                kerror!("syscall: uname: {:?}", err);
                return -1;
            }
        }
        SN_BREAK => {
            sys_break();
            unreachable!();
        }
        SN_STAT => {
            let fd_num = arg0 as i32;
            let buf = arg1 as *mut f_stat;
            if let Err(err) = sys_stat(fd_num, buf) {
                kerror!("syscall: stat: {:?}", err);
                return -1;
            }
        }
        SN_UPTIME => {
            return sys_uptime();
        }
        SN_EXEC => {
            let args = arg0 as *const u8;
            let flags = arg1 as i32;
            if let Err(err) = sys_exec(args, flags) {
                kerror!("syscall: exec: {:?}", err);
                return -1;
            }
        }
        SN_GETCWD => {
            let buf = arg0 as *mut u8;
            let buf_len = arg1 as usize;
            if let Err(err) = sys_getcwd(buf, buf_len) {
                kerror!("syscall: getcwd: {:?}", err);
                return -1;
            }
        }
        SN_CHDIR => {
            let path = arg0 as *const u8;
            if let Err(err) = sys_chdir(path) {
                kerror!("syscall: chdir: {:?}", err);
                return -1;
            }
        }
        SN_FREE => {
            let ptr = arg0 as *const u8;
            if let Err(err) = sys_free(ptr) {
                kerror!("syscall: free: {:?}", err);
                return -1;
            }
        }
        SN_SBRKSZ => {
            let target = arg0 as *const u8;
            match sys_sbrksz(target) {
                Ok(size) => return size as i64,
                Err(err) => {
                    kerror!("syscall: sbrksz: {:?}, target addr: 0x{:x}", err, arg0);
                    return 0;
                }
            };
        }
        SN_GETENAMES => {
            let path = arg0 as *const u8;
            let buf = arg1 as *mut u8;
            let buf_len = arg2 as usize;
            if let Err(err) = sys_getenames(path, buf, buf_len) {
                kerror!("syscall: getenames: {:?}", err);
                return -1;
            }
        }
        SN_IOMSG => {
            let msgbuf = arg0 as *const u8;
            let replymsgbuf = arg1 as *mut u8;
            let replymsgbuf_len = arg2 as usize;
            if let Err(err) = sys_iomsg(msgbuf, replymsgbuf, replymsgbuf_len) {
                kerror!("syscall: iomsg: {:?}", err);
                return -1;
            }
        }
        SN_SOCKET => {
            let domain = arg0 as i32;
            let type_ = arg1 as i32;
            let protocol = arg2 as i32;
            match sys_socket(domain, type_, protocol) {
                Ok(socket_id) => return socket_id.get() as i64,
                Err(err) => {
                    kerror!("syscall: socket: {:?}", err);
                    return -1;
                }
            }
        }
        SN_BIND => {
            let sockfd = arg0 as i32;
            let addr = arg1 as *const sockaddr;
            let addrlen = arg2 as usize;
            if let Err(err) = sys_bind(sockfd, addr, addrlen) {
                kerror!("syscall: bind: {:?}", err);
                return -1;
            }
        }
        SN_SENDTO => {
            let sockfd = arg0 as i32;
            let buf = arg1 as *const u8;
            let len = arg2 as usize;
            let flags = arg3 as i32;
            let dest_addr = arg4 as *const sockaddr;
            let addrlen = arg5 as usize;

            match sys_sendto(sockfd, buf, len, flags, dest_addr, addrlen) {
                Ok(send_len) => return send_len as i64,
                Err(err) => {
                    kerror!("syscall: sendto: {:?}", err);
                    return -1;
                }
            }
        }
        SN_RECVFROM => {
            let sockfd = arg0 as i32;
            let buf = arg1 as *mut u8;
            let len = arg2 as usize;
            let flags = arg3 as i32;
            let src_addr = arg4 as *const sockaddr;
            let addrlen = arg5 as usize;

            match sys_recvfrom(sockfd, buf, len, flags, src_addr, addrlen) {
                Ok(read_len) => return read_len as i64,
                Err(err) => {
                    kerror!("syscall: recvfrom: {:?}", err);
                    return -1;
                }
            }
        }
        SN_SEND => {
            let sockfd = arg0 as i32;
            let buf = arg1 as *const u8;
            let len = arg2 as usize;
            let flags = arg3 as i32;

            match sys_sendto(sockfd, buf, len, flags, core::ptr::null(), 0) {
                Ok(send_len) => return send_len as i64,
                Err(err) => {
                    kerror!("syscall: send: {:?}", err);
                    return -1;
                }
            }
        }
        SN_RECV => {
            let sockfd = arg0 as i32;
            let buf = arg1 as *mut u8;
            let len = arg2 as usize;
            let flags = arg3 as i32;

            match sys_recvfrom(sockfd, buf, len, flags, core::ptr::null(), 0) {
                Ok(read_len) => return read_len as i64,
                Err(err) => {
                    kerror!("syscall: recv: {:?}", err);
                    return -1;
                }
            }
        }
        SN_CONNECT => {
            let sockfd = arg0 as i32;
            let addr = arg1 as *const sockaddr;
            let addrlen = arg2 as usize;

            if let Err(err) = sys_connect(sockfd, addr, addrlen) {
                kerror!("syscall: connect: {:?}", err);
                return -1;
            }
        }
        SN_LISTEN => {
            let sockfd = arg0 as i32;
            let backlog = arg1 as i32;

            if let Err(err) = sys_listen(sockfd, backlog) {
                kerror!("syscall: listen: {:?}", err);
                return -1;
            }
        }
        SN_ACCEPT => {
            let sockfd = arg0 as i32;
            let addr = arg1 as *const sockaddr;
            let addrlen = arg2 as *const i32;

            match sys_accept(sockfd, addr, addrlen) {
                Ok(socket_id) => return socket_id.get() as i64,
                Err(err) => {
                    kerror!("syscall: accept: {:?}", err);
                    return -1;
                }
            }
        }
        num => {
            kerror!("syscall: Syscall number 0x{:x} is not defined", num);
            return -1;
        }
    }

    0
}

fn sys_read(fd_num: i32, buf: *mut u8, buf_len: usize) -> Result<usize> {
    let fd_num = FileDescriptorNumber::new_val(fd_num)?;

    match fd_num {
        FileDescriptorNumber::STDOUT | FileDescriptorNumber::STDERR => {
            return Err("fd is not defined".into());
        }
        FileDescriptorNumber::STDIN => {
            if buf_len > 1 {
                let mut input_s = None;

                while input_s.is_none() {
                    tty::check_sigint();
                    input_s = x86_64::disabled_int(|| tty::get_line()).ok().flatten();
                    x86_64::stihlt();
                }

                let c_s = util::cstring::into_cstring_bytes_with_nul(&input_s.unwrap());

                if buf_len < c_s.len() {
                    return Err("buffer is too small".into());
                }

                unsafe {
                    buf.copy_from_nonoverlapping(c_s.as_ptr(), c_s.len());
                }

                Ok(c_s.len())
            } else if buf_len == 1 {
                let mut c = None;
                while c.is_none() {
                    tty::check_sigint();
                    c = x86_64::disabled_int(|| tty::get_char()).ok().flatten();
                    if c.is_none() {
                        x86_64::stihlt();
                    }
                }

                if buf_len < 1 {
                    return Err("buffer is too small".into());
                }

                unsafe {
                    buf.write(c.unwrap() as u8);
                }

                Ok(1)
            } else {
                Ok(0)
            }
        }
        fd => {
            let data = vfs::read_file(fd)?;

            if buf_len < data.len() {
                return Err("buffer is too small".into());
            }

            unsafe {
                buf.copy_from_nonoverlapping(data.as_ptr(), data.len());
            }

            Ok(data.len())
        }
    }
}

fn sys_write(fd_num: i32, buf: *const u8, buf_len: usize) -> Result<usize> {
    let fd_num = FileDescriptorNumber::new_val(fd_num)?;
    let buf_slice = unsafe { slice::from_raw_parts(buf, buf_len) };

    match fd_num {
        FileDescriptorNumber::STDOUT | FileDescriptorNumber::STDERR => {
            let s = String::from_utf8_lossy(buf_slice).to_string();
            print!("{}", s);
            Ok(buf_len)
        }
        FileDescriptorNumber::STDIN => {
            return Err("cannot write data to stdin".into());
        }
        fd => {
            vfs::write_file(fd, buf_slice)?;
            Ok(buf_len)
        }
    }
}

fn sys_open(filepath: *const u8, flags: i32) -> Result<i32> {
    let filepath = unsafe { util::cstring::from_cstring_ptr(filepath) }
        .as_str()
        .into();
    let create = (flags as u32) & OPEN_FLAG_CREATE != 0;
    let fd_num = vfs::open_file(&filepath, create)?;

    let TaskResult::Ok =
        task::single_scheduler::request(TaskRequest::PushFileDescriptorNumber(fd_num))?
    else {
        unreachable!()
    };

    Ok(fd_num.get() as i32)
}

fn sys_close(fd_num: i32) -> Result<()> {
    if let Ok(fd) = FileDescriptorNumber::new_val(fd_num) {
        if vfs::close_file(fd).is_ok() {
            let TaskResult::Ok =
                task::single_scheduler::request(TaskRequest::RemoveFileDescriptorNumber(fd))?
            else {
                unreachable!()
            };
            return Ok(());
        }
    }

    if let Ok(socket_id) = SocketId::new_val(fd_num) {
        if net::close_socket(socket_id).is_ok() {
            return Ok(());
        }
    }

    Err("Invalid file descriptor".into())
}

fn sys_exit(status: i32) {
    task::single_scheduler::return_task(status)
}

fn sys_sbrk(len: usize) -> Result<*const u8> {
    if len == 0 {
        return Ok(core::ptr::null());
    }

    let mem_frame_info = bitmap::alloc_mem_frame((len + PAGE_SIZE).div_ceil(PAGE_SIZE))?;
    mem_frame_info.set_permissions_to_user()?;
    let virt_addr = mem_frame_info.frame_start_virt_addr()?;
    // kdebug!(
    //     "syscall: sbrk: allocated {} bytes at 0x{:x}",
    //     mem_frame_info.frame_size,
    //     virt_addr.get()
    // );

    let TaskResult::Ok = task::single_scheduler::request(TaskRequest::PushMemory(mem_frame_info))?
    else {
        unreachable!()
    };

    Ok(virt_addr.as_ptr())
}

fn sys_uname(buf: *mut utsname) -> Result<()> {
    let sysname = env::OS_NAME.as_bytes();
    let nodename = "nodename".as_bytes();
    let release = "release".as_bytes();
    let version = env::ENV_VERSION.as_bytes();
    let machine = "x86_64".as_bytes();
    let domainname = "domainname".as_bytes();

    let utsname_mut = unsafe { &mut *buf };

    let sysname_i8: &[i8] =
        unsafe { slice::from_raw_parts(sysname.as_ptr() as *const i8, sysname.len()) };
    utsname_mut.sysname[..sysname.len()].copy_from_slice(sysname_i8);

    let nodename_i8: &[i8] =
        unsafe { slice::from_raw_parts(nodename.as_ptr() as *const i8, nodename.len()) };
    utsname_mut.nodename[..nodename.len()].copy_from_slice(nodename_i8);

    let release_i8: &[i8] =
        unsafe { slice::from_raw_parts(release.as_ptr() as *const i8, release.len()) };
    utsname_mut.release[..release.len()].copy_from_slice(release_i8);

    let version_i8: &[i8] =
        unsafe { slice::from_raw_parts(version.as_ptr() as *const i8, version.len()) };
    utsname_mut.version[..version.len()].copy_from_slice(version_i8);

    let machine_i8: &[i8] =
        unsafe { slice::from_raw_parts(machine.as_ptr() as *const i8, machine.len()) };
    utsname_mut.machine[..machine.len()].copy_from_slice(machine_i8);

    let domainname_i8: &[i8] =
        unsafe { slice::from_raw_parts(domainname.as_ptr() as *const i8, domainname.len()) };
    utsname_mut.domainname[..domainname.len()].copy_from_slice(domainname_i8);

    Ok(())
}

fn sys_break() {
    let _ = task::single_scheduler::request(TaskRequest::ExecuteDebugger);
    x86_64::int3();
}

fn sys_stat(fd_num: i32, buf: *mut f_stat) -> Result<()> {
    let fd_num = FileDescriptorNumber::new_val(fd_num)?;
    let stat_mut = unsafe { &mut *buf };

    let size = match fd_num {
        FileDescriptorNumber::STDIN => tty::input_count()? as usize,
        FileDescriptorNumber::STDOUT | FileDescriptorNumber::STDERR => 0,
        fd => vfs::file_size(fd)?,
    };
    stat_mut.size = size;
    Ok(())
}

fn sys_uptime() -> i64 {
    util::time::global_uptime().as_millis() as i64
}

fn sys_exec(args: *const u8, flags: i32) -> Result<()> {
    let args = unsafe { util::cstring::from_cstring_ptr(args) };
    let args: Vec<&str> = args.split(' ').collect();

    let enable_debug = (flags as u32) & EXEC_FLAG_DEBUG != 0;
    fs::exec::exec_elf(&args[0].into(), &args[1..], enable_debug)?;

    Ok(())
}

fn sys_getcwd(buf: *mut u8, buf_len: usize) -> Result<()> {
    let cwd = vfs::cwd_path()?;
    let cwd_s = util::cstring::into_cstring_bytes_with_nul(cwd.as_str());

    if buf_len < cwd_s.len() {
        return Err("Buffer is too small".into());
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

fn sys_free(ptr: *const u8) -> Result<()> {
    let virt_addr: VirtualAddress = (ptr as u64).into();
    // kdebug!("syscall: free: target memory at 0x{:x}", virt_addr.get());

    let TaskResult::PopMemory(mem_frame_info) =
        task::single_scheduler::request(TaskRequest::PopMemory(virt_addr))?
    else {
        unreachable!()
    };

    mem_frame_info.set_permissions_to_supervisor()?;
    bitmap::dealloc_mem_frame(mem_frame_info)?;
    Ok(())
}

fn sys_sbrksz(target: *const u8) -> Result<usize> {
    let target_virt_addr: VirtualAddress = (target as u64).into();
    let TaskResult::MemoryFrameSize(size) =
        task::single_scheduler::request(TaskRequest::GetMemoryFrameSize(target_virt_addr))?
    else {
        unreachable!()
    };

    let size = size.ok_or::<Error>("Failed to get memory frame size".into())?;
    Ok(size)
}

fn sys_getenames(path: *const u8, buf: *mut u8, buf_len: usize) -> Result<()> {
    let path = unsafe { util::cstring::from_cstring_ptr(path) }
        .as_str()
        .into();

    let entry_names = fs::vfs::entry_names(&path)?;
    let entry_names_s: Vec<u8> = entry_names
        .iter()
        .map(|n| util::cstring::into_cstring_bytes_with_nul(n))
        .flatten()
        .collect();

    if buf_len < entry_names_s.len() {
        return Err("Buffer is too small".into());
    }

    unsafe {
        buf.copy_from_nonoverlapping(entry_names_s.as_ptr(), entry_names_s.len());
    }

    Ok(())
}

fn sys_iomsg(msgbuf: *const u8, replymsgbuf: *mut u8, replymsgbuf_len: usize) -> Result<()> {
    let mut offset = 0;
    let header: &iomsg_header = unsafe { &*(msgbuf as *const iomsg_header) };
    offset += size_of::<iomsg_header>();
    kdebug!("{:?}", header);

    match header.cmd()? {
        IomsgCommand::RemoveComponent => {
            let layer_id: i32 = unsafe { *(msgbuf.offset(offset as isize) as *const i32) };
            offset += size_of::<i32>();

            if (offset - size_of::<iomsg_header>()) != header.payload_size as usize {
                return Err("Invalid payload size for RemoveComponent".into());
            }

            if layer_id < 0 {
                return Err("Invalid layer id".into());
            }

            let layer_id = LayerId::new_val(layer_id as usize);
            window_manager::remove_component(layer_id)?;

            let TaskResult::Ok =
                task::single_scheduler::request(TaskRequest::RemoveLayerId(layer_id))?
            else {
                unreachable!()
            };

            // reply
            let reply_header = iomsg_header::new(IomsgCommand::RemoveComponent, 0);
            if replymsgbuf_len < size_of::<iomsg_header>() {
                return Err("Reply buffer is too small".into());
            }

            unsafe {
                let reply_header_ptr = replymsgbuf as *mut iomsg_header;
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

            let xy = Point::new(x_pos, y_pos);
            let wh = Size::new(width, height);
            let title = unsafe { util::cstring::from_cstring_ptr(title_ptr) };
            offset += title.len() + 1; // null terminator

            if (offset - size_of::<iomsg_header>()) != header.payload_size as usize {
                return Err("Invalid payload size for CreateComponentWindow".into());
            }

            let layer_id = window_manager::create_window(title, xy, wh)?;
            let TaskResult::Ok =
                task::single_scheduler::request(TaskRequest::PushLayerId(layer_id.clone()))?
            else {
                unreachable!()
            };

            // reply
            let reply_header =
                iomsg_header::new(IomsgCommand::CreateComponentWindow, size_of::<u64>() as u32);
            if replymsgbuf_len < size_of::<iomsg_header>() + reply_header.payload_size as usize {
                return Err("Reply buffer is too small".into());
            }

            unsafe {
                let reply_header_ptr = replymsgbuf as *mut iomsg_header;
                reply_header_ptr.write(reply_header);
                let reply_wd = layer_id.get() as u64;
                (replymsgbuf.offset(size_of::<iomsg_header>() as isize) as *mut u64)
                    .write(reply_wd);
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

            if (offset - size_of::<iomsg_header>()) != header.payload_size as usize {
                return Err("Invalid payload size for CreateComponentImage".into());
            }

            if layer_id < 0 {
                return Err("Invalid layer id".into());
            }

            let layer_id = LayerId::new_val(layer_id as usize);
            let wh = Size::new(image_width, image_height);
            let framebuf_virt_addr: VirtualAddress = (framebuf_ptr as u64).into();

            let image = window_manager::components::Image::create_and_push_from_framebuf(
                Point::default(),
                wh,
                framebuf_virt_addr,
                pixel_format.into(),
            )?;
            let new_layer_id = window_manager::add_component_to_window(layer_id, Box::new(image))?;

            // reply
            let reply_header =
                iomsg_header::new(IomsgCommand::CreateComponentImage, size_of::<i32>() as u32);
            if replymsgbuf_len < size_of::<iomsg_header>() + reply_header.payload_size as usize {
                return Err("Reply buffer is too small".into());
            }

            unsafe {
                let reply_header_ptr = replymsgbuf as *mut iomsg_header;
                reply_header_ptr.write(reply_header);
                (replymsgbuf.offset(size_of::<iomsg_header>() as isize) as *mut i32)
                    .write(new_layer_id.get() as i32);
            }
        }
    }

    Ok(())
}

fn sys_socket(domain: i32, type_: i32, _protocol: i32) -> Result<SocketId> {
    if (domain as u32) != SOCKET_DOMAIN_AF_INET {
        return Err("Unsupported domain".into());
    }

    let socket_type = match type_ as u32 {
        SOCKET_TYPE_SOCK_STREAM => SocketType::Stream,
        SOCKET_TYPE_SOCK_DGRAM => SocketType::Dgram,
        _ => return Err("Unsupported type".into()),
    };

    net::create_new_socket(socket_type)
}

fn sys_bind(sockfd: i32, addr: *const sockaddr, addrlen: usize) -> Result<()> {
    let socket_id = SocketId::new_val(sockfd)?;
    let addr = unsafe { *(addr as *const sockaddr_in) };
    assert_eq!(size_of::<sockaddr_in>(), addrlen);

    if addr.sin_family as u32 != SOCKET_DOMAIN_AF_INET {
        return Err("Address family not supported".into());
    }

    let s_addr = addr.sin_addr.s_addr;
    if s_addr != Ipv4Addr::UNSPECIFIED.into() && s_addr != net::my_ipv4_addr()?.into() {
        return Err("Address not available".into());
    }

    let bound_addr = if s_addr == Ipv4Addr::UNSPECIFIED.into() {
        None
    } else {
        Some(s_addr.into())
    };

    let port = if addr.sin_port == 0 {
        None
    } else {
        Some(addr.sin_port)
    };

    net::bind_socket_v4(socket_id, bound_addr, port)
}

fn sys_sendto(
    sockfd: i32,
    buf: *const u8,
    len: usize,
    flags: i32,
    dest_addr: *const sockaddr,
    addrlen: usize,
) -> Result<usize> {
    let socket_id = SocketId::new_val(sockfd)?;
    let data = unsafe { slice::from_raw_parts(buf, len) };

    if dest_addr.is_null() {
        // TCP
        net::send_tcp_packet(socket_id, data)?;
        return Ok(data.len());
    }

    // UDP
    let addr = unsafe { *(dest_addr as *const sockaddr_in) };
    assert_eq!(size_of::<sockaddr_in>(), addrlen);

    let dst_addr = addr.sin_addr.s_addr.into();
    let dst_port = addr.sin_port;

    net::sendto_udp_v4(socket_id, dst_addr, dst_port, data)?;
    Ok(data.len())
}

fn sys_recvfrom(
    sockfd: i32,
    buf: *mut u8,
    len: usize,
    flags: i32,
    src_addr: *const sockaddr,
    addrlen: usize,
) -> Result<usize> {
    let socket_id = SocketId::new_val(sockfd)?;
    let buf_mut = unsafe { slice::from_raw_parts_mut(buf, len) };

    if src_addr.is_null() {
        // TCP
        loop {
            match net::recv_tcp_packet(socket_id, buf_mut) {
                Ok(0) => match net::is_tcp_established(socket_id) {
                    Ok(true) => {
                        x86_64::stihlt();
                        continue;
                    }
                    Ok(false) => return Ok(0),
                    Err(Error::Failed("Mutex is already locked")) => continue,
                    Err(e) => return Err(e),
                },
                Ok(len) => return Ok(len),
                Err(Error::Failed("Mutex is already locked")) => continue,
                Err(e) => return Err(e),
            }
        }
    }

    // UDP
    let read_len = net::recvfrom_udp_v4(socket_id, buf_mut)?;
    Ok(read_len)
}

fn sys_connect(sockfd: i32, addr: *const sockaddr, addrlen: usize) -> Result<()> {
    let socket_id = SocketId::new_val(sockfd)?;

    let addr = unsafe { *(addr as *const sockaddr_in) };
    assert_eq!(size_of::<sockaddr_in>(), addrlen);

    if addr.sin_family as u32 != SOCKET_DOMAIN_AF_INET {
        return Err("Address family not supported".into());
    }

    let dst_addr = addr.sin_addr.s_addr.into();
    let dst_port = addr.sin_port;
    net::connect_tcp_v4(socket_id, dst_addr, dst_port)?;
    net::send_tcp_syn(socket_id)?;

    while !net::is_tcp_established(socket_id)? {
        tty::check_sigint();
        x86_64::stihlt();
    }

    Ok(())
}

fn sys_listen(sockfd: i32, backlog: i32) -> Result<()> {
    let socket_id = SocketId::new_val(sockfd)?;
    net::listen_tcp_v4(socket_id)
}

fn sys_accept(sockfd: i32, addr: *const sockaddr, addrlen: *const i32) -> Result<SocketId> {
    let socket_id = SocketId::new_val(sockfd)?;

    loop {
        tty::check_sigint();
        match net::accept_tcp_v4(socket_id) {
            Ok(client_socket_id) => return Ok(client_socket_id),
            Err(_) => {
                x86_64::stihlt();
            }
        }
    }
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

    kinfo!("syscall: Enabled syscall");
}
