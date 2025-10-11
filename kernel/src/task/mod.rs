use crate::{
    arch::{
        x86_64::context::{Context, ContextMode},
        VirtualAddress,
    },
    debug::dwarf::Dwarf,
    error::{Error, Result},
    fs::{
        path::Path,
        vfs::{self, *},
    },
    graphics::{multi_layer::LayerId, simple_window_manager},
    kdebug,
    mem::{
        bitmap::{self, MemoryFrameInfo},
        paging::{self, *},
    },
    sync::mutex::Mutex,
    util::{
        self,
        id::{AtomicId, AtomicIdMarker},
    },
};
use alloc::{string::ToString, vec::Vec};
use common::elf::{self, Elf64};

pub mod async_task;
pub mod syscall;

const USER_TASK_STACK_SIZE: usize = 1024 * 1024; // 1MiB

static mut KERNEL_TASK: Mutex<Option<Task>> = Mutex::new(None);
static mut USER_TASKS: Mutex<Vec<Task>> = Mutex::new(Vec::new());
static mut USER_EXIT_STATUS: Option<i32> = None;

#[derive(Debug, Clone)]
pub struct TaskIdInner;
impl AtomicIdMarker for TaskIdInner {}
pub type TaskId = AtomicId<TaskIdInner>;

#[derive(Debug, Clone)]
struct Task {
    id: TaskId,
    context: Context,
    args_mem_frame_info: Option<MemoryFrameInfo>,
    stack_mem_frame_info: MemoryFrameInfo,
    program_mem_info: Vec<(MemoryFrameInfo, MappingInfo)>,
    allocated_mem_frame_info: Vec<MemoryFrameInfo>,
    created_layer_ids: Vec<LayerId>,
    opend_fd_num: Vec<FileDescriptorNumber>,
    dwarf: Option<Dwarf>,
}

impl Drop for Task {
    fn drop(&mut self) {
        if let Some(args_mem_frame_info) = self.args_mem_frame_info {
            args_mem_frame_info.set_permissions_to_supervisor().unwrap();
            bitmap::dealloc_mem_frame(args_mem_frame_info).unwrap();
        }

        self.stack_mem_frame_info
            .set_permissions_to_supervisor()
            .unwrap();
        bitmap::dealloc_mem_frame(self.stack_mem_frame_info).unwrap();

        for (mem_info, mapping_info) in self.program_mem_info.iter() {
            let start = mapping_info.start;
            paging::update_mapping(&MappingInfo {
                start,
                end: mapping_info.end,
                phys_addr: start.get().into(),
                rw: ReadWrite::Write,
                us: EntryMode::Supervisor,
                pwt: PageWriteThroughLevel::WriteThrough,
                pcd: false,
            })
            .unwrap();

            // assert_eq!(
            //     paging::calc_virt_addr(start.get().into()).unwrap().get(),
            //     start.get()
            // );
            bitmap::dealloc_mem_frame(*mem_info).unwrap();
        }

        for mem_frame_info in self.allocated_mem_frame_info.iter() {
            mem_frame_info.set_permissions_to_supervisor().unwrap();
            bitmap::dealloc_mem_frame(*mem_frame_info).unwrap();
        }

        // destroy all created windows
        for layer_id in self.created_layer_ids.iter() {
            let _ = simple_window_manager::remove_component(layer_id);
        }

        // close all opend files
        for fd in self.opend_fd_num.iter() {
            vfs::close_file(fd).unwrap();
        }

        kdebug!("task: Dropped tid: {}", self.id.get());
    }
}

