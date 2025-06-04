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
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub const RED: Self = Self {
        r: 255,
        g: 0,
        b: 0,
        a: 0,
    };

    pub const GREEN: Self = Self {
        r: 0,
        g: 255,
        b: 0,
        a: 0,
    };

    pub const YELLOW: Self = Self {
        r: 255,
        g: 255,
        b: 0,
        a: 0,
    };

    pub const BLUE: Self = Self {
        r: 0,
        g: 0,
        b: 255,
        a: 0,
    };

    pub const MAGENTA: Self = Self {
        r: 255,
        g: 0,
        b: 255,
        a: 0,
    };

    pub const CYAN: Self = Self {
        r: 0,
        g: 255,
        b: 255,
        a: 0,
    };

    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 0,
    };

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

    pub fn from_pixel_data(data: u32, pixel_format: PixelFormat) -> Self {
        match pixel_format {
            PixelFormat::Bgr => Self {
                r: (data >> 16) as u8,
                g: (data >> 8) as u8,
                b: (data >> 0) as u8,
                a: 0,
            },
            PixelFormat::Rgb => Self {
                r: (data >> 0) as u8,
                g: (data >> 8) as u8,
                b: (data >> 16) as u8,
                a: 0,
            },
            PixelFormat::Bgra => Self {
                r: (data >> 16) as u8,
                g: (data >> 8) as u8,
                b: (data >> 0) as u8,
                a: (data >> 24) as u8,
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
