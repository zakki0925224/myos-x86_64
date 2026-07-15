use crate::{
    arch::{
        x86_64::{
            context::{Context, ContextMode},
            paging::{PageWriteThroughLevel, ReadWrite, UserPageTable, PAGE_SIZE},
            registers::{Cr3, Register},
        },
        VirtualAddress,
    },
    debug::dwarf::Dwarf,
    error::{Error, Result},
    fs::{
        path::Path,
        vfs::{self, *},
    },
    graphics::{multi_layer::LayerId, window_manager},
    kdebug,
    mem::bitmap::{self, MemoryFrame},
    util,
};
use alloc::{string::String, vec::Vec};
use common::elf::{self, Elf64};
use core::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

pub mod async_task;
pub mod exec;
pub mod scheduler;
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
    pub const KERNEL: Self = Self(0);

    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

impl From<usize> for TaskId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
struct TaskResource {
    page_table: UserPageTable,
    args_frame: Option<MemoryFrame>,
    stack_frame: Option<MemoryFrame>,
    program_frames: Vec<MemoryFrame>,
    alloc_frames: Vec<MemoryFrame>,
    created_layer_ids: Vec<LayerId>,
    fd_nums: Vec<FileDescriptorNumber>,
    pipe_fd: [Option<FileDescriptorNumber>; 3],
}

impl Drop for TaskResource {
    fn drop(&mut self) {
        if let Some(args_frame) = self.args_frame.take() {
            bitmap::dealloc_mem_frame(args_frame).unwrap();
        }

        if let Some(stack_frame) = self.stack_frame.take() {
            bitmap::dealloc_mem_frame(stack_frame).unwrap();
        }

        for frame in self.program_frames.drain(..) {
            bitmap::dealloc_mem_frame(frame).unwrap();
        }

        for frame in self.alloc_frames.drain(..) {
            bitmap::dealloc_mem_frame(frame).unwrap();
        }

        // destroy all created windows
        for layer_id in self.created_layer_ids.iter() {
            let _ = window_manager::remove_component(*layer_id);
        }

        // close all opened files
        for fd in self.fd_nums.iter() {
            vfs::close_file(*fd).unwrap();
        }
    }
}

impl TaskResource {
    fn new(
        page_table: UserPageTable,
        args_frame: Option<MemoryFrame>,
        stack_frame: Option<MemoryFrame>,
        program_frames: Vec<MemoryFrame>,
        pipe_fd: [Option<FileDescriptorNumber>; 3],
    ) -> Self {
        Self {
            page_table,
            args_frame,
            stack_frame,
            program_frames,
            alloc_frames: Vec::new(),
            created_layer_ids: Vec::new(),
            fd_nums: Vec::new(),
            pipe_fd,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Ready,
    Sleeping,
    Exited(i32),
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => write!(f, "Running"),
            Self::Ready => write!(f, "Ready"),
            Self::Sleeping => write!(f, "Sleeping"),
            Self::Exited(code) => write!(f, "Exited({})", code),
        }
    }
}

impl TaskState {
    const fn default() -> Self {
        Self::Ready
    }
}

pub struct TaskSnapshot {
    pub id: TaskId,
    pub name: String,
    pub state: TaskState,
    pub parent: Option<TaskId>,
}

#[derive(Debug)]
struct Task {
    id: TaskId,
    name: String,
    state: TaskState,
    context: Context,
    resource: TaskResource,
    dwarf: Option<Dwarf>,
    waiting_for: Option<TaskId>,
    parent: Option<TaskId>,
    children: Vec<TaskId>,
}

impl Drop for Task {
    fn drop(&mut self) {
        // kdebug!("task: Dropped tid: {}", self.id);
    }
}

