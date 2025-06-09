use crate::{
    arch::{self, async_task},
    device,
};
use core::time::Duration;

pub fn global_uptime() -> Duration {
    device::local_apic_timer::global_uptime()
}

pub fn sleep(duration: Duration) {
    let target_time = global_uptime() + duration;

    while global_uptime() < target_time {
        arch::hlt();
    }
}

pub async fn sleep_async(duration: Duration) {
    let target_time = global_uptime() + duration;

    while global_uptime() < target_time {
        arch::hlt();
        async_task::exec_yield().await;
    }
}
