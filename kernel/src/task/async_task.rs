use crate::{error::Result, kdebug, sync::mutex::Mutex, task::TaskId, util};
use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, VecDeque},
};
use core::{
    future::Future,
    pin::Pin,
    ptr::null,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    time::Duration,
};

static mut ASYNC_TASK_EXECUTOR: Mutex<Executor> = Mutex::new(Executor::new());

#[derive(Default)]
struct Yield {
    polled: AtomicBool,
}

impl Future for Yield {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        if self.polled.fetch_or(true, Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

pub struct TimeoutFuture {
    timeout: Duration,
}

impl TimeoutFuture {
    pub fn new(durtion: Duration) -> Self {
        let global_uptime = util::time::global_uptime();
        Self {
            timeout: global_uptime + durtion,
        }
    }
}

impl Future for TimeoutFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        let global_uptime = util::time::global_uptime();

        if self.timeout <= global_uptime {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    High,
    Normal,
    Low,
}

struct AsyncTask {
    id: TaskId,
    future: Pin<Box<dyn Future<Output = ()>>>,
    priority: Priority,
}

impl AsyncTask {
    fn new(future: impl Future<Output = ()> + 'static, priority: Priority) -> Self {
        Self {
            id: TaskId::new(),
            future: Box::pin(future),
            priority,
        }
    }

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}

struct Executor {
    task_queues: BTreeMap<Priority, VecDeque<AsyncTask>>,
    is_ready: bool,
    poll_count: usize,
}

impl Executor {
    const fn new() -> Self {
        Self {
            task_queues: BTreeMap::new(),
            is_ready: false,
            poll_count: 0,
        }
    }

    fn poll(&mut self) {
        if !self.is_ready {
            return;
        }

        self.poll_count = self.poll_count.wrapping_add(1);

        for &p in &[Priority::High, Priority::Normal, Priority::Low] {
            let do_skip = match p {
                Priority::High => false,
                Priority::Normal => self.poll_count % 2 != 0,
                Priority::Low => self.poll_count % 4 != 0,
            };
            if do_skip {
                continue;
            }

            if let Some(queue) = self.task_queues.get_mut(&p) {
                if let Some(mut task) = queue.pop_front() {
                    let waker = dummy_waker();
                    let mut context = Context::from_waker(&waker);
                    match task.poll(&mut context) {
                        Poll::Ready(()) => {
                            kdebug!("task: Done (id: {})", task.id.get());
                        }
                        Poll::Pending => {
                            queue.push_back(task);
                        }
                    }
                }
            }
        }
    }

    fn ready(&mut self) {
        self.is_ready = true;
        self.poll_count = 0;
    }

    fn spawn(&mut self, task: AsyncTask) {
        let priority = task.priority;
        self.task_queues
            .entry(priority)
            .or_insert_with(VecDeque::new)
            .push_back(task);
    }
}

fn dummy_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker()
    }
    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(null(), vtable)
}

fn dummy_waker() -> Waker {
    unsafe { Waker::from_raw(dummy_raw_waker()) }
}

pub async fn exec_yield() {
    Yield::default().await
}

pub fn poll() -> Result<()> {
    unsafe { ASYNC_TASK_EXECUTOR.try_lock() }?.poll();
    Ok(())
}

pub fn ready() -> Result<()> {
    unsafe { ASYNC_TASK_EXECUTOR.try_lock() }?.ready();
    Ok(())
}

pub fn spawn(future: impl Future<Output = ()> + 'static) -> Result<()> {
    let task = AsyncTask::new(future, Priority::Normal);
    unsafe { ASYNC_TASK_EXECUTOR.try_lock() }?.spawn(task);
    Ok(())
}

pub fn spawn_with_priority(
    future: impl Future<Output = ()> + 'static,
    priority: Priority,
) -> Result<()> {
    let task = AsyncTask::new(future, priority);
    unsafe { ASYNC_TASK_EXECUTOR.try_lock() }?.spawn(task);
    Ok(())
}
