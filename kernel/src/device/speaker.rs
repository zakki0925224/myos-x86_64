use crate::{
    arch::x86_64,
    device::{DeviceDriverFunction, DeviceDriverInfo},
    error::Result,
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
    util,
};
use alloc::vec::Vec;
use core::time::Duration;

static SPEAKER_DRIVER: Mutex<SpeakerDriver> = Mutex::new(SpeakerDriver::new());

struct SpeakerDriver {
    device_driver_info: DeviceDriverInfo,
    current_freq: u32,
}

// https://wiki.osdev.org/PC_Speaker
impl SpeakerDriver {
    const PIT_BASE_FREQ: u32 = 1193182;
    const PORT_PIT_CTRL: u16 = 0x43;
    const PORT_TIMER2_CTRL: u16 = 0x42;
    const TIMER2_SELECT: u8 = 0x80;
    const WRITE_WORD: u8 = 0x30;
    const MODE_SQUARE_WAVE: u8 = 0x06;

    const fn new() -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("speaker"),
            current_freq: 0,
        }
    }

    fn play(&mut self, freq: u32) {
        if freq == 0 {
            self.stop();
            return;
        }

        if self.current_freq == freq {
            return;
        }

        let div = (Self::PIT_BASE_FREQ / freq) as u16;

        x86_64::out8(
            Self::PORT_PIT_CTRL,
            Self::TIMER2_SELECT | Self::WRITE_WORD | Self::MODE_SQUARE_WAVE,
        );
        x86_64::out8(Self::PORT_TIMER2_CTRL, (div & 0xFF) as u8);
        x86_64::out8(Self::PORT_TIMER2_CTRL, (div >> 8) as u8);

        let status = x86_64::in8(0x61);
        if status & 3 != 3 {
            x86_64::out8(0x61, status | 3);
        }

        self.current_freq = freq;
    }

    fn stop(&mut self) {
        if self.current_freq == 0 {
            return;
        }
        x86_64::out8(0x61, x86_64::in8(0x61) & !3);
        self.current_freq = 0;
    }
}

impl DeviceDriverFunction for SpeakerDriver {
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
        let dev_desc = vfs::DeviceFileDescriptor {
            get_device_driver_info,
            open,
            close,
            read,
            write,
        };
        vfs::add_dev_file(dev_desc, self.device_driver_info.name)?;
        self.device_driver_info.attached = true;
        Ok(())
    }

    fn poll_normal(&mut self) -> Result<Self::PollNormalOutput> {
        unimplemented!()
    }

    fn poll_int(&mut self) -> Result<Self::PollInterruptOutput> {
        unimplemented!()
    }

    fn open(&mut self) -> Result<()> {
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        Ok(())
    }

    fn read(&mut self) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        let s = str::from_utf8(data).map_err(|_| "Failed to parse string")?;
        let freq: u32 = s.trim().parse().map_err(|_| "Failed to parse u32 number")?;
        self.play(freq);

        Ok(())
    }
}

pub fn get_device_driver_info() -> Result<DeviceDriverInfo> {
    SPEAKER_DRIVER.try_lock()?.get_device_driver_info()
}

pub fn probe_and_attach() -> Result<()> {
    let mut driver = SPEAKER_DRIVER.try_lock()?;
    driver.probe()?;
    driver.attach(())?;
    kinfo!("{}: Attached!", driver.get_device_driver_info()?.name);

    Ok(())
}

pub fn open() -> Result<()> {
    SPEAKER_DRIVER.try_lock()?.open()
}

pub fn close() -> Result<()> {
    SPEAKER_DRIVER.try_lock()?.close()
}

pub fn read() -> Result<Vec<u8>> {
    SPEAKER_DRIVER.try_lock()?.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    SPEAKER_DRIVER.try_lock()?.write(data)
}

pub fn play(freq: u32, duration: Duration) -> Result<()> {
    let mut driver = SPEAKER_DRIVER.try_lock()?;
    driver.play(freq);
    util::time::sleep(duration);
    driver.stop();

    Ok(())
}
