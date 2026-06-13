use crate::{
    arch::{x86_64::context::ContextMode, VirtualAddress},
    debug::dwarf::Dwarf,
    error::{Error, Result},
    fs::{path::Path, vfs::FileDescriptorNumber},
    graphics::multi_layer::LayerId,
    kdebug,
    mem::bitmap::MemoryFrameInfo,
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
        task.unmap_virt_addr().unwrap();
        if let Some(current) = &self.current_task {
            current.remap_virt_addr().unwrap();
        }

        self.ready_queue.push_back(Box::new(task));
    }

    fn pick_next_task(&mut self) -> Option<(*const Task, *const Task)> {
        let prev_task = self.current_task.take()?;

        if let Some(next_task) = self.ready_queue.pop_front() {
            prev_task.unmap_virt_addr().unwrap();
            next_task.remap_virt_addr().unwrap();

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

        current.unmap_virt_addr().unwrap();
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
        next_task.remap_virt_addr().unwrap();

        self.current_task = Some(next_task);

        let prev_ptr = &**self.exited_tasks.last().unwrap() as *const Task;
        let next_ptr = &**self.current_task.as_ref().unwrap() as *const Task;

        (prev_ptr, next_ptr, old)
    }

    fn sleep_current_waiting_for(&mut self, child_id: TaskId) -> (*const Task, *const Task) {
        let mut current = self.current_task.take().expect("No current task to sleep");
        current.waiting_for = Some(child_id);
        current.state = TaskState::Sleeping;
        current.unmap_virt_addr().unwrap();

        self.sleeping_tasks.push(current);

        let next_task = self
            .ready_queue
            .pop_front()
            .expect("No task to run after sleep");
        next_task.remap_virt_addr().unwrap();

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
    let (prev, next) = TASK_SCHED.spin_lock().sleep_current_waiting_for(child_id);
    unsafe {
        (*prev).switch_to(&*next);
    }
}

pub fn sched() {
    let (switch_pair, stale) = {
        let mut s = TASK_SCHED.spin_lock();
        let pair = s.pick_next_task();
        let stale = core::mem::take(&mut s.exited_tasks);
        (pair, stale)
    };

    // drop old exited tasks outside the lock
    drop(stale);

    if let Some((prev, next)) = switch_pair {
        unsafe { (*prev).switch_to(&*next) };
    } else {
        panic!("No next task!")
    }
}

pub fn current() -> Option<&'static Task> {
    unsafe {
        let ptr = TASK_SCHED.spin_lock().current_task.as_deref()? as *const Task;
        Some(&*ptr)
    }
}

pub fn exit_current(exit_code: i32) -> ! {
    let (prev, next, old) = TASK_SCHED.spin_lock().pick_next_task_on_exit(exit_code);
    // drop old exited tasks outside the lock
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

pub fn add_mem_frame_info(mem_frame_info: MemoryFrameInfo) -> Result<()> {
    let mut s = TASK_SCHED.spin_lock();
    s.current_task_mut()?
        .resource
        .alloc_frames
        .push(mem_frame_info);
    Ok(())
}

pub fn mem_frame_size(virt_addr: VirtualAddress) -> Result<Option<usize>> {
    let mut s = TASK_SCHED.spin_lock();
    let task = s.current_task_mut()?;
    for mem_frame_info in &task.resource.alloc_frames {
        if mem_frame_info.frame_start_virt_addr()? == virt_addr {
            return Ok(Some(mem_frame_info.frame_size));
        }
    }
    Ok(None)
}

pub fn remove_mem_frame_info(virt_addr: VirtualAddress) -> Result<MemoryFrameInfo> {
    let mut s = TASK_SCHED.spin_lock();
    let allocated = &mut s.current_task_mut()?.resource.alloc_frames;
    if let Some(index) = allocated
        .iter()
        .position(|info| info.frame_start_virt_addr().ok() == Some(virt_addr))
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
