use common::graphic_info::PixelFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ColorCode {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorCode {
    pub const BLACK: Self = Self::new_rgb(0, 0, 0);
    pub const RED: Self = Self::new_rgb(255, 0, 0);
    pub const GREEN: Self = Self::new_rgb(0, 255, 0);
    pub const YELLOW: Self = Self::new_rgb(255, 255, 0);
    pub const BLUE: Self = Self::new_rgb(0, 0, 255);
    pub const MAGENTA: Self = Self::new_rgb(255, 0, 255);
    pub const CYAN: Self = Self::new_rgb(0, 255, 255);
    pub const WHITE: Self = Self::new_rgb(255, 255, 255);

    pub const fn default() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    }

    pub const fn new_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 0 }
    }

    pub const fn new_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_pixel_data(data: &[u8], pixel_format: PixelFormat) -> Self {
        match pixel_format {
            PixelFormat::Bgr => Self {
                r: data[2],
                g: data[1],
                b: data[0],
                a: 0,
            },
            PixelFormat::Rgb => Self {
                r: data[0],
                g: data[1],
                b: data[2],
                a: 0,
            },
            PixelFormat::Bgra => Self {
                r: data[2],
                g: data[1],
                b: data[0],
                a: data[3],
            },
        }
    }

    pub fn to_color_code(&self, pixel_format: PixelFormat) -> u32 {
        match pixel_format {
            PixelFormat::Bgr => (self.r as u32) << 16 | (self.g as u32) << 8 | (self.b as u32) << 0,
            PixelFormat::Rgb => (self.r as u32) << 0 | (self.g as u32) << 8 | (self.b as u32) << 16,
            PixelFormat::Bgra => {
                (self.r as u32) << 16
                    | (self.g as u32) << 8
                    | (self.b as u32) << 0
                    | (self.a as u32) << 24
            }
        }
    }
}
