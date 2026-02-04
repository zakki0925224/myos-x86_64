use crate::{
    arch::{x86_64::context::ContextMode, VirtualAddress},
    debug::dwarf::Dwarf,
    error::Result,
    fs::vfs::FileDescriptorNumber,
    graphics::multi_layer::LayerId,
    kdebug,
    mem::bitmap::MemoryFrameInfo,
    sync::mutex::Mutex,
    task::*,
};
use alloc::{boxed::Box, collections::vec_deque::VecDeque, vec::Vec};

static MULTI_TASK_SCHED: Mutex<MultiTaskScheduler> = Mutex::new(MultiTaskScheduler::new());

struct MultiTaskScheduler {
    ready_queue: VecDeque<Box<Task>>,
    running_task: Option<Box<Task>>,
    exited_tasks: Vec<Box<Task>>,
}

impl MultiTaskScheduler {
    const fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            running_task: None,
            exited_tasks: Vec::new(),
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
        self.ready_queue.push_back(Box::new(task));
    }

    fn pick_next_task(&mut self) -> Option<(*const Task, *const Task)> {
        let prev_task = self.running_task.take()?;

        if let Some(next_task) = self.ready_queue.pop_front() {
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

    fn pick_next_task_on_exit(&mut self, exit_code: i32) -> (*const Task, *const Task) {
        let mut current = self
            .running_task
            .take()
            .expect("task: No running task to exit");

        kdebug!("task: Task exited with code: {}", exit_code);
        current.state = TaskState::Exited(exit_code);
        self.exited_tasks.push(current);

        if let Some(next_task) = self.ready_queue.pop_front() {
            self.running_task = Some(next_task);

            let prev_ptr = &**self.exited_tasks.last().unwrap() as *const Task;
            let next_ptr = &**self.running_task.as_ref().unwrap() as *const Task;

            (prev_ptr, next_ptr)
        } else {
            panic!("task: No tasks available to switch to after exit");
        }
    }

    fn push_layer_id(&mut self, layer_id: LayerId) -> Result<()> {
        let task = self.running_task.as_mut().ok_or("No running task")?;
        task.resource.created_layer_ids.push(layer_id);
        Ok(())
    }

    fn remove_layer_id(&mut self, layer_id: LayerId) -> Result<()> {
        let task = self.running_task.as_mut().ok_or("No running task")?;
        task.resource.created_layer_ids.retain(|id| *id != layer_id);
        Ok(())
    }

    fn push_fd_num(&mut self, fd_num: FileDescriptorNumber) -> Result<()> {
        let task = self.running_task.as_mut().ok_or("No running task")?;
        task.resource.opend_fd_num.push(fd_num);
        Ok(())
    }

    fn remove_fd_num(&mut self, fd_num: FileDescriptorNumber) -> Result<()> {
        let task = self.running_task.as_mut().ok_or("No running task")?;
        task.resource.opend_fd_num.retain(|fd| *fd != fd_num);
        Ok(())
    }

    fn push_allocated_mem_frame_info(&mut self, mem_frame_info: MemoryFrameInfo) -> Result<()> {
        let task = self.running_task.as_mut().ok_or("No running task")?;
        task.resource.allocated_mem_frame_info.push(mem_frame_info);
        Ok(())
    }

    fn get_memory_frame_size_by_virt_addr(
        &mut self,
        virt_addr: VirtualAddress,
    ) -> Result<Option<usize>> {
        let task = self.running_task.as_mut().ok_or("No running task")?;
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
        let task = self.running_task.as_mut().ok_or("No running task")?;
        let allocated_mem_frame_info = &mut task.resource.allocated_mem_frame_info;

        if let Some(index) = allocated_mem_frame_info
            .iter()
            .position(|info| info.frame_start_virt_addr() == Ok(virt_addr))
        {
            return Ok(allocated_mem_frame_info.remove(index));
        }

        Err("Invalid virtual address".into())
    }

    fn debug_running_task(&self) -> bool {
        if let Some(task) = self.running_task.as_ref() {
            super::debug_task(task);
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
    MULTI_TASK_SCHED.spin_lock().init()
}

pub fn spawn(task: Task) {
    MULTI_TASK_SCHED.spin_lock().spawn(task)
}

pub fn sched() {
    let switch_pair = MULTI_TASK_SCHED.spin_lock().pick_next_task();

    if let Some((prev, next)) = switch_pair {
        unsafe { (*prev).switch_to(&*next) };
    }
}

pub fn current() -> Option<&'static Task> {
    unsafe {
        let ptr = MULTI_TASK_SCHED.spin_lock().current()? as *const Task;
        Some(&*ptr)
    }
}

pub fn exit_current(exit_code: i32) -> ! {
    let (prev, next) = MULTI_TASK_SCHED
        .spin_lock()
        .pick_next_task_on_exit(exit_code);

    unsafe {
        (*prev).switch_to(&*next);
    }

    unreachable!();
}

pub fn request(req: TaskRequest) -> Result<TaskResult> {
    let mut sched = MULTI_TASK_SCHED.spin_lock();

    match req {
        TaskRequest::PushLayerId(layer_id) => {
            sched.push_layer_id(layer_id)?;
            Ok(TaskResult::Ok)
        }
        TaskRequest::RemoveLayerId(layer_id) => {
            sched.remove_layer_id(layer_id)?;
            Ok(TaskResult::Ok)
        }
        TaskRequest::PushFileDescriptorNumber(fd_num) => {
            sched.push_fd_num(fd_num)?;
            Ok(TaskResult::Ok)
        }
        TaskRequest::RemoveFileDescriptorNumber(fd_num) => {
            sched.remove_fd_num(fd_num)?;
            Ok(TaskResult::Ok)
        }
        TaskRequest::PushMemory(mem_frame_info) => {
            sched.push_allocated_mem_frame_info(mem_frame_info)?;
            Ok(TaskResult::Ok)
        }
        TaskRequest::GetMemoryFrameSize(virt_addr) => {
            let size = sched.get_memory_frame_size_by_virt_addr(virt_addr)?;
            Ok(TaskResult::MemoryFrameSize(size))
        }
        TaskRequest::PopMemory(virt_addr) => {
            let info = sched.pop_allocated_memory_by_virt_addr(virt_addr)?;
            Ok(TaskResult::PopMemory(info))
        }
        TaskRequest::ExecuteDebugger => {
            let res = sched.debug_running_task();
            Ok(TaskResult::ExecuteDebugger(res))
        }
        TaskRequest::GetDwarf => {
            let dwarf = sched.get_running_task_dwarf();
            Ok(TaskResult::Dwarf(dwarf))
        }
    }
}

#[test_case]
fn test_multitask_scheduler_round_robin() {
    let mut sched = MultiTaskScheduler::new();
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
    let mut sched = MultiTaskScheduler::new();
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

    let (prev_ptr, next_ptr) = sched.pick_next_task_on_exit(123);

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
