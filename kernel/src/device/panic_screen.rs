use crate::{
    arch::VirtualAddress,
    device::{DeviceDriverFunction, DeviceDriverInfo},
    error::Result,
    graphics::{color::ColorCode, font::FONT},
    kinfo,
    sync::mutex::Mutex,
};
use alloc::{fmt, vec::Vec};
use common::graphic_info::{GraphicInfo, PixelFormat};
use core::fmt::Write;

const BACK_COLOR: ColorCode = ColorCode::BLACK;
const FORE_COLOR: ColorCode = ColorCode::RED;

static mut PANIC_SCREEN_DRIVER: Mutex<PanicScreenDriver> = Mutex::new(PanicScreenDriver::new());

struct PanicScreenDriver {
    device_driver_info: DeviceDriverInfo,
    cursor_x: Option<usize>,
    cursor_y: Option<usize>,
    res_x: Option<usize>,
    res_y: Option<usize>,
    pixel_format: Option<PixelFormat>,
    frame_buf_virt_addr: Option<VirtualAddress>,
}

impl PanicScreenDriver {
    const fn new() -> Self {
        Self {
            device_driver_info: DeviceDriverInfo::new("panic-screen"),
            cursor_x: None,
            cursor_y: None,
            res_x: None,
            res_y: None,
            pixel_format: None,
            frame_buf_virt_addr: None,
        }
    }

    fn char_max_xy_len(&self) -> (usize, usize) {
        let (font_width, font_height) = FONT.get_wh();

        (
            self.res_x.unwrap_or(0) / font_width - 1,
            self.res_y.unwrap_or(0) / font_height - 1,
        )
    }

    fn inc_cursor(&mut self) {
        let mut cursor_x = self.cursor_x.unwrap_or(0) + 1;
        let mut cursor_y = self.cursor_y.unwrap_or(0);
        let (char_max_x_len, char_max_y_len) = self.char_max_xy_len();

        if cursor_x > char_max_x_len {
            cursor_x = 0;
            cursor_y += 1;
        }

        if cursor_y > char_max_y_len {
            cursor_x = 0;
            cursor_y = 0;
        }

        self.cursor_x = Some(cursor_x);
        self.cursor_y = Some(cursor_y);
    }

    fn write_pixel(&mut self, x: usize, y: usize, color_code: ColorCode) {
        let res_x = self.res_x.unwrap_or(0);
        let res_y = self.res_y.unwrap_or(0);
        let offset = (res_x * y + x) * 4;
        let pixel_format = match self.pixel_format {
            Some(format) => format,
            None => return,
        };
        let frame_buf_virt_addr = match self.frame_buf_virt_addr {
            Some(addr) => addr,
            None => return,
        };

        let data = color_code.to_color_code(pixel_format);

        if x >= res_x || y >= res_y {
            return;
        }

        unsafe {
            let ptr_mut = frame_buf_virt_addr.offset(offset).as_ptr_mut();
            *ptr_mut = data;
        }
    }

    fn write_str(&mut self, s: &str) -> Result<()> {
        for c in s.chars() {
            self.write_char(c)?;
        }

        Ok(())
    }

    fn write_char(&mut self, c: char) -> Result<()> {
        let (_, char_max_y_len) = self.char_max_xy_len();

        match c {
            '\n' => {
                self.cursor_x = Some(0);
                let mut cursor_y = self.cursor_y.unwrap_or(0) + 1;

                if cursor_y > char_max_y_len {
                    cursor_y = 0;
                }

                self.cursor_y = Some(cursor_y);
                return Ok(());
            }
            '\t' => {
                for _ in 0..4 {
                    self.write_char(' ')?;
                }
                return Ok(());
            }
            _ => (),
        }

        // draw font
        let font_glyph = FONT.get_glyph(c)?;
        let (font_width, font_height) = FONT.get_wh();
        let x = self.cursor_x.unwrap_or(0) * font_width;
        let y = self.cursor_y.unwrap_or(0) * font_height;

        for h in 0..font_height {
            for w in 0..font_width {
                let color_code = if (font_glyph[h] << w) & 0x80 == 0x80 {
                    FORE_COLOR
                } else {
                    BACK_COLOR
                };
                self.write_pixel(x + w, y + h, color_code);
            }
        }

        self.inc_cursor();
        Ok(())
    }
}

impl fmt::Write for PanicScreenDriver {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _ = self.write_str(s);
        Ok(())
    }
}

impl DeviceDriverFunction for PanicScreenDriver {
    type AttachInput = GraphicInfo;
    type PollNormalOutput = ();
    type PollInterruptOutput = ();

    fn get_device_driver_info(&self) -> Result<DeviceDriverInfo> {
        Ok(self.device_driver_info.clone())
    }

    fn probe(&mut self) -> Result<()> {
        Ok(())
    }

    fn attach(&mut self, arg: Self::AttachInput) -> Result<()> {
        self.cursor_x = Some(0);
        self.cursor_y = Some(0);
        self.res_x = Some(arg.resolution.0);
        self.res_y = Some(arg.resolution.1);
        self.pixel_format = Some(arg.format);
        self.frame_buf_virt_addr = Some(arg.framebuf_addr.into());
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
    let driver = unsafe { PANIC_SCREEN_DRIVER.try_lock() }?;
    driver.get_device_driver_info()
}

pub fn probe_and_attach(graphic_info: GraphicInfo) -> Result<()> {
    let mut driver = unsafe { PANIC_SCREEN_DRIVER.try_lock() }?;
    driver.probe()?;
    driver.attach(graphic_info)?;
    let info = driver.get_device_driver_info()?;
    kinfo!("{}: Attached!", info.name);

    Ok(())
}

pub fn open() -> Result<()> {
    let mut driver = unsafe { PANIC_SCREEN_DRIVER.try_lock() }?;
    driver.open()
}

pub fn close() -> Result<()> {
    let mut driver = unsafe { PANIC_SCREEN_DRIVER.try_lock() }?;
    driver.close()
}

pub fn read() -> Result<Vec<u8>> {
    let mut driver = unsafe { PANIC_SCREEN_DRIVER.try_lock() }?;
    driver.read()
}

pub fn write(data: &[u8]) -> Result<()> {
    let mut driver = unsafe { PANIC_SCREEN_DRIVER.try_lock() }?;
    driver.write(data)
}

pub fn write_fmt(args: fmt::Arguments) -> Result<()> {
    let _ = unsafe { PANIC_SCREEN_DRIVER.try_lock() }?.write_fmt(args);
    Ok(())
}
