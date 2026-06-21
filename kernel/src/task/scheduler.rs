use crate::{
    arch::{
        x86_64::{
            context::{Context, ContextMode, InterruptedContext},
            paging::{PageWriteThroughLevel, ReadWrite},
            registers::{Cr3, Register, Rflags},
        },
        VirtualAddress,
    },
    debug::dwarf::Dwarf,
    error::{Error, Result},
    fs::{path::Path, vfs::FileDescriptorNumber},
    graphics::multi_layer::LayerId,
    mem::bitmap::MemoryFrame,
    sync::mutex::Mutex,
    task::*,
};
use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    string::ToString,
    vec::Vec,
};

static TASK_SCHED: Mutex<TaskScheduler> = Mutex::new(TaskScheduler::new());

struct TaskScheduler {
    ready_queue: VecDeque<Box<Task>>,
    current_task: Option<Box<Task>>,
    exited_tasks: Vec<Box<Task>>,
    sleeping_tasks: Vec<Box<Task>>,
    exit_codes: BTreeMap<TaskId, i32>,
}

impl TaskScheduler {
    const fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            current_task: None,
            exited_tasks: Vec::new(),
            sleeping_tasks: Vec::new(),
            exit_codes: BTreeMap::new(),
        }
    }

    fn current_task_mut(&mut self) -> Result<&mut Task> {
        self.current_task
            .as_mut()
            .map(|t| t.as_mut())
            .ok_or(Error::NotInitialized.with_context("current task"))
    }

    fn spawn(&mut self, task: Task) {
        self.ready_queue.push_back(Box::new(task));
    }

    fn pick_next_task(&mut self) -> Option<(*const Task, *const Task)> {
        let prev_task = self.current_task.take()?;

        if let Some(next_task) = self.ready_queue.pop_front() {
            self.ready_queue.push_back(prev_task);
            self.current_task = Some(next_task);

            let prev_ptr = &**self.ready_queue.back().unwrap() as *const Task;
            let next_ptr = &**self.current_task.as_ref().unwrap() as *const Task;

            Some((prev_ptr, next_ptr))
        } else {
            self.current_task = Some(prev_task);
            None
        }
    }

    fn pick_next_task_on_exit(
        &mut self,
        exit_code: i32,
    ) -> (*const Task, *const Task, Vec<Box<Task>>) {
        let mut current = self.current_task.take().expect("No current task to exit");
        let exiting_id = current.id;

        current.state = TaskState::Exited(exit_code);

        let old = core::mem::take(&mut self.exited_tasks);
        self.exited_tasks.push(current);
        self.exit_codes.insert(exiting_id, exit_code);

        if let Some(i) = self
            .sleeping_tasks
            .iter()
            .position(|t| t.waiting_for == Some(exiting_id))
        {
            let mut waiter = self.sleeping_tasks.remove(i);
            waiter.state = TaskState::Ready;
            waiter.waiting_for = None;
            self.ready_queue.push_front(waiter);
        }

        let next_task = self
            .ready_queue
            .pop_front()
            .expect("No task to run after exit");
        self.current_task = Some(next_task);

        let prev_ptr = &**self.exited_tasks.last().unwrap() as *const Task;
        let next_ptr = &**self.current_task.as_ref().unwrap() as *const Task;

        (prev_ptr, next_ptr, old)
    }

    fn sleep_current_waiting_for(&mut self, child_id: TaskId) -> (*const Task, *const Task) {
        let mut current = self.current_task.take().expect("No current task to sleep");
        current.waiting_for = Some(child_id);
        current.state = TaskState::Sleeping;
        self.sleeping_tasks.push(current);

        let next_task = self
            .ready_queue
            .pop_front()
            .expect("No task to run after sleep");
        self.current_task = Some(next_task);

        let prev_ptr = &**self.sleeping_tasks.last().unwrap() as *const Task;
        let next_ptr = &**self.current_task.as_ref().unwrap() as *const Task;

        (prev_ptr, next_ptr)
    }
}

pub fn init() -> Result<()> {
    let kernel_task = Task::new(0, None, None, ContextMode::Kernel, None, [None, None, None])?;
    TASK_SCHED.spin_lock().current_task = Some(Box::new(kernel_task));
    Ok(())
}

pub fn spawn(task: Task) {
    TASK_SCHED.spin_lock().spawn(task)
}

pub fn spawn_user_task(
    elf64: Elf64,
    path: &Path,
    args: &[&str],
    dwarf: Option<Dwarf>,
    pipe_fd: [Option<FileDescriptorNumber>; 3],
) -> Result<TaskId> {
    let path_string = path.to_string();
    let all_args: Vec<&str> = [&[path_string.as_str()], args].concat();
    let task = Task::new(
        super::USER_TASK_STACK_SIZE,
        Some(elf64),
        Some(&all_args),
        ContextMode::User,
        dwarf,
        pipe_fd,
    )?;

    let id = task.id;
    TASK_SCHED.spin_lock().spawn(task);
    Ok(id)
}