impl Task {
    fn new(
        stack_size: usize, // 4KiB align
        elf64: Option<Elf64>,
        args: Option<&[&str]>, // file name + args
        mode: ContextMode,
        dwarf: Option<Dwarf>,
    ) -> Result<Self> {
        // parse ELF
        let mut entry = None;
        let mut program_mem_info = Vec::new();
        if let Some(elf64) = elf64 {
            let header = elf64.header();

            if header.elf_type() != elf::Type::Executable {
                return Err(Error::Failed("The file is not an executable file"));
            }

            if header.machine() != elf::Machine::X8664 {
                return Err(Error::Failed("Unsupported ISA"));
            }

            for program_header in elf64.program_headers() {
                if program_header.segment_type() != elf::SegmentType::Load {
                    continue;
                }

                let p_virt_addr = program_header.virt_addr;
                let p_mem_size = program_header.mem_size;
                let p_file_size = program_header.file_size;

                let pages_needed =
                    ((p_virt_addr % PAGE_SIZE as u64 + p_mem_size + PAGE_SIZE as u64 - 1)
                        / PAGE_SIZE as u64) as usize;
                let user_mem_frame_info = bitmap::alloc_mem_frame(pages_needed)?;
                bitmap::mem_clear(&user_mem_frame_info)?;
                let user_mem_frame_start_virt_addr = user_mem_frame_info.frame_start_virt_addr()?;

                // copy data
                let program_data = elf64.data_by_program_header(program_header);
                if let Some(data) = program_data {
                    user_mem_frame_start_virt_addr
                        .offset(p_virt_addr as usize % PAGE_SIZE)
                        .copy_from_nonoverlapping(data.as_ptr(), p_file_size as usize);
                }

                // update page mapping
                let start_virt_addr = (p_virt_addr / PAGE_SIZE as u64 * PAGE_SIZE as u64).into();
                let mapping_info = MappingInfo {
                    start: start_virt_addr,
                    end: start_virt_addr.offset(user_mem_frame_info.frame_size),
                    phys_addr: user_mem_frame_info.frame_start_phys_addr,
                    rw: ReadWrite::Write,
                    us: EntryMode::User,
                    pwt: PageWriteThroughLevel::WriteThrough,
                    pcd: false,
                };
                paging::update_mapping(&mapping_info)?;
                program_mem_info.push((user_mem_frame_info, mapping_info));

                if header.entry_point >= p_virt_addr
                    && header.entry_point < p_virt_addr + p_mem_size
                {
                    entry = Some(header.entry_point);
                }
            }
        }

        let rip = match entry {
            Some(f) => f as u64,
            None => 0,
        };

        // stack
        let stack_mem_frame_info = bitmap::alloc_mem_frame(stack_size.div_ceil(PAGE_SIZE).max(1))?;
        match mode {
            ContextMode::Kernel => stack_mem_frame_info.set_permissions_to_supervisor()?,
            ContextMode::User => stack_mem_frame_info.set_permissions_to_user()?,
        }
        let rsp =
            (stack_mem_frame_info.frame_start_virt_addr()?.get() + stack_size as u64 - 63) & !63;
        assert!(rsp % 64 == 0); // must be 64 bytes align for SSE and AVX instructions, etc.

        // args
        let mut args_mem_frame_info = None;
        let mut arg0 = 0; // args len
        let mut arg1 = 0; // args virt addr
        if let Some(args) = args {
            let mut c_args = Vec::new();
            for arg in args {
                c_args.extend(util::cstring::into_cstring_bytes_with_nul(arg.to_string()));
            }

            let mut c_args_offset = (args.len() + 2) * 8;
            let mem_frame_info =
                bitmap::alloc_mem_frame(((c_args.len() + c_args_offset) / PAGE_SIZE).max(1))?;
            bitmap::mem_clear(&mem_frame_info)?;
            match mode {
                ContextMode::Kernel => mem_frame_info.set_permissions_to_supervisor()?,
                ContextMode::User => mem_frame_info.set_permissions_to_user()?,
            }

            let args_mem_virt_addr = mem_frame_info.frame_start_virt_addr()?;
            args_mem_virt_addr
                .offset(c_args_offset)
                .copy_from_nonoverlapping(c_args.as_ptr(), c_args.len());

            let mut c_args_ref = Vec::new();
            for arg in args {
                c_args_ref.push(args_mem_virt_addr.offset(c_args_offset).get());
                c_args_offset += arg.len() + 1;
            }
            args_mem_virt_addr.copy_from_nonoverlapping(c_args_ref.as_ptr(), c_args_ref.len());

            args_mem_frame_info = Some(mem_frame_info);
            arg0 = args.len() as u64;
            arg1 = args_mem_virt_addr.get();
        }

        // context
        let mut context = Context::new();
        context.init(rip, arg0, arg1, rsp, mode, dwarf.is_some());

        Ok(Self {
            id: TaskId::new(),
            context,
            args_mem_frame_info,
            stack_mem_frame_info,
            program_mem_info,
            allocated_mem_frame_info: Vec::new(),
            created_layer_ids: Vec::new(),
            opend_fd_num: Vec::new(),
            dwarf,
        })
    }

    fn unmap_virt_addr(&self) -> Result<()> {
        for (_, mapping_info) in self.program_mem_info.iter() {
            let start = mapping_info.start;
            paging::update_mapping(&MappingInfo {
                start,
                end: mapping_info.end,
                phys_addr: start.get().into(),
                rw: ReadWrite::Write,
                us: EntryMode::Supervisor,
                pwt: PageWriteThroughLevel::WriteThrough,
                pcd: false,
            })?;

            // assert_eq!(
            //     paging::calc_virt_addr(start.get().into()).unwrap().get(),
            //     start.get()
            // );
        }

        Ok(())
    }

