use crate::{
    arch::x86_64::context::ContextMode,
    error::Result,
    kdebug,
    task::{Task, TaskState},
};
use alloc::{collections::vec_deque::VecDeque, vec::Vec};

static mut MULTI_TASK_SCHED: MultiTaskScheduler = MultiTaskScheduler::new();

struct MultiTaskScheduler {
    ready_queue: VecDeque<Task>,
    running_task: Option<Task>,
    exited_tasks: Vec<Task>,
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
        self.running_task = Some(kernel_task);

        Ok(())
    }

    fn current(&self) -> Option<&Task> {
        self.running_task.as_ref()
    }

    fn spawn(&mut self, task: Task) {
        self.ready_queue.push_back(task);
    }

    fn sched(&mut self) {
        if let Some(prev_task) = self.running_task.take() {
            if let Some(next_task) = self.ready_queue.pop_front() {
                self.ready_queue.push_back(prev_task);
                self.running_task = Some(next_task);

                let prev_ctx = &self.ready_queue.back().unwrap().context;
                let next_ctx = &self.running_task.as_ref().unwrap().context;
                prev_ctx.switch_to(next_ctx);
            } else {
                self.running_task = Some(prev_task);
            }
        }
    }

    fn exit_current(&mut self, exit_code: i32) -> ! {
        if let Some(mut current) = self.running_task.take() {
            kdebug!("task: Task exited with code: {}", exit_code);
            current.state = TaskState::Exited(exit_code);
            self.exited_tasks.push(current);
        }

        if let Some(next_task) = self.ready_queue.pop_front() {
            self.running_task = Some(next_task);

            let prev_ctx = self.exited_tasks.last().unwrap();
            let next_ctx = self.running_task.as_ref().unwrap();

            prev_ctx.switch_to(next_ctx);
        } else {
            panic!("task: No tasks available");
        }

        unreachable!();
    }
}

pub fn init() -> Result<()> {
    unsafe { MULTI_TASK_SCHED.init() }
}

pub fn spawn(task: Task) {
    unsafe { MULTI_TASK_SCHED.spawn(task) }
}

pub fn sched() {
    unsafe { MULTI_TASK_SCHED.sched() }
}

pub fn current() -> Option<&'static Task> {
    unsafe { MULTI_TASK_SCHED.current() }
}

pub fn exit_current(exit_code: i32) -> ! {
    unsafe { MULTI_TASK_SCHED.exit_current(exit_code) }
}
