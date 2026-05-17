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
use alloc::{boxed::Box, collections::vec_deque::VecDeque, string::ToString, vec::Vec};

static TASK_SCHED: Mutex<TaskScheduler> = Mutex::new(TaskScheduler::new());

struct TaskScheduler {
    ready_queue: VecDeque<Box<Task>>,
    running_task: Option<Box<Task>>,
    exited_tasks: Vec<Box<Task>>,
    sleeping_tasks: Vec<Box<Task>>,
}

impl TaskScheduler {
    const fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            running_task: None,
            exited_tasks: Vec::new(),
            sleeping_tasks: Vec::new(),
        }
    }

    fn init(&mut self) -> Result<()> {
        let kernel_task = Task::new(0, None, None, ContextMode::Kernel, None)?;
        self.running_task = Some(Box::new(kernel_task));
        Ok(())
    }

    fn current(&self) -> Option<&Task> {
        self.running_task.as_deref()
    }

    fn spawn(&mut self, task: Task) {
        task.unmap_virt_addr().unwrap();
        if let Some(current) = &self.running_task {
            current.remap_virt_addr().unwrap();
        }

        self.ready_queue.push_back(Box::new(task));
    }

    fn pick_next_task(&mut self) -> Option<(*const Task, *const Task)> {
        let prev_task = self.running_task.take()?;

        if let Some(next_task) = self.ready_queue.pop_front() {
            prev_task.unmap_virt_addr().unwrap();
            next_task.remap_virt_addr().unwrap();

            self.ready_queue.push_back(prev_task);
            self.running_task = Some(next_task);

            let prev_ptr = &**self.ready_queue.back().unwrap() as *const Task;
            let next_ptr = &**self.running_task.as_ref().unwrap() as *const Task;

            Some((prev_ptr, next_ptr))
        } else {
            self.running_task = Some(prev_task);
            None
        }
    }

    fn pick_next_task_on_exit(
        &mut self,
        exit_code: i32,
    ) -> (*const Task, *const Task, Vec<Box<Task>>) {
        let mut current = self
            .running_task
            .take()
            .expect("task: No running task to exit");
        let exiting_id = current.id;

        current.unmap_virt_addr().unwrap();

        kdebug!("task: Task exited with code: {}", exit_code);
        current.state = TaskState::Exited(exit_code);

        let old = core::mem::take(&mut self.exited_tasks);
        self.exited_tasks.push(current);

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

        self.running_task = Some(next_task);

        let prev_ptr = &**self.exited_tasks.last().unwrap() as *const Task;
        let next_ptr = &**self.running_task.as_ref().unwrap() as *const Task;

        (prev_ptr, next_ptr, old)
    }

    fn sleep_current_waiting_for(&mut self, child_id: TaskId) -> (*const Task, *const Task) {
        let mut current = self.running_task.take().expect("No running task to sleep");
        current.waiting_for = Some(child_id);
        current.state = TaskState::Sleeping;
        current.unmap_virt_addr().unwrap();

        self.sleeping_tasks.push(current);

        let next_task = self
            .ready_queue
            .pop_front()
            .expect("No task to run after sleep");
        next_task.remap_virt_addr().unwrap();

        self.running_task = Some(next_task);

        let prev_ptr = &**self.sleeping_tasks.last().unwrap() as *const Task;
        let next_ptr = &**self.running_task.as_ref().unwrap() as *const Task;

        (prev_ptr, next_ptr)
    }

    fn push_layer_id(&mut self, layer_id: LayerId) -> Result<()> {
        let task = self
            .running_task
            .as_mut()
            .ok_or(Error::NotInitialized.with_context("running task"))?;
        task.resource.created_layer_ids.push(layer_id);
        Ok(())
    }

    fn remove_layer_id(&mut self, layer_id: LayerId) -> Result<()> {
        let task = self
            .running_task
            .as_mut()
            .ok_or(Error::NotInitialized.with_context("running task"))?;
        task.resource.created_layer_ids.retain(|id| *id != layer_id);
        Ok(())
    }

    fn push_fd_num(&mut self, fd_num: FileDescriptorNumber) -> Result<()> {
        let task = self
            .running_task
            .as_mut()
            .ok_or(Error::NotInitialized.with_context("running task"))?;
        task.resource.opend_fd_num.push(fd_num);
        Ok(())
    }

    fn remove_fd_num(&mut self, fd_num: FileDescriptorNumber) -> Result<()> {
        let task = self
            .running_task
            .as_mut()
            .ok_or(Error::NotInitialized.with_context("running task"))?;
        task.resource.opend_fd_num.retain(|fd| *fd != fd_num);
        Ok(())
    }

    fn push_allocated_mem_frame_info(&mut self, mem_frame_info: MemoryFrameInfo) -> Result<()> {
        let task = self
            .running_task
            .as_mut()
            .ok_or(Error::NotInitialized.with_context("running task"))?;
        task.resource.allocated_mem_frame_info.push(mem_frame_info);
        Ok(())
    }

    fn get_memory_frame_size_by_virt_addr(
        &mut self,
        virt_addr: VirtualAddress,
    ) -> Result<Option<usize>> {
        let task = self
            .running_task
            .as_mut()
            .ok_or(Error::NotInitialized.with_context("running task"))?;
        for mem_frame_info in &task.resource.allocated_mem_frame_info {
            if mem_frame_info.frame_start_virt_addr()? == virt_addr {
                return Ok(Some(mem_frame_info.frame_size));
            }
        }
        Ok(None)
    }

    fn pop_allocated_memory_by_virt_addr(
        &mut self,
        virt_addr: VirtualAddress,
    ) -> Result<MemoryFrameInfo> {
        let task = self
            .running_task
            .as_mut()
            .ok_or(Error::NotInitialized.with_context("running task"))?;
        let allocated_mem_frame_info = &mut task.resource.allocated_mem_frame_info;

        if let Some(index) = allocated_mem_frame_info
            .iter()
            .position(|info| info.frame_start_virt_addr().ok() == Some(virt_addr))
        {
            return Ok(allocated_mem_frame_info.remove(index));
        }

        Err(Error::InvalidData.with_context("virtual address"))
    }

    fn show_running_task_debug(&self) -> bool {
        if let Some(task) = self.running_task.as_ref() {
            super::show_task_debug(task);
            true
        } else {
            false
        }
    }

    fn get_running_task_dwarf(&self) -> Option<Dwarf> {
        self.running_task.as_ref()?.dwarf.clone()
    }
}