    fn remap_virt_addr(&self) -> Result<()> {
        for (_, mapping_info) in self.program_mem_info.iter() {
            paging::update_mapping(mapping_info)?;
        }

        Ok(())
    }

    fn switch_to(&self, next_task: &Task) {
        kdebug!(
            "task: Switch context tid: {} to {}",
            self.id.get(),
            next_task.id.get()
        );

        self.context.switch_to(&next_task.context);
    }
}

pub fn exec_user_task(
    elf64: Elf64,
    path: &Path,
    args: &[&str],
    dwarf: Option<Dwarf>,
) -> Result<i32> {
    let kernel_task = unsafe { KERNEL_TASK.get_force_mut() };
    let user_tasks = unsafe { USER_TASKS.get_force_mut() };

    if kernel_task.is_none() {
        // stack is unused, because already allocated static area for kernel stack
        *kernel_task = Some(Task::new(0, None, None, ContextMode::Kernel, None)?);
    }

    let is_user = !user_tasks.is_empty();
    if is_user {
        user_tasks.last().unwrap().unmap_virt_addr()?;
    }

    let user_task = Task::new(
        USER_TASK_STACK_SIZE,
        Some(elf64),
        Some(&[&[path.to_string().as_str()], args].concat()),
        ContextMode::User,
        dwarf,
    );

    let task = match user_task {
        Ok(task) => task,
        Err(e) => {
            if is_user {
                user_tasks.last().unwrap().remap_virt_addr()?;
            }
            return Err(e);
        }
    };

    // debug_task(&task);
    user_tasks.push(task);

    let is_user = user_tasks.len() > 1;
    let current_task = if is_user {
        user_tasks.get(user_tasks.len() - 2).unwrap()
    } else {
        kernel_task.as_ref().unwrap()
    };

    current_task.switch_to(user_tasks.last().unwrap());

    // returned
    drop(user_tasks.pop().unwrap());
    if let Some(task) = user_tasks.last() {
        task.remap_virt_addr()?;
    }

    // get exit status
    let exit_status = unsafe {
        let status = match USER_EXIT_STATUS {
            Some(s) => s,
            None => panic!("task: User exit status was not found"),
        };
        USER_EXIT_STATUS = None;
        status
    };

    Ok(exit_status)
}

pub fn push_allocated_mem_frame_info_for_user_task(mem_frame_info: MemoryFrameInfo) -> Result<()> {
    let user_task = unsafe { USER_TASKS.get_force_mut() }
        .iter_mut()
        .last()
        .unwrap();
    user_task.allocated_mem_frame_info.push(mem_frame_info);

    Ok(())
}

pub fn get_memory_frame_size_by_virt_addr(virt_addr: VirtualAddress) -> Result<Option<usize>> {
    let user_task = unsafe { USER_TASKS.get_force_mut() }
        .iter_mut()
        .last()
        .unwrap();

    for mem_frame_info in &user_task.allocated_mem_frame_info {
        if mem_frame_info.frame_start_virt_addr()? == virt_addr {
            return Ok(Some(mem_frame_info.frame_size));
        }
    }

    Ok(None)
}

pub fn push_layer_id(layer_id: LayerId) {
    let user_task = unsafe { USER_TASKS.get_force_mut() }
        .iter_mut()
        .last()
        .unwrap();

    user_task.created_layer_ids.push(layer_id);
}

pub fn remove_layer_id(layer_id: &LayerId) {
    let user_task = unsafe { USER_TASKS.get_force_mut() }
        .iter_mut()
        .last()
        .unwrap();

    user_task
        .created_layer_ids
        .retain(|cwd| cwd.get() != layer_id.get());
}

pub fn push_fd_num(fd_num: FileDescriptorNumber) {
    let user_task = unsafe { USER_TASKS.get_force_mut() }
        .iter_mut()
        .last()
        .unwrap();

    user_task.opend_fd_num.push(fd_num);
}

pub fn remove_fd_num(fd_num: &FileDescriptorNumber) {
    let user_task = unsafe { USER_TASKS.get_force_mut() }
        .iter_mut()
        .last()
        .unwrap();

    user_task
        .opend_fd_num
        .retain(|cfdn| cfdn.get() != fd_num.get());
}

