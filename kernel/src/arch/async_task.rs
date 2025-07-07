use super::task::TaskId;
use crate::{debug_, error::Result, sync::mutex::Mutex, util};
use alloc::{boxed::Box, collections::VecDeque};
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

struct AsyncTask {
    id: TaskId,
    future: Pin<Box<dyn Future<Output = ()>>>,
    poll_interval: Option<Duration>,
    last_polled_at: Option<Duration>,
}

impl AsyncTask {
    fn new(future: impl Future<Output = ()> + 'static, poll_interval: Option<Duration>) -> Self {
        Self {
            id: TaskId::new(),
            future: Box::pin(future),
            poll_interval,
            last_polled_at: None,
        }
    }

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        if let Some(interval) = self.poll_interval {
            let global_uptime = util::time::global_uptime();
            if let Some(last_polled) = self.last_polled_at {
                if global_uptime < last_polled + interval {
                    return Poll::Pending;
                }
            }
            self.last_polled_at = Some(global_uptime);
        }

        self.future.as_mut().poll(context)
    }
}

struct Executor {
    task_queue: VecDeque<AsyncTask>,
    is_ready: bool,
}

impl Executor {
    const fn new() -> Self {
        Self {
            task_queue: VecDeque::new(),
            is_ready: false,
        }
    }

    fn poll(&mut self) {
        if !self.is_ready {
            return;
        }

        if let Some(mut task) = self.task_queue.pop_front() {
            let waker = dummy_waker();
            let mut context = Context::from_waker(&waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => debug_!("task: Done (id: {})", task.id.get()),
                Poll::Pending => self.task_queue.push_back(task),
            }
        }
    }

    fn ready(&mut self) {
        self.is_ready = true;
    }

    fn spawn(&mut self, task: AsyncTask) {
        self.task_queue.push_back(task);
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

pub fn spawn(
    future: impl Future<Output = ()> + 'static,
    poll_interval: Option<Duration>,
) -> Result<()> {
    let task = AsyncTask::new(future, poll_interval);
    unsafe { ASYNC_TASK_EXECUTOR.try_lock() }?.spawn(task);
    Ok(())
}