pub fn sleep_waiting_for(child_id: TaskId) {
    let saved = Rflags::read_with_cli();
    let (prev, next) = TASK_SCHED.spin_lock().sleep_current_waiting_for(child_id);
    unsafe {
        (*prev).switch_to(&*next);
    }
    saved.write();
}

pub fn sched() {
    let saved = Rflags::read_with_cli();

    let (switch_pair, stale) = {
        let mut s = TASK_SCHED.spin_lock();
        let pair = s.pick_next_task();
        let stale = core::mem::take(&mut s.exited_tasks);
        (pair, stale)
    };

    drop(stale);

    if let Some((prev, next)) = switch_pair {
        unsafe { (*prev).switch_to(&*next) };
    } else {
        saved.write();
        panic!("No next task!")
    }

    saved.write();
}

pub fn current() -> Option<&'static Task> {
    unsafe {
        let ptr = TASK_SCHED.spin_lock().current_task.as_deref()? as *const Task;
        Some(&*ptr)
    }
}

pub fn exit_current(exit_code: i32) -> ! {
    Rflags::read_with_cli();
    let (prev, next, old) = TASK_SCHED.spin_lock().pick_next_task_on_exit(exit_code);
    drop(old);

    unsafe {
        (*prev).switch_to(&*next);
    }

    unreachable!();
}

pub fn take_exit_code(id: TaskId) -> Option<i32> {
    TASK_SCHED.spin_lock().exit_codes.remove(&id)
}

pub fn add_layer_id(layer_id: LayerId) -> Result<()> {
    let mut s = TASK_SCHED.spin_lock();
    s.current_task_mut()?
        .resource
        .created_layer_ids
        .push(layer_id);
    Ok(())
}

pub fn remove_layer_id(layer_id: LayerId) -> Result<()> {
    let mut s = TASK_SCHED.spin_lock();
    s.current_task_mut()?
        .resource
        .created_layer_ids
        .retain(|id| *id != layer_id);
    Ok(())
}

pub fn add_fd_num(fd_num: FileDescriptorNumber) -> Result<()> {
    let mut s = TASK_SCHED.spin_lock();
    s.current_task_mut()?.resource.fd_nums.push(fd_num);
    Ok(())
}

pub fn remove_fd_num(fd_num: FileDescriptorNumber) -> Result<()> {
    let mut s = TASK_SCHED.spin_lock();
    s.current_task_mut()?
        .resource
        .fd_nums
        .retain(|fd| *fd != fd_num);
    Ok(())
}

pub fn add_mem_frame(mem_frame: MemoryFrame) -> Result<()> {
    let mut s = TASK_SCHED.spin_lock();
    s.current_task_mut()?.resource.alloc_frames.push(mem_frame);
    Ok(())
}

pub fn map_current_user_page(frame: &MemoryFrame) -> Result<()> {
    let mut s = TASK_SCHED.spin_lock();
    let task = s.current_task_mut()?;
    let phys = frame.frame_start_phys_addr();
    let start: VirtualAddress = phys.into();
    let end = start.offset(frame.frame_size());
    task.resource.page_table.map(
        start,
        end,
        phys,
        ReadWrite::Write,
        PageWriteThroughLevel::WriteThrough,
        false,
    )?;
    Ok(())
}

pub fn unmap_current_user_page(frame: &MemoryFrame) -> Result<()> {
    let mut s = TASK_SCHED.spin_lock();
    let task = s.current_task_mut()?;
    let start: VirtualAddress = frame.frame_start_phys_addr().into();
    let end = start.offset(frame.frame_size());
    unsafe { task.resource.page_table.unmap(start, end) };
    Ok(())
}

pub fn mem_frame_size(virt_addr: VirtualAddress) -> Result<Option<usize>> {
    let mut s = TASK_SCHED.spin_lock();
    let task = s.current_task_mut()?;
    for mem_frame in &task.resource.alloc_frames {
        if mem_frame.frame_start_virt_addr() == virt_addr {
            return Ok(Some(mem_frame.frame_size()));
        }
    }
    Ok(None)
}

pub fn remove_mem_frame(virt_addr: VirtualAddress) -> Result<MemoryFrame> {
    let mut s = TASK_SCHED.spin_lock();
    let allocated = &mut s.current_task_mut()?.resource.alloc_frames;
    if let Some(index) = allocated
        .iter()
        .position(|info| info.frame_start_virt_addr() == virt_addr)
    {
        return Ok(allocated.remove(index));
    }
    Err(Error::InvalidData.with_context("virtual address"))
}

pub fn debug_current() -> bool {
    let s = TASK_SCHED.spin_lock();
    if let Some(task) = s.current_task.as_ref() {
        super::debug_task(task);
        true
    } else {
        false
    }
}

pub fn current_dwarf() -> Option<Dwarf> {
    TASK_SCHED.spin_lock().current_task.as_ref()?.dwarf.clone()
}

pub fn current_pipe_fd() -> Option<[Option<FileDescriptorNumber>; 3]> {
    let sched = TASK_SCHED.spin_lock();
    let task = sched.current_task.as_ref()?;
    Some(task.resource.pipe_fd)
}