pub fn return_task(exit_status: i32) {
    unsafe {
        USER_EXIT_STATUS = Some(exit_status);
    }

    let user_tasks = unsafe { USER_TASKS.get_force_mut() };
    let current_task = user_tasks.last().unwrap();

    let before_task;
    if let Some(before_task_i) = user_tasks.len().checked_sub(2) {
        before_task = user_tasks.get(before_task_i).unwrap();
    } else {
        before_task = unsafe { KERNEL_TASK.get_force_mut() }.as_ref().unwrap();
    }

    current_task.switch_to(before_task);

    unreachable!();
}

pub fn debug_user_task() {
    kdebug!("===USER TASK INFO===");
    let user_task = unsafe { USER_TASKS.get_force_mut() }.last();
    if let Some(task) = user_task {
        debug_task(task);
    } else {
        kdebug!("User task no available");
    }
}

pub fn get_running_user_task_dwarf() -> Option<Dwarf> {
    let user_task = unsafe { USER_TASKS.get_force_mut() }.last();
    if let Some(task) = user_task {
        task.dwarf.clone()
    } else {
        None
    }
}

pub fn is_running_user_task() -> bool {
    unsafe { USER_TASKS.get_force_mut() }.len() > 1
}

fn debug_task(task: &Task) {
    let ctx = &task.context;
    kdebug!("task id: {}", task.id.get());
    kdebug!(
        "stack: (phys)0x{:x}, size: 0x{:x}bytes",
        task.stack_mem_frame_info.frame_start_phys_addr.get(),
        task.stack_mem_frame_info.frame_size,
    );
    kdebug!("context:");
    kdebug!(
        "\tcr3: 0x{:016x}, rip: 0x{:016x}, rflags: {:?},",
        ctx.cr3,
        ctx.rip,
        ctx.rflags
    );
    kdebug!(
        "\tcs : 0x{:016x}, ss : 0x{:016x}, fs : 0x{:016x}, gs : 0x{:016x},",
        ctx.cs,
        ctx.ss,
        ctx.fs,
        ctx.gs
    );
    kdebug!(
        "\trax: 0x{:016x}, rbx: 0x{:016x}, rcx: 0x{:016x}, rdx: 0x{:016x},",
        ctx.rax,
        ctx.rbx,
        ctx.rcx,
        ctx.rdx
    );
    kdebug!(
        "\trdi: 0x{:016x}, rsi: 0x{:016x}, rsp: 0x{:016x}, rbp: 0x{:016x},",
        ctx.rdi,
        ctx.rsi,
        ctx.rsp,
        ctx.rbp
    );
    kdebug!(
        "\tr8 : 0x{:016x}, r9 : 0x{:016x}, r10: 0x{:016x}, r11: 0x{:016x},",
        ctx.r8,
        ctx.r9,
        ctx.r10,
        ctx.r11
    );
    kdebug!(
        "\tr12: 0x{:016x}, r13: 0x{:016x}, r14: 0x{:016x}, r15: 0x{:016x}",
        ctx.r12,
        ctx.r13,
        ctx.r14,
        ctx.r15
    );

    kdebug!("args mem frame info:");
    if let Some(mem_frame_info) = &task.args_mem_frame_info {
        let virt_addr = mem_frame_info.frame_start_virt_addr().unwrap();
        kdebug!(
            "\t(virt)0x{:x}-0x{:x}",
            virt_addr.get(),
            virt_addr.offset(mem_frame_info.frame_size).get(),
        );
    }

    kdebug!("stack mem frame info:");
    let virt_addr = task.stack_mem_frame_info.frame_start_virt_addr().unwrap();
    kdebug!(
        "\t(virt)0x{:x}-0x{:x}",
        virt_addr.get(),
        virt_addr.offset(task.stack_mem_frame_info.frame_size).get(),
    );

    kdebug!("program mem frame info:");
    for (mem_frame_info, mapping_info) in &task.program_mem_info {
        let virt_addr = mem_frame_info.frame_start_virt_addr().unwrap();
        kdebug!(
            "\t(virt)0x{:x}-0x{:x} mapped to (virt)0x{:x}-0x{:x}",
            virt_addr.get(),
            virt_addr.offset(mem_frame_info.frame_size).get(),
            mapping_info.start.get(),
            mapping_info.end.get(),
        );
    }

    kdebug!("allocated mem frame info:");
    for mem_frame_info in &task.allocated_mem_frame_info {
        let virt_addr = mem_frame_info.frame_start_virt_addr().unwrap();

        kdebug!(
            "\t(virt)0x{:x}-0x{:x}",
            virt_addr.get(),
            virt_addr.offset(mem_frame_info.frame_size).get(),
        );
    }
}
