use crate::{
    arch::{
        x86_64::{
            context::{Context, InterruptedContext},
            *,
        },
        VirtualAddress,
    },
    device::*,
    error::{Error, Result},
    kdebug, kinfo, kwarn,
    sync::{mutex::Mutex, volatile::Volatile},
    task::{self, async_task},
    util::mmio::Mmio,
};
use core::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

static ATTACHED: AtomicBool = AtomicBool::new(false);

const DIV_VALUE: DivideValue = DivideValue::By1;
pub const INT_INTERVAL_MS: usize = 10;

const WINDOW_PM_TICKS: u32 = acpi::PM_TIMER_FREQ / 10; // 100ms
const MAX_CALIBRATION_ATTEMPTS: usize = 10;
const PM_SAMPLE_SKEW_THRESHOLD: u32 = 100; // ~28us
const SELF_CHECK_TOLERANCE_PERMILLE: u64 = 10; // 1%

#[allow(dead_code)]
#[derive(Debug)]
#[repr(u8)]
enum DivideValue {
    By1 = 0b1011,
    // By2 = 0b0000,
    // By4 = 0b0001,
    // By8 = 0b0010,
    // By16 = 0b0011,
    // By32 = 0b1000,
    // By64 = 0b1001,
    // By128 = 0b1010,
}

const NAME: &str = "local-apic-timer";

static LOCAL_APIC_TIMER_DRIVER: Mutex<LocalApicTimerDriver> =
    Mutex::new(LocalApicTimerDriver::new());

struct LocalApicTimerDriver {
    tick: usize,
    freq: Option<usize>,

    lvt_timer_reg: Option<Mmio<Volatile<u32>>>,
    int_cnt_reg: Option<Mmio<Volatile<u32>>>,
    curr_cnt_reg: Option<Mmio<Volatile<u32>>>,
    div_conf_reg: Option<Mmio<Volatile<u32>>>,

    pm_timer: Option<acpi::PmTimer>,
    pm_last: u32,
    pm_accum_ticks: u64,
}

impl LocalApicTimerDriver {
    fn poll_int(&mut self) -> Result<()> {
        if !ATTACHED.load(Ordering::Acquire) {
            return Ok(());
        }

        if self.tick == usize::MAX {
            self.tick = 0;
        } else {
            self.tick += 1;
        }

        if let Some(pm_timer) = self.pm_timer.as_ref() {
            let now = pm_timer.read();
            self.pm_accum_ticks += pm_timer.elapsed(self.pm_last, now) as u64;
            self.pm_last = now;
        }

        let _ = async_task::poll();

        Ok(())
    }

    const fn new() -> Self {
        Self {
            tick: 0,
            freq: None,

            lvt_timer_reg: None,
            int_cnt_reg: None,
            curr_cnt_reg: None,
            div_conf_reg: None,

            pm_timer: None,
            pm_last: 0,
            pm_accum_ticks: 0,
        }
    }

    unsafe fn stop(&mut self) {
        self.int_cnt_reg().get_unchecked_mut().write(0);
    }

    unsafe fn sample_pm_apic(&mut self, pm: &acpi::PmTimer) -> (u32, u32) {
        let mut fallback: Option<(u32, u32, u32)> = None;

        for _ in 0..MAX_CALIBRATION_ATTEMPTS {
            let pm_a = pm.read();
            let apic = self.curr_cnt_reg().as_ref().read();
            let pm_b = pm.read();
            let skew = pm.elapsed(pm_a, pm_b);

            if skew <= PM_SAMPLE_SKEW_THRESHOLD {
                return (pm_a, apic);
            }

            let replace = match fallback {
                Some((_, _, best_skew)) => skew < best_skew,
                None => true,
            };
            if replace {
                fallback = Some((pm_a, apic, skew));
            }
        }

        let (pm_a, apic, skew) = fallback.unwrap();
        kwarn!(
            "{}: PM timer sample skew exceeded threshold after {} attempts ({} > {})",
            NAME,
            MAX_CALIBRATION_ATTEMPTS,
            skew,
            PM_SAMPLE_SKEW_THRESHOLD
        );
        (pm_a, apic)
    }

