use crate::geometry::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PixelFormat {
    Rgb = 0,
    Bgr = 1,
    Bgra = 2,
}

impl From<u8> for PixelFormat {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Rgb,
            1 => Self::Bgr,
            2 => Self::Bgra,
            _ => panic!("Invalid pixel format"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GraphicInfo {
    pub resolution: Size,
    pub format: PixelFormat,
    pub stride: usize,
    pub framebuf_addr: u64,
    pub framebuf_size: usize,
}

impl GraphicInfo {
    pub fn fill_screen(&self, r: u8, g: u8, b: u8) {
        let (w, h) = self.resolution.wh();
        let framebuf_slice = unsafe {
            core::slice::from_raw_parts_mut(self.framebuf_addr as *mut u32, h * self.stride)
        };

        let pixel = match self.format {
            PixelFormat::Rgb => ((b as u32) << 16) | ((g as u32) << 8) | (r as u32),
            PixelFormat::Bgr => ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
            _ => panic!("Unsupported pixel format"),
        };

        for y in 0..h {
            for x in 0..w {
                framebuf_slice[y * self.stride + x] = pixel;
            }
        }
    }
}
