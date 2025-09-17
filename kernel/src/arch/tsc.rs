use crate::{
    arch::{self, acpi, cpu},
    error::{Error, Result},
    kdebug,
};

fn is_available() -> bool {
    let info = cpu::version_info();
    info.feature_tsc
}

fn calc_freq() -> Result<u64> {
    if !is_available() {
        return Err(Error::Failed("TSC not available"));
    }

    let start = arch::rdtsc();
    acpi::pm_timer_wait_ms(1)?;
    let end = arch::rdtsc();
    Ok((end - start) * 1000)
}

pub fn check_available() {
    let tsc_freq = calc_freq().unwrap();
    kdebug!("tsc: Timer frequency: {}Hz (variant)", tsc_freq);
}

pub fn wait_ms(ms: u64) -> Result<()> {
    let current_tsc_freq = calc_freq()?;
    let start = arch::rdtsc();
    let end = start + (current_tsc_freq / 1000) * ms;
    while arch::rdtsc() < end {}
    Ok(())
}