    unsafe fn tick(&mut self) -> usize {
        if self.freq.is_some() {
            return self.tick;
        }

        let current_cnt = self.curr_cnt_reg().as_ref().read();
        u32::MAX as usize - current_cnt as usize
    }

    fn current_ms(&mut self) -> Result<usize> {
        let _freq = self
            .freq
            .ok_or(Error::NotInitialized.with_context("frequency"))?;
        Ok(self.tick * INT_INTERVAL_MS)
    }

    fn lvt_timer_reg(&mut self) -> &mut Mmio<Volatile<u32>> {
        if self.lvt_timer_reg.is_none() {
            let reg = unsafe { Mmio::from_raw(VirtualAddress::new(0xfee00320).as_ptr_mut()) };
            self.lvt_timer_reg = Some(reg);
        }

        self.lvt_timer_reg.as_mut().unwrap()
    }

    fn int_cnt_reg(&mut self) -> &mut Mmio<Volatile<u32>> {
        if self.int_cnt_reg.is_none() {
            let reg = unsafe { Mmio::from_raw(VirtualAddress::new(0xfee00380).as_ptr_mut()) };
            self.int_cnt_reg = Some(reg);
        }

        self.int_cnt_reg.as_mut().unwrap()
    }

    fn curr_cnt_reg(&mut self) -> &mut Mmio<Volatile<u32>> {
        if self.curr_cnt_reg.is_none() {
            let reg = unsafe { Mmio::from_raw(VirtualAddress::new(0xfee00390).as_ptr_mut()) };
            self.curr_cnt_reg = Some(reg);
        }

        self.curr_cnt_reg.as_mut().unwrap()
    }

    fn div_conf_reg(&mut self) -> &mut Mmio<Volatile<u32>> {
        if self.div_conf_reg.is_none() {
            let reg = unsafe { Mmio::from_raw(VirtualAddress::new(0xfee003e0).as_ptr_mut()) };
            self.div_conf_reg = Some(reg);
        }

        self.div_conf_reg.as_mut().unwrap()
    }
}

impl Driver for LocalApicTimerDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        let vec_num = idt::set_handler_dyn_vec(
            idt::InterruptHandler::Naked(preempt_timer_isr),
            idt::GateType::Interrupt,
        )?;
        kdebug!(
            "{}: Interrupt vector number: {:#x}, Interrupt occures every {}ms",
            NAME,
            vec_num,
            INT_INTERVAL_MS
        );

        let pm = acpi::pm_timer()?;

        unsafe {
            self.stop();
            self.div_conf_reg()
                .get_unchecked_mut()
                .write(DIV_VALUE as u32);
            self.lvt_timer_reg().get_unchecked_mut().write(1 << 16); // masked
            self.int_cnt_reg().get_unchecked_mut().write(u32::MAX);

            let tsc0 = rdtsc();
            let (pm0, apic0) = self.sample_pm_apic(&pm);

            while pm.elapsed(pm0, pm.read()) < WINDOW_PM_TICKS {}

            let (pm1, apic1) = self.sample_pm_apic(&pm);
            let tsc1 = rdtsc();

            let apic_ticks = apic0 as u64 - apic1 as u64;
            let pm_ticks = pm.elapsed(pm0, pm1) as u64;
            assert!(apic_ticks > 0 && pm_ticks > 0);

            let freq_hz = apic_ticks * acpi::PM_TIMER_FREQ as u64 / pm_ticks;
            let num = apic_ticks * acpi::PM_TIMER_FREQ as u64 * INT_INTERVAL_MS as u64;
            let den = pm_ticks * 1000;
            let init_cnt = (num + den / 2) / den;
            assert!(init_cnt <= u32::MAX as u64);

            let tsc_freq = (tsc1 - tsc0) * acpi::PM_TIMER_FREQ as u64 / pm_ticks;
            kdebug!("tsc: Timer frequency: {}Hz", tsc_freq);
            kdebug!(
                "{}: Timer frequency: {}Hz, init_cnt: {} ({:?})",
                NAME,
                freq_hz,
                init_cnt,
                DIV_VALUE
            );

            crystal_clock_cross_check(freq_hz);

            self.freq = Some(freq_hz as usize);

            self.stop();
            self.lvt_timer_reg()
                .get_unchecked_mut()
                .write((2 << 16) | vec_num as u32); // periodic, unmasked
            self.int_cnt_reg()
                .get_unchecked_mut()
                .write(init_cnt as u32);
            self.tick = 0;
            self.pm_last = pm.read();
            self.pm_accum_ticks = 0;
            self.pm_timer = Some(pm);
        }

        ATTACHED.store(true, Ordering::Release);
        Ok(())
    }
}

