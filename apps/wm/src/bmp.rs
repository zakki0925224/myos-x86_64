use alloc::vec::Vec;

const MAGIC: [u8; 2] = *b"BM";
const FILE_HEADER_SIZE: usize = 14;

#[derive(Debug)]
pub enum BmpError {
    TooShort,
    InvalidMagic,
    UnsupportedBitsPerPixel(u16),
    UnsupportedCompression(u32),
}

impl core::fmt::Display for BmpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort => write!(f, "file is too short"),
            Self::InvalidMagic => write!(f, "invalid magic"),
            Self::UnsupportedBitsPerPixel(bpp) => write!(f, "unsupported bits per pixel: {}", bpp),
            Self::UnsupportedCompression(c) => write!(f, "unsupported compression: {}", c),
        }
    }
}

pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, BmpError> {
    let bytes = data.get(offset..offset + 2).ok_or(BmpError::TooShort)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, BmpError> {
    let bytes = data.get(offset..offset + 4).ok_or(BmpError::TooShort)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

impl Bitmap {
    pub fn parse(data: &[u8]) -> Result<Self, BmpError> {
        if data.get(0..2) != Some(&MAGIC[..]) {
            return Err(BmpError::InvalidMagic);
        }

        let pixel_offset = read_u32(data, 10)? as usize;
        let width = read_u32(data, FILE_HEADER_SIZE + 4)? as i32;
        let height = read_u32(data, FILE_HEADER_SIZE + 8)? as i32;
        let bits_per_pixel = read_u16(data, FILE_HEADER_SIZE + 14)?;
        let compression = read_u32(data, FILE_HEADER_SIZE + 16)?;

        if bits_per_pixel != 24 && bits_per_pixel != 32 {
            return Err(BmpError::UnsupportedBitsPerPixel(bits_per_pixel));
        }

        let bytes_per_pixel = bits_per_pixel as usize / 8;
        if compression != 0 && !(compression == 3 && bytes_per_pixel == 4) {
            return Err(BmpError::UnsupportedCompression(compression));
        }

        let width = width.unsigned_abs() as usize;
        let top_down = height < 0;
        let height = height.unsigned_abs() as usize;
        let stride = (width * bytes_per_pixel).div_ceil(4) * 4;

        let pixel_data = data.get(pixel_offset..).ok_or(BmpError::TooShort)?;
        if pixel_data.len() < stride * height {
            return Err(BmpError::TooShort);
        }

        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            let row = if top_down { y } else { height - y - 1 };
            for x in 0..width {
                let offset = row * stride + x * bytes_per_pixel;
                let b = pixel_data[offset] as u32;
                let g = pixel_data[offset + 1] as u32;
                let r = pixel_data[offset + 2] as u32;
                pixels.push((r << 16) | (g << 8) | b);
            }
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }
}
