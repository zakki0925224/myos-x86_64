use crate::{
    arch::{self, async_task},
    device,
};
use core::time::Duration;

pub fn sleep(duration: Duration) {
    let global_uptime = device::local_apic_timer::global_uptime();
    let target_time = global_uptime + duration;

    while device::local_apic_timer::global_uptime() < target_time {
        arch::hlt();
    }
}

pub async fn sleep_async(duration: Duration) {
    let global_uptime = device::local_apic_timer::global_uptime();
    let target_time = global_uptime + duration;

    while device::local_apic_timer::global_uptime() < target_time {
        arch::hlt();
        async_task::exec_yield().await;
    }
}