pub fn init() -> Result<()> {
    TASK_SCHED.spin_lock().init()
}

pub fn spawn(task: Task) {
    TASK_SCHED.spin_lock().spawn(task)
}

pub fn spawn_user_task(
    elf64: Elf64,
    path: &Path,
    args: &[&str],
    dwarf: Option<Dwarf>,
) -> Result<TaskId> {
    let path_string = path.to_string();
    let all_args: Vec<&str> = [&[path_string.as_str()], args].concat();
    let task = Task::new(
        super::USER_TASK_STACK_SIZE,
        Some(elf64),
        Some(&all_args),
        ContextMode::User,
        dwarf,
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
    }
}

pub fn current() -> Option<&'static Task> {
    unsafe {
        let ptr = TASK_SCHED.spin_lock().current()? as *const Task;
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

pub fn push_layer_id(layer_id: LayerId) -> Result<()> {
    TASK_SCHED.spin_lock().push_layer_id(layer_id)
}

pub fn remove_layer_id(layer_id: LayerId) -> Result<()> {
    TASK_SCHED.spin_lock().remove_layer_id(layer_id)
}

pub fn push_fd_num(fd_num: FileDescriptorNumber) -> Result<()> {
    TASK_SCHED.spin_lock().push_fd_num(fd_num)
}

pub fn remove_fd_num(fd_num: FileDescriptorNumber) -> Result<()> {
    TASK_SCHED.spin_lock().remove_fd_num(fd_num)
}

pub fn push_mem_frame_info(mem_frame_info: MemoryFrameInfo) -> Result<()> {
    TASK_SCHED
        .spin_lock()
        .push_allocated_mem_frame_info(mem_frame_info)
}

pub fn get_mem_frame_size(virt_addr: VirtualAddress) -> Result<Option<usize>> {
    TASK_SCHED
        .spin_lock()
        .get_memory_frame_size_by_virt_addr(virt_addr)
}

pub fn pop_mem_frame_info(virt_addr: VirtualAddress) -> Result<MemoryFrameInfo> {
    TASK_SCHED
        .spin_lock()
        .pop_allocated_memory_by_virt_addr(virt_addr)
}

pub fn show_task_debug() -> bool {
    TASK_SCHED.spin_lock().show_running_task_debug()
}

pub fn get_dwarf() -> Option<Dwarf> {
    TASK_SCHED.spin_lock().get_running_task_dwarf()
}

#[test_case]
fn test_multitask_scheduler_round_robin() {
    let mut sched = TaskScheduler::new();
    sched.init().expect("Failed to init scheduler");

    // Running: KernelTask(TID: 0)
    // ReadyQueue: []

    let t1 = Task::new(0, None, None, ContextMode::Kernel, None).unwrap();
    let t1_id = t1.id;
    sched.spawn(t1);

    let t2 = Task::new(0, None, None, ContextMode::Kernel, None).unwrap();
    let t2_id = t2.id;
    sched.spawn(t2);

    // ReadyQueue: [T1, T2]
    // Running: KernelTask

    let (prev_ptr, next_ptr) = sched.pick_next_task().expect("Sched 1 failed");

    unsafe {
        let prev = &*prev_ptr;
        let next = &*next_ptr;

        assert_ne!(prev.id, next.id);
        assert_eq!(next.id, t1_id);
    }

    // ReadyQueue: [T2, KernelTask]
    // Running: T1

    let (prev_ptr, next_ptr) = sched.pick_next_task().expect("Sched 2 failed");

    unsafe {
        let prev = &*prev_ptr; // T1
        let next = &*next_ptr; // T2

        assert_eq!(prev.id, t1_id);
        assert_eq!(next.id, t2_id);
    }

    // ReadyQueue: [KernelTask, T1]
    // Running: T2

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
    sched.init().unwrap();

    let t1 = Task::new(0, None, None, ContextMode::Kernel, None).unwrap();
    let t1_id = t1.id;
    sched.spawn(t1);

    sched.pick_next_task();

    if let Some(running) = &sched.running_task {
        assert_eq!(running.id, t1_id);
    } else {
        panic!("No running task");
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
