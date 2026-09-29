use crate::{
    arch::x86_64,
    device::{DeviceInfo, Driver},
    error::{Error, Result},
    fs::vfs,
    kinfo,
    sync::mutex::Mutex,
    util,
};
use core::time::Duration;

pub const NAME: &str = "speaker";

static SPEAKER_DRIVER: Mutex<SpeakerDriver> = Mutex::new(SpeakerDriver::new());

struct SpeakerDriver {
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
        Self { current_freq: 0 }
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

impl Driver for SpeakerDriver {
    fn info(&self) -> DeviceInfo {
        DeviceInfo::new(NAME)
    }

    fn attach(&mut self) -> Result<()> {
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        let s = str::from_utf8(data).map_err(|_| Error::InvalidData.with_context("data"))?;
        let freq: u32 = s
            .trim()
            .parse()
            .map_err(|_| Error::InvalidData.with_context("frequency"))?;
        self.play(freq);

        Ok(())
    }
}

pub fn probe_and_attach() -> Result<()> {
    SPEAKER_DRIVER.try_lock()?.attach()?;
    vfs::add_dev(&SPEAKER_DRIVER)?;
    kinfo!("{}: Attached!", NAME);

    Ok(())
}

pub fn play(freq: u32, duration: Duration) -> Result<()> {
    let mut driver = SPEAKER_DRIVER.try_lock()?;
    driver.play(freq);
    util::time::sleep(duration);
    driver.stop();

    Ok(())
}
