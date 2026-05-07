use crate::{
    arch::x86_64::{self, acpi, cpu},
    error::Result,
    kdebug,
};

fn calc_freq() -> Result<u64> {
    let start = x86_64::rdtsc();
    acpi::pm_timer_wait_ms(1)?;
    let end = x86_64::rdtsc();
    Ok((end - start) * 1000)
}

pub fn init() {
    // check TSC available
    let info = cpu::version_info();
    if !info.feature_tsc {
        panic!("TSC not available");
    }

    let tsc_freq = calc_freq().unwrap();
    kdebug!("tsc: Timer frequency: {}Hz (variant)", tsc_freq);
}

pub fn wait_ms(ms: u64) -> Result<()> {
    let current_tsc_freq = calc_freq()?;
    let start = x86_64::rdtsc();
    let end = start + (current_tsc_freq / 1000) * ms;
    while x86_64::rdtsc() < end {}
    Ok(())
}