impl Task {
    fn new(
        parent: Option<TaskId>,
        stack_size: usize, // 4KiB align
        elf64: Option<Elf64>,
        args: Option<&[&str]>, // file name + args
        mode: ContextMode,
        dwarf: Option<Dwarf>,
        pipe_fd: [Option<FileDescriptorNumber>; 3],
    ) -> Result<Self> {
        let mut user_page_table = match mode {
            ContextMode::User => UserPageTable::new_cloned_from_kernel()?,
            ContextMode::Kernel => UserPageTable::new()?,
        };

        // parse ELF
        let mut entry = None;
        let mut program_frames = Vec::new();
        if let Some(elf64) = elf64 {
            let header = elf64.header();

            if header.elf_type() != elf::Type::Executable {
                return Err(Error::InvalidData.with_context("ELF type"));
            }

            if header.machine() != elf::Machine::X8664 {
                return Err(Error::InvalidData.with_context("ELF machine"));
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
                let user_mem_frame = bitmap::alloc_mem_frame(pages_needed)?;
                user_mem_frame.zero_out()?;
                let user_mem_frame_start_virt_addr = user_mem_frame.frame_start_virt_addr();

                // copy data
                let program_data = elf64.data_by_program_header(program_header);
                if let Some(data) = program_data {
                    unsafe {
                        user_mem_frame_start_virt_addr
                            .offset(p_virt_addr as usize % PAGE_SIZE)
                            .as_ptr_mut::<u8>()
                            .copy_from_nonoverlapping(data.as_ptr(), p_file_size as usize);
                    }
                }

                // map into user page table at ELF virtual address
                let start_virt_addr: VirtualAddress =
                    (p_virt_addr / PAGE_SIZE as u64 * PAGE_SIZE as u64).into();
                user_page_table.map(
                    start_virt_addr,
                    start_virt_addr.offset(user_mem_frame.frame_size()),
                    user_mem_frame.frame_start_phys_addr(),
                    ReadWrite::Write,
                    PageWriteThroughLevel::WriteThrough,
                    false,
                )?;
                program_frames.push(user_mem_frame);

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
        let stack_frame = if stack_size > 0 {
            let stack = bitmap::alloc_mem_frame(stack_size.div_ceil(PAGE_SIZE).max(1))?;
            if mode == ContextMode::User {
                let phys = stack.frame_start_phys_addr();
                let start: VirtualAddress = phys.into();
                user_page_table.map(
                    start,
                    start.offset(stack.frame_size()),
                    phys,
                    ReadWrite::Write,
                    PageWriteThroughLevel::WriteThrough,
                    false,
                )?;
            }
            Some(stack)
        } else {
            None
        };

        let rsp = if let Some(stack) = stack_frame.as_ref() {
            (stack.frame_start_virt_addr().get() + stack_size as u64 - 63) & !63
        } else {
            0
        };
        assert!(rsp % 64 == 0); // must be 64 bytes align for SSE and AVX instructions, etc.

        // args
        let mut args_frame = None;
        let mut arg0 = 0; // args len
        let mut arg1 = 0; // args virt addr
        if let Some(args) = args {
            let mut c_args = Vec::new();
            for arg in args {
                c_args.extend(util::cstring::into_cstring_bytes_with_nul(arg));
            }

            let mut c_args_offset = (args.len() + 2) * 8;
            let mem_frame =
                bitmap::alloc_mem_frame(((c_args.len() + c_args_offset) / PAGE_SIZE).max(1))?;
            mem_frame.zero_out()?;
            if mode == ContextMode::User {
                let phys = mem_frame.frame_start_phys_addr();
                let start: VirtualAddress = phys.into();
                user_page_table.map(
                    start,
                    start.offset(mem_frame.frame_size()),
                    phys,
                    ReadWrite::Write,
                    PageWriteThroughLevel::WriteThrough,
                    false,
                )?;
            }

            let args_mem_virt_addr = mem_frame.frame_start_virt_addr();
            unsafe {
                args_mem_virt_addr
                    .offset(c_args_offset)
                    .as_ptr_mut::<u8>()
                    .copy_from_nonoverlapping(c_args.as_ptr(), c_args.len());
            }

            let mut c_args_ref = Vec::new();
            for arg in args {
                c_args_ref.push(args_mem_virt_addr.offset(c_args_offset).get());
                c_args_offset += arg.len() + 1;
            }
            unsafe {
                args_mem_virt_addr
                    .as_ptr_mut::<u64>()
                    .copy_from_nonoverlapping(c_args_ref.as_ptr(), c_args_ref.len());
            }

            args_frame = Some(mem_frame);
            arg0 = args.len() as u64;
            arg1 = args_mem_virt_addr.get();
        }

        let name = Path::new(args.unwrap_or(&["/kernel"])[0]).name();

        // context
        let cr3 = match mode {
            ContextMode::User => user_page_table.pml4_phys_addr(),
            ContextMode::Kernel => Cr3::read().raw(),
        };
        let mut context = Context::new();
        context.init(rip, arg0, arg1, rsp, mode, dwarf.is_some());
        context.cr3 = cr3;

        Ok(Self {
            id: TaskId::new(),
            name,
            state: TaskState::default(),
            context,
            resource: TaskResource::new(
                user_page_table,
                args_frame,
                stack_frame,
                program_frames,
                pipe_fd,
            ),
            dwarf,
            waiting_for: None,
            parent,
            children: Vec::new(),
        })
    }

    fn switch_to(&self, next_task: &Task) {
        // kdebug!("task: Switch context tid: {} to {}", self.id, next_task.id);

        self.context.switch_to(&next_task.context);
    }
}

pub fn debug_task(task: &Task) {
    let ctx = &task.context;
    kdebug!("task id: {}", task.id);

    if let Some(stack) = &task.resource.stack_frame {
        kdebug!(
            "stack: (phys){:#x}, size: {:#x}bytes",
            stack.frame_start_phys_addr(),
            stack.frame_size(),
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

    kdebug!("args frame:");
    if let Some(frame) = &task.resource.args_frame {
        let virt_addr = frame.frame_start_virt_addr();
        kdebug!(
            "\t(virt){:#x}-{:#x}",
            virt_addr.get(),
            virt_addr.offset(frame.frame_size()).get(),
        );
    }

    if let Some(stack) = &task.resource.stack_frame {
        kdebug!("stack frame:");
        let virt_addr = stack.frame_start_virt_addr();
        kdebug!(
            "\t(virt){:#x}-{:#x}",
            virt_addr.get(),
            virt_addr.offset(stack.frame_size()).get(),
        );
    }

    kdebug!("alloc frames:");
    for frame in &task.resource.alloc_frames {
        let virt_addr = frame.frame_start_virt_addr();

        kdebug!(
            "\t(virt){:#x}-{:#x}",
            virt_addr.get(),
            virt_addr.offset(frame.frame_size()).get(),
        );
    }
}