pub fn probe_and_attach() -> Result<()> {
    LOCAL_APIC_TIMER_DRIVER.try_lock()?.attach()?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}

pub fn global_uptime() -> Duration {
    let driver = unsafe { LOCAL_APIC_TIMER_DRIVER.get_force_mut() };
    let ms = driver.current_ms().unwrap_or(0);
    Duration::from_millis(ms as u64)
}

fn crystal_clock_cross_check(measured_freq_hz: u64) {
    if cpu::version_info().feature_hypervisor {
        return;
    }

    let (max_leaf, _, _, _) = cpu::cpuid(0);
    if max_leaf < 0x15 {
        return;
    }

    let (_, _, crystal_hz, _) = cpu::cpuid(0x15);
    if crystal_hz == 0 {
        return;
    }

    kdebug!(
        "{}: CPUID.15H core crystal clock: {}Hz (measured: {}Hz)",
        NAME,
        crystal_hz,
        measured_freq_hz
    );
}

pub fn self_check() -> Result<()> {
    let driver = LOCAL_APIC_TIMER_DRIVER.try_lock()?;

    if driver.pm_timer.is_none() {
        return Err(Error::NotInitialized.with_context("pm_timer"));
    }
    let pm_ticks = driver.pm_accum_ticks;
    let apic_ms = driver.tick as u64 * INT_INTERVAL_MS as u64;
    drop(driver);

    let pm_ms = pm_ticks * 1000 / acpi::PM_TIMER_FREQ as u64;
    if pm_ms == 0 {
        return Ok(());
    }

    let ratio_permille = (apic_ms * 1000 + pm_ms / 2) / pm_ms;
    let deviation = ratio_permille.abs_diff(1000);
    let whole = ratio_permille / 1000;
    let frac = ratio_permille % 1000;

    if deviation <= SELF_CHECK_TOLERANCE_PERMILLE {
        kdebug!(
            "{}: self-check: apic={}ms pm={}ms ratio={}.{:03}",
            NAME,
            apic_ms,
            pm_ms,
            whole,
            frac
        );
    } else {
        kwarn!(
            "{}: self-check: apic={}ms pm={}ms ratio={}.{:03}",
            NAME,
            apic_ms,
            pm_ms,
            whole,
            frac
        );
    }

    Ok(())
}

#[unsafe(naked)]
unsafe extern "C" fn preempt_timer_isr() {
    core::arch::naked_asm!(
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rbp",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov rdi, rsp",
        "mov rbp, rsp",
        "and rsp, -16",
        "call timer_preempt_handler",
        "mov rsp, rbp",
        "test rax, rax",
        "jz 2f",
        "mov rdi, rax",
        "jmp restore_context_and_iret",
        "2:",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rbp",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "pop rax",
        "iretq",
    );
}

#[no_mangle]
unsafe extern "sysv64" fn timer_preempt_handler(
    interrupted: *const InterruptedContext,
) -> *const Context {
    let driver = LOCAL_APIC_TIMER_DRIVER.get_force_mut();

    if !ATTACHED.load(Ordering::Acquire) {
        apic::notify_eoi();
        return core::ptr::null();
    }

    let _ = driver.poll_int();
    apic::notify_eoi();

    task::scheduler::preempt_sched(&*interrupted)
}
