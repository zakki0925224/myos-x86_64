use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{
    acpi,
    addr::VirtualAddress,
    arch::{self, mmio::Mmio, volatile::Volatile},
    async_task,
    error::Result,
    idt::{self, GateType, InterruptHandler},
    util::mutex::Mutex,
};
use alloc::vec::Vec;
use core::time::Duration;
use log::{debug, info};

const DIV_VALUE: DivideValue = DivideValue::By1;
const INT_INTERVAL_MS: usize = 10; // must be >= 10ms

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

impl DivideValue {
    fn divisor(&self) -> usize {
        match self {
            Self::By1 => 1,
            // Self::By2 => 2,
            // Self::By4 => 4,
            // Self::By8 => 8,
            // Self::By16 => 16,
            // Self::By32 => 32,
            // Self::By64 => 64,
            // Self::By128 => 128,
        }
    }
}

static mut LOCAL_APIC_TIMER_DRIVER: Mutex<LocalApicTimerDriver> =
    Mutex::new(LocalApicTimerDriver::new());

struct LocalApicTimerDriver {
    device_driver_info: DeviceDriverInfo,
    tick: usize,
    freq: Option<usize>,

    lvt_timer_reg: Option<Mmio<Volatile<u32>>>,
    int_cnt_reg: Option<Mmio<Volatile<u32>>>,
    curr_cnt_reg: Option<Mmio<Volatile<u32>>>,
    div_conf_reg: Option<Mmio<Volatile<u32>>>,
}

impl LocalApicTimerDriver {
    const fn new() -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("local-apic-timer"),
            tick: 0,
            freq: None,

            lvt_timer_reg: None,
            int_cnt_reg: None,
            curr_cnt_reg: None,
            div_conf_reg: None,
        }
    }

    unsafe fn start(&mut self) {
        let init_cnt = if let Some(freq) = self.freq {
            ((freq / 1000 * INT_INTERVAL_MS) / DIV_VALUE.divisor()) as u32
        } else {
            u32::MAX // -1
        };

        self.int_cnt_reg().get_unchecked_mut().write(init_cnt);
    }

    unsafe fn stop(&mut self) {
        self.int_cnt_reg().get_unchecked_mut().write(0);
    }

    unsafe fn tick(&mut self) -> usize {
        if self.freq.is_some() {
            return self.tick;
        }

        let current_cnt = self.curr_cnt_reg().as_ref().read();
        u32::MAX as usize - current_cnt as usize
    }

    fn current_ms(&mut self) -> Result<usize> {
        let _freq = self.freq.ok_or("Frequency not set")?;
        let current_tick = unsafe { self.tick() };
        Ok(current_tick * DIV_VALUE.divisor() * INT_INTERVAL_MS)
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

impl DeviceDriverFunction for LocalApicTimerDriver {
    type AttachInput = ();
    type PollNormalOutput = ();
    type PollInterruptOutput = ();

    fn get_device_driver_info(&self) -> Result<DeviceDriverInfo> {
        Ok(self.device_driver_info.clone())
    }

    fn probe(&mut self) -> Result<()> {
        Ok(())
    }

    fn attach(&mut self, _arg: Self::AttachInput) -> Result<()> {
        let device_name = self.device_driver_info.name;

        // register interrupt handler
        let vec_num = idt::set_handler_dyn_vec(
            InterruptHandler::Normal(poll_int_local_apic_timer),
            GateType::Interrupt,
        )?;
        debug!(
            "{}: Interrupt vector number: 0x{:x}, Interrupt occures every {}ms",
            device_name, vec_num, INT_INTERVAL_MS
        );

        unsafe {
            // calc freq
            self.stop();
            self.div_conf_reg()
                .get_unchecked_mut()
                .write(DIV_VALUE as u32);
            self.lvt_timer_reg()
                .get_unchecked_mut()
                .write((2 << 16) | vec_num as u32); // non masked, periodic
            self.start();
            acpi::pm_timer_wait_ms(1000)?; // wait 1 sec
            let tick = self.tick() * DIV_VALUE.divisor();
            self.stop();

            assert!(tick > 0);
            debug!(
                "{}: Timer frequency was detected: {}Hz ({:?})",
                device_name, tick, DIV_VALUE
            );

            self.freq = Some(tick);

            // start timer
            self.start();
        }

        self.device_driver_info.attached = true;
        Ok(())
    }

    fn poll_normal(&mut self) -> Result<Self::PollNormalOutput> {
        unimplemented!()
    }

    fn poll_int(&mut self) -> Result<Self::PollInterruptOutput> {
        if !self.device_driver_info.attached {
            return Ok(());
        }

        if self.tick == usize::MAX {
            self.tick = 0;
        } else {
            self.tick += 1;
        }

        // poll async tasks
        let _ = async_task::poll();

        Ok(())
    }

    fn open(&mut self) -> Result<()> {
        unimplemented!()
    }

    fn close(&mut self) -> Result<()> {
        unimplemented!()
    }

    fn read(&mut self) -> Result<Vec<u8>> {
        unimplemented!()
    }

    fn write(&mut self, _data: &[u8]) -> Result<()> {
        unimplemented!()
    }
}

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    let driver = unsafe { LOCAL_APIC_TIMER_DRIVER.try_lock() }?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = unsafe { LOCAL_APIC_TIMER_DRIVER.try_lock() }?;
    driver.probe()?;
    driver.attach(())?;
    info!("{}: Attached!", driver.device_driver_info.name);

    Ok(())
}

pub fn open() -> Result<()> {
    let mut driver = unsafe { LOCAL_APIC_TIMER_DRIVER.try_lock() }?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = unsafe { LOCAL_APIC_TIMER_DRIVER.try_lock() }?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = unsafe { LOCAL_APIC_TIMER_DRIVER.try_lock() }?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = unsafe { LOCAL_APIC_TIMER_DRIVER.try_lock() }?;
    driver.write(data)
}

pub fn global_uptime() -> Duration {
    let driver = unsafe { LOCAL_APIC_TIMER_DRIVER.get_force_mut() };
    let ms = driver.current_ms().unwrap_or(0);
    Duration::from_millis(ms as u64)
}

extern "x86-interrupt" fn poll_int_local_apic_timer() {
    unsafe {
        let driver = LOCAL_APIC_TIMER_DRIVER.get_force_mut();
        let _ = driver.poll_int();

        arch::apic::notify_end_of_int();
    }
}