pub fn preempt_sched(interrupted: &InterruptedContext) -> *const Context {
    let (pair, stale) = {
        let mut s = TASK_SCHED.spin_lock();

        if let Some(current) = s.current_task.as_mut() {
            let ctx = &mut current.context;
            ctx.rip = interrupted.rip;
            ctx.rflags.set_raw(interrupted.rflags);
            ctx.cs = interrupted.cs;
            ctx.ss = interrupted.ss;
            ctx.rsp = interrupted.rsp;
            ctx.rax = interrupted.rax;
            ctx.rbx = interrupted.rbx;
            ctx.rcx = interrupted.rcx;
            ctx.rdx = interrupted.rdx;
            ctx.rdi = interrupted.rdi;
            ctx.rsi = interrupted.rsi;
            ctx.rbp = interrupted.rbp;
            ctx.r8 = interrupted.r8;
            ctx.r9 = interrupted.r9;
            ctx.r10 = interrupted.r10;
            ctx.r11 = interrupted.r11;
            ctx.r12 = interrupted.r12;
            ctx.r13 = interrupted.r13;
            ctx.r14 = interrupted.r14;
            ctx.r15 = interrupted.r15;
            ctx.cr3 = Cr3::read().raw();

            let mut fs: u64 = 0;
            let mut gs: u64 = 0;
            unsafe {
                core::arch::asm!(
                    "mov {0:x}, fs",
                    "mov {1:x}, gs",
                    inout(reg) fs,
                    inout(reg) gs,
                    options(nostack, nomem),
                );
            }
            ctx.fs = fs;
            ctx.gs = gs;

            let fpu_ptr = ctx.fpu_context.as_mut_ptr();
            unsafe {
                core::arch::asm!(
                    "fxsave64 [{0}]",
                    in(reg) fpu_ptr,
                    options(nostack),
                );
            }
        }

        let pair = s.pick_next_task();
        let stale = core::mem::take(&mut s.exited_tasks);
        (pair, stale)
    };

    drop(stale);

    match pair {
        Some((_, next)) => unsafe { &(*next).context as *const Context },
        None => core::ptr::null(),
    }
}

#[test_case]
fn test_multitask_scheduler_round_robin() {
    let mut sched = TaskScheduler::new();
    let kernel_task =
        Task::new(0, None, None, ContextMode::Kernel, None, [None, None, None]).unwrap();
    sched.current_task = Some(Box::new(kernel_task));

    // current: KernelTask(TID: 0)
    // ReadyQueue: []

    let t1 = Task::new(0, None, None, ContextMode::Kernel, None, [None, None, None]).unwrap();
    let t1_id = t1.id;
    sched.spawn(t1);

    let t2 = Task::new(0, None, None, ContextMode::Kernel, None, [None, None, None]).unwrap();
    let t2_id = t2.id;
    sched.spawn(t2);

    // ReadyQueue: [T1, T2]
    // current: KernelTask

    let (prev_ptr, next_ptr) = sched.pick_next_task().expect("Sched 1 failed");

    unsafe {
        let prev = &*prev_ptr;
        let next = &*next_ptr;

        assert_ne!(prev.id, next.id);
        assert_eq!(next.id, t1_id);
    }

    // ReadyQueue: [T2, KernelTask]
    // current: T1

    let (prev_ptr, next_ptr) = sched.pick_next_task().expect("Sched 2 failed");

    unsafe {
        let prev = &*prev_ptr; // T1
        let next = &*next_ptr; // T2

        assert_eq!(prev.id, t1_id);
        assert_eq!(next.id, t2_id);
    }

    // ReadyQueue: [KernelTask, T1]
    // current: T2

    let (prev_ptr, next_ptr) = sched.pick_next_task().expect("Sched 3 failed");

    unsafe {
        let prev = &*prev_ptr; // T2
        let next = &*next_ptr; // KernelTask

        assert_eq!(prev.id, t2_id);
        assert_ne!(next.id, t1_id);
        assert_ne!(next.id, t2_id);
    }
}

#[test_case]
fn test_multitask_scheduler_exit() {
    let mut sched = TaskScheduler::new();
    let kernel_task =
        Task::new(0, None, None, ContextMode::Kernel, None, [None, None, None]).unwrap();
    sched.current_task = Some(Box::new(kernel_task));

    let t1 = Task::new(0, None, None, ContextMode::Kernel, None, [None, None, None]).unwrap();
    let t1_id = t1.id;
    sched.spawn(t1);

    sched.pick_next_task();

    if let Some(current) = &sched.current_task {
        assert_eq!(current.id, t1_id);
    } else {
        panic!("No current task");
    }

    let (prev_ptr, next_ptr, stale) = sched.pick_next_task_on_exit(123);

    unsafe {
        let prev = &*prev_ptr; // T1 (Exited)
        let next = &*next_ptr; // KernelTask (Next)

        assert_eq!(prev.id, t1_id);
        assert_eq!(prev.state, TaskState::Exited(123));

        assert!(sched.ready_queue.iter().all(|t| t.id != t1_id));
        assert_eq!(sched.exited_tasks.last().unwrap().id, t1_id);

        assert_ne!(next.id, t1_id);
    }
}
