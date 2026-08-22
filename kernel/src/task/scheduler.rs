use crate::{
    arch::x86_64::{
        context::{Context, ContextMode, InterruptedContext},
        registers::Rflags,
    },
    debug::dwarf::Dwarf,
    error::{Error, Result},
    fs::{path::Path, vfs::FileDescriptorNumber},
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

    fn find_task(&self, id: TaskId) -> Option<&Task> {
        if let Some(task) = self.current_task.as_deref() {
            if task.id == id {
                return Some(task);
            }
        }

        if let Some(task) = self.ready_queue.iter().find(|t| t.id == id) {
            return Some(task.as_ref());
        }

        if let Some(task) = self.sleeping_tasks.iter().find(|t| t.id == id) {
            return Some(task.as_ref());
        }

        None
    }

    fn find_task_mut(&mut self, id: TaskId) -> Option<&mut Task> {
        if self.current_task.as_deref().map_or(false, |t| t.id == id) {
            return self.current_task.as_deref_mut();
        }

        if let Some(task) = self.ready_queue.iter_mut().find(|t| t.id == id) {
            return Some(task.as_mut());
        }

        if let Some(task) = self.sleeping_tasks.iter_mut().find(|t| t.id == id) {
            return Some(task.as_mut());
        }

        None
    }

    fn spawn(&mut self, task: Task) {
        self.ready_queue.push_back(Box::new(task));
    }

    fn pick_next_task(&mut self) -> Option<(*const Task, *const Task)> {
        let mut prev_task = self.current_task.take()?;

        if let Some(mut next_task) = self.ready_queue.pop_front() {
            prev_task.state = TaskState::Ready;
            next_task.state = TaskState::Running;

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

        if let Some(parent_id) = current.parent {
            if let Some(parent_task) = self.find_task_mut(parent_id) {
                parent_task.children.retain(|id| *id != exiting_id);
            }
        }

        let new_parent_id = current.parent.unwrap_or(TaskId::KERNEL);
        for child_id in current.children.drain(..) {
            if let Some(child_task) = self.find_task_mut(child_id) {
                child_task.parent = Some(new_parent_id);
            }
            if let Some(new_parent_task) = self.find_task_mut(new_parent_id) {
                new_parent_task.children.push(child_id);
            }
        }

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

        let mut next_task = self
            .ready_queue
            .pop_front()
            .expect("No task to run after exit");
        next_task.state = TaskState::Running;
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

        let mut next_task = self
            .ready_queue
            .pop_front()
            .expect("No task to run after sleep");
        next_task.state = TaskState::Running;
        self.current_task = Some(next_task);

        let prev_ptr = &**self.sleeping_tasks.last().unwrap() as *const Task;
        let next_ptr = &**self.current_task.as_ref().unwrap() as *const Task;

        (prev_ptr, next_ptr)
    }

    fn try_sleep_current_waiting_for(
        &mut self,
        child_id: TaskId,
    ) -> Option<(*const Task, *const Task)> {
        if self.exit_codes.contains_key(&child_id) {
            return None;
        }
        Some(self.sleep_current_waiting_for(child_id))
    }
}

pub fn init() -> Result<()> {
    let mut kernel_task = Task::new(
        None,
        0,
        None,
        None,
        ContextMode::Kernel,
        None,
        [None, None, None],
    )?;
    assert!(kernel_task.id == TaskId::KERNEL);
    kernel_task.state = TaskState::Running;
    TASK_SCHED.spin_lock().current_task = Some(Box::new(kernel_task));
    Ok(())
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
    let parent_id = current_task_id().ok_or(Error::NotFound.with_context("current task"))?;
    let task = Task::new(
        Some(parent_id),
        super::USER_TASK_STACK_SIZE,
        Some(elf64),
        Some(&all_args),
        ContextMode::User,
        dwarf,
        pipe_fd,
    )?;

    let id = task.id;
    let mut s = TASK_SCHED.spin_lock();
    s.spawn(task);
    s.current_task_mut()?.children.push(id);

    Ok(id)
}

pub fn sleep_waiting_for(child_id: TaskId) {
    let saved = Rflags::read_with_cli();
    let pair = TASK_SCHED
        .spin_lock()
        .try_sleep_current_waiting_for(child_id);
    if let Some((prev, next)) = pair {
        unsafe {
            (*prev).switch_to(&*next);
        }
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

pub fn current_task_id() -> Option<TaskId> {
    let s = TASK_SCHED.spin_lock();
    Some(s.current_task.as_deref()?.id)
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

pub fn current_debug_print() -> bool {
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

pub fn with_current_resource<R>(f: impl FnOnce(&mut TaskResource) -> R) -> Result<R> {
    let mut s = TASK_SCHED.spin_lock();
    Ok(f(&mut s.current_task_mut()?.resource))
}

pub fn capture_current_syscall_frame(interrupted: &InterruptedContext) {
    let mut s = TASK_SCHED.spin_lock();
    if let Some(current) = s.current_task.as_mut() {
        current.context.capture_from_interrupted(interrupted);
    }
}

pub fn preempt_sched(interrupted: &InterruptedContext) -> *const Context {
    let (pair, stale) = {
        let mut s = TASK_SCHED.spin_lock();

        if let Some(current) = s.current_task.as_mut() {
            current.context.capture_from_interrupted(interrupted);
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

pub fn task_ids() -> Vec<TaskId> {
    let mut ids = Vec::new();
    let s = TASK_SCHED.spin_lock();
    s.ready_queue.iter().for_each(|t| ids.push(t.id));

    if let Some(t) = &s.current_task {
        ids.push(t.id);
    }

    s.sleeping_tasks.iter().for_each(|t| ids.push(t.id));

    ids
}

pub fn task_snapshot(id: TaskId) -> Option<TaskSnapshot> {
    let s = TASK_SCHED.spin_lock();
    s.find_task(id).map(|t| TaskSnapshot {
        id: t.id,
        name: t.name.clone(),
        state: t.state,
        parent: t.parent,
    })
}

pub fn fork_current() -> Result<TaskId> {
    Rflags::read_with_cli();
    fork_current_locked()
}

fn fork_current_locked() -> Result<TaskId> {
    let mut s = TASK_SCHED.spin_lock();
    let parent = s.current_task_mut()?;
    let child = Task::fork_from(parent, parent.context)?;
    let child_id = child.id;
    s.spawn(child);
    s.current_task_mut()?.children.push(child_id);
    Ok(child_id)
}

#[test_case]
fn test_multitask_scheduler_round_robin() {
    let mut sched = TaskScheduler::new();
    let kernel_task = Task::new(
        None,
        0,
        None,
        None,
        ContextMode::Kernel,
        None,
        [None, None, None],
    )
    .unwrap();
    sched.current_task = Some(Box::new(kernel_task));

    // current: KernelTask(TID: 0)
    // ReadyQueue: []

    let t1 = Task::new(
        None,
        0,
        None,
        None,
        ContextMode::Kernel,
        None,
        [None, None, None],
    )
    .unwrap();
    let t1_id = t1.id;
    sched.spawn(t1);

    let t2 = Task::new(
        None,
        0,
        None,
        None,
        ContextMode::Kernel,
        None,
        [None, None, None],
    )
    .unwrap();
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
    let kernel_task = Task::new(
        None,
        0,
        None,
        None,
        ContextMode::Kernel,
        None,
        [None, None, None],
    )
    .unwrap();
    sched.current_task = Some(Box::new(kernel_task));

    let t1 = Task::new(
        None,
        0,
        None,
        None,
        ContextMode::Kernel,
        None,
        [None, None, None],
    )
    .unwrap();
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
