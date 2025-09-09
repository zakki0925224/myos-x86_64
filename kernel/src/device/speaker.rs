use super::{DeviceDriverFunction, DeviceDriverInfo};
use crate::{arch, error::Result, fs::vfs, kinfo, util};
use alloc::vec::Vec;
use core::time::Duration;

static mut SPEAKER_DRIVER: SpeakerDriver = SpeakerDriver::new();

struct SpeakerDriver {
    device_driver_info: DeviceDriverInfo,
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
        }
    }

    fn play(&self, freq: u32) {
        let div = (Self::PIT_BASE_FREQ / freq) as u16;

        arch::out8(
            Self::PORT_PIT_CTRL,
            Self::TIMER2_SELECT | Self::WRITE_WORD | Self::MODE_SQUARE_WAVE,
        );
        arch::out8(Self::PORT_TIMER2_CTRL, div as u8);

        arch::out8(Self::PORT_TIMER2_CTRL, (div >> 8) as u8);
        arch::out8(Self::PORT_TIMER2_CTRL, div as u8);

        arch::out8(0x61, arch::in8(0x61) | 3);
    }

    fn stop(&self) {
        arch::out8(0x61, arch::in8(0x61) & !3);
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
    unsafe { SPEAKER_DRIVER.get_device_driver_info() }
}

pub fn probe_and_attach() -> Result<()> {
    unsafe {
        SPEAKER_DRIVER.probe()?;
        SPEAKER_DRIVER.attach(())?;
        kinfo!("{}: Attached!", get_device_driver_info()?.name);
    }

    Ok(())
}

pub fn open() -> Result<()> {
    unsafe { SPEAKER_DRIVER.open() }
}

pub fn close() -> Result<()> {
    unsafe { SPEAKER_DRIVER.close() }
}

pub fn read() -> Result<Vec<u8>> {
    unsafe { SPEAKER_DRIVER.read() }
}

pub fn write(data: &[u8]) -> Result<()> {
    unsafe { SPEAKER_DRIVER.write(data) }
}

pub fn play(freq: u32, duration: Duration) {
    unsafe {
        SPEAKER_DRIVER.play(freq);
        util::time::sleep(duration);
        SPEAKER_DRIVER.stop();
    }
}

pub async fn play_async(freq: u32, duration: Duration) {
    unsafe {
        SPEAKER_DRIVER.play(freq);
        util::time::sleep_async(duration).await;
        SPEAKER_DRIVER.stop();
    }
}
