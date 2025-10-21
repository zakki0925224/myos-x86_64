use crate::{arch::x86_64, device, task::async_task};
use core::time::Duration;

pub fn global_uptime() -> Duration {
    device::local_apic_timer::global_uptime()
}

pub fn sleep(duration: Duration) {
    let target_time = global_uptime() + duration;

    while global_uptime() < target_time {
        x86_64::stihlt();
    }
}

pub async fn sleep_async(duration: Duration) {
    let target_time = global_uptime() + duration;

    while global_uptime() < target_time {
        x86_64::stihlt();
        async_task::exec_yield().await;
    }
}
