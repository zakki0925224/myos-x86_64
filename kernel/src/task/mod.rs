use crate::{
    arch::{
        x86_64::context::{Context, ContextMode},
        VirtualAddress,
    },
    debug::dwarf::Dwarf,
    error::Result,
    fs::vfs::{self, *},
    graphics::{multi_layer::LayerId, simple_window_manager},
    kdebug,
    mem::{
        bitmap::{self, MemoryFrameInfo},
        paging::{self, *},
    },
    util,
};
use alloc::vec::Vec;
use common::elf::{self, Elf64};
use core::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

pub mod async_task;
pub mod multi_scheduler;
pub mod single_scheduler;
pub mod syscall;

pub const USER_TASK_STACK_SIZE: usize = 1024 * 1024; // 1MiB

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(usize);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TaskId {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn new_val(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
pub enum TaskRequest {
    PushLayerId(LayerId),
    RemoveLayerId(LayerId),
    PushFileDescriptorNumber(FileDescriptorNumber),
    RemoveFileDescriptorNumber(FileDescriptorNumber),
    PushMemory(MemoryFrameInfo),
    GetMemoryFrameSize(VirtualAddress),
    ExecuteDebugger,
    GetDwarf,
}

#[derive(Debug, Clone)]
pub enum TaskResult {
    Ok,
    MemoryFrameSize(Option<usize>),
    ExecuteDebugger(bool),
    Dwarf(Option<Dwarf>),
}

#[derive(Debug, Clone)]
struct TaskResource {
    args_mem_frame_info: Option<MemoryFrameInfo>,
    stack_mem_frame_info: Option<MemoryFrameInfo>,
    program_mem_info: Vec<(MemoryFrameInfo, MappingInfo)>,
    allocated_mem_frame_info: Vec<MemoryFrameInfo>,
    created_layer_ids: Vec<LayerId>,
    opend_fd_num: Vec<FileDescriptorNumber>,
}

impl Drop for TaskResource {
    fn drop(&mut self) {
        if let Some(args_mem_frame_info) = self.args_mem_frame_info {
            args_mem_frame_info.set_permissions_to_supervisor().unwrap();
            bitmap::dealloc_mem_frame(args_mem_frame_info).unwrap();
        }

        if let Some(stack_mem_frame_info) = self.stack_mem_frame_info {
            stack_mem_frame_info
                .set_permissions_to_supervisor()
                .unwrap();
            bitmap::dealloc_mem_frame(stack_mem_frame_info).unwrap();
        }

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
            let _ = simple_window_manager::remove_component(*layer_id);
        }

        // close all opend files
        for fd in self.opend_fd_num.iter() {
            vfs::close_file(*fd).unwrap();
        }
    }
}

impl TaskResource {
    fn new(
        args_mem_frame_info: Option<MemoryFrameInfo>,
        stack_mem_frame_info: Option<MemoryFrameInfo>,
        program_mem_info: Vec<(MemoryFrameInfo, MappingInfo)>,
    ) -> Self {
        Self {
            args_mem_frame_info,
            stack_mem_frame_info,
            program_mem_info,
            allocated_mem_frame_info: Vec::new(),
            created_layer_ids: Vec::new(),
            opend_fd_num: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    Running,
    Ready,
    Sleeping,
    Exited(i32),
}

impl TaskState {
    const fn default() -> Self {
        Self::Ready
    }
}

#[derive(Debug, Clone)]
struct Task {
    id: TaskId,
    state: TaskState,
    context: Context,
    resource: TaskResource,
    dwarf: Option<Dwarf>,
}

impl Drop for Task {
    fn drop(&mut self) {
        kdebug!("task: Dropped tid: {}", self.id);
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
                return Err("The file is not an executable file".into());
            }

            if header.machine() != elf::Machine::X8664 {
                return Err("Unsupported ISA".into());
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
        let stack_mem_frame_info = if stack_size > 0 {
            let stack = bitmap::alloc_mem_frame(stack_size.div_ceil(PAGE_SIZE).max(1))?;
            match mode {
                ContextMode::Kernel => stack.set_permissions_to_supervisor()?,
                ContextMode::User => stack.set_permissions_to_user()?,
            }

            Some(stack)
        } else {
            None
        };

        let rsp = if let Some(stack) = stack_mem_frame_info.as_ref() {
            (stack.frame_start_virt_addr()?.get() + stack_size as u64 - 63) & !63
        } else {
            0
        };
        assert!(rsp % 64 == 0); // must be 64 bytes align for SSE and AVX instructions, etc.

        // args
        let mut args_mem_frame_info = None;
        let mut arg0 = 0; // args len
        let mut arg1 = 0; // args virt addr
        if let Some(args) = args {
            let mut c_args = Vec::new();
            for arg in args {
                c_args.extend(util::cstring::into_cstring_bytes_with_nul(arg));
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
            state: TaskState::default(),
            context,
            resource: TaskResource::new(
                args_mem_frame_info,
                stack_mem_frame_info,
                program_mem_info,
            ),
            dwarf,
        })
    }

    fn unmap_virt_addr(&self) -> Result<()> {
        for (_, mapping_info) in self.resource.program_mem_info.iter() {
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
        for (_, mapping_info) in self.resource.program_mem_info.iter() {
            paging::update_mapping(mapping_info)?;
        }

        Ok(())
    }

    fn switch_to(&self, next_task: &Task) {
        kdebug!("task: Switch context tid: {} to {}", self.id, next_task.id);

        self.context.switch_to(&next_task.context);
    }
}

pub fn debug_task(task: &Task) {
    let ctx = &task.context;
    kdebug!("task id: {}", task.id);

    if let Some(stack) = task.resource.stack_mem_frame_info {
        kdebug!(
            "stack: (phys)0x{:x}, size: 0x{:x}bytes",
            stack.frame_start_phys_addr.get(),
            stack.frame_size,
        );
    }

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
    if let Some(mem_frame_info) = &task.resource.args_mem_frame_info {
        let virt_addr = mem_frame_info.frame_start_virt_addr().unwrap();
        kdebug!(
            "\t(virt)0x{:x}-0x{:x}",
            virt_addr.get(),
            virt_addr.offset(mem_frame_info.frame_size).get(),
        );
    }

    if let Some(stack) = task.resource.stack_mem_frame_info {
        kdebug!("stack mem frame info:");
        let virt_addr = stack.frame_start_virt_addr().unwrap();
        kdebug!(
            "\t(virt)0x{:x}-0x{:x}",
            virt_addr.get(),
            virt_addr.offset(stack.frame_size).get(),
        );
    }

    kdebug!("program mem frame info:");
    for (mem_frame_info, mapping_info) in &task.resource.program_mem_info {
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
    for mem_frame_info in &task.resource.allocated_mem_frame_info {
        let virt_addr = mem_frame_info.frame_start_virt_addr().unwrap();

        kdebug!(
            "\t(virt)0x{:x}-0x{:x}",
            virt_addr.get(),
            virt_addr.offset(mem_frame_info.frame_size).get(),
        );
    }
}
