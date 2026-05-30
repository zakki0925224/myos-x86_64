use super::{color::ColorCode, font::FONT};
use crate::error::Result;
use common::geometry::{Point, Rect, Size};
use common::graphic_info::PixelFormat;

#[derive(Debug)]
pub enum DrawError {
    SourcePositionOutOfBounds { point: Point },
    DestinationPositionOutOfBounds { point: Point },
    RectSizeOutOfBounds { size: Size },
    InvalidPixelFormat { src: PixelFormat, dst: PixelFormat },
}

impl core::fmt::Display for DrawError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SourcePositionOutOfBounds { point } => {
                write!(
                    f,
                    "Source position out of bounds: ({}, {})",
                    point.x, point.y
                )
            }
            Self::DestinationPositionOutOfBounds { point } => {
                write!(
                    f,
                    "Destination position out of bounds: ({}, {})",
                    point.x, point.y
                )
            }
            Self::RectSizeOutOfBounds { size } => {
                write!(f, "Rect size out of bounds: {}x{}", size.width, size.height)
            }
            Self::InvalidPixelFormat { src, dst } => {
                write!(f, "Invalid pixel format: src: {:?}, dst: {:?}", src, dst)
            }
        }
    }
}

pub trait Draw {
    // pixel resolution
    fn resolution(&self) -> Result<Size>;

    fn format(&self) -> Result<PixelFormat>;

    fn buf_ptr(&self) -> Result<*const u32>;

    fn buf_ptr_mut(&mut self) -> Result<*mut u32>;

    fn dirty(&self) -> bool;

    fn set_dirty(&mut self, dirty: bool);

    fn extend_dirty_rect(&mut self, rect: Rect) {
        let _ = rect;
        self.set_dirty(true);
    }

    fn draw_pixel(&mut self, point: Point, color: ColorCode) -> Result<()> {
        let res = self.resolution()?;
        let format = self.format()?;
        let buf_ptr = self.buf_ptr_mut()?;
        let code = color.to_color_code(format);
        let (x, y) = point.xy();

        if x > res.width || y > res.height {
            return Err(DrawError::SourcePositionOutOfBounds { point }.into());
        }

        unsafe {
            let pixel_ptr = buf_ptr.add(y * res.width + x);
            pixel_ptr.write(code);
        }

        self.extend_dirty_rect(Rect::new(x, y, 1, 1));
        Ok(())
    }

    fn draw_rect(&mut self, rect: Rect, color: ColorCode) -> Result<()> {
        let res = self.resolution()?;
        let format = self.format()?;
        let buf_ptr = self.buf_ptr_mut()?;
        let code = color.to_color_code(format);
        let (x, y) = rect.origin.xy();
        let (w, h) = rect.size.wh();

        if x > res.width || y > res.height {
            return Err(DrawError::SourcePositionOutOfBounds { point: rect.origin }.into());
        }

        if x + w > res.width || y + h > res.height {
            return Err(DrawError::RectSizeOutOfBounds { size: rect.size }.into());
        }

        unsafe {
            let mut ptr = buf_ptr.add(y * res.width + x);

            // write the first line
            core::slice::from_raw_parts_mut(ptr, w).fill(code);

            // copy the first line to the rest
            // SAFETY: We already checked bounds. The rect fits in the buffer.
            for _ in 1..h {
                let src = ptr;
                ptr = ptr.add(res.width);
                src.copy_to_nonoverlapping(ptr, w);
            }
        }

        self.extend_dirty_rect(rect);
        Ok(())
    }

    fn copy_rect(&mut self, src_point: Point, dst_point: Point, size: Size) -> Result<()> {
        let res = self.resolution()?;
        let buf_ptr = self.buf_ptr_mut()?;

        if src_point.x > res.width || src_point.y > res.height {
            return Err(DrawError::SourcePositionOutOfBounds { point: src_point }.into());
        }

        if dst_point.x > res.width || dst_point.y > res.height {
            return Err(DrawError::DestinationPositionOutOfBounds { point: dst_point }.into());
        }

        if src_point.x + size.width > res.width || src_point.y + size.height > res.height {
            return Err(DrawError::RectSizeOutOfBounds { size }.into());
        }

        unsafe {
            let src_buf_ptr = buf_ptr.add(src_point.y * res.width + src_point.x);
            let dst_buf_ptr = buf_ptr.add(dst_point.y * res.width + dst_point.x);

            for i in 0..size.height {
                let src_line_ptr = src_buf_ptr.add(i * res.width);
                let dst_line_ptr = dst_buf_ptr.add(i * res.width);
                src_line_ptr.copy_to(dst_line_ptr, size.width);
            }
        }

        self.extend_dirty_rect(Rect::from_point_and_size(dst_point, size));
        Ok(())
    }

    fn fill(&mut self, color: ColorCode) -> Result<()> {
        let res = self.resolution()?;
        let count = res.width * res.height;
        let format = self.format()?;
        let buf_ptr = self.buf_ptr_mut()?;
        let code = color.to_color_code(format);

        unsafe {
            // write once
            buf_ptr.write(code);

            let mut copied = 1;
            while copied < count {
                let write_count = (count - copied).min(copied);
                buf_ptr.copy_to(buf_ptr.add(copied), write_count);
                copied += write_count;
            }
        }

        self.extend_dirty_rect(Rect::new(0, 0, res.width, res.height));
        Ok(())
    }

    fn draw_char(
        &mut self,
        point: Point,
        c: char,
        fore_color: ColorCode,
        back_color: ColorCode,
    ) -> Result<()> {
        let res = self.resolution()?;
        let (f_w, f_h) = FONT.get_wh();
        let f_glyph = FONT.get_glyph(c)?;
        let (x, y) = point.xy();

        if x >= res.width || y >= res.height {
            return Ok(());
        }

        let format = self.format()?;
        let buf_ptr = self.buf_ptr_mut()?;
        let fore_code = fore_color.to_color_code(format);
        let back_code = back_color.to_color_code(format);

        // clipping
        let draw_w = (f_w).min(res.width - x);
        let draw_h = (f_h).min(res.height - y);

        if draw_w == 0 || draw_h == 0 {
            return Ok(());
        }

        unsafe {
            let mut ptr = buf_ptr.add(y * res.width + x);
            let mut row_buf = [0u32; 8];

            for h in 0..draw_h {
                let line = f_glyph[h];
                for w in 0..draw_w {
                    row_buf[w] = if (line << w) & 0x80 != 0 {
                        fore_code
                    } else {
                        back_code
                    };
                }
                core::slice::from_raw_parts_mut(ptr, draw_w).copy_from_slice(&row_buf[..draw_w]);
                ptr = ptr.add(res.width);
            }
        }

        self.extend_dirty_rect(Rect::new(x, y, draw_w, draw_h));
        Ok(())
    }

    fn draw_string_wrap(
        &mut self,
        point: Point,
        s: &str,
        fore_color: ColorCode,
        back_color: ColorCode,
    ) -> Result<()> {
        let res = self.resolution()?;
        let (mut x, mut y) = point.xy();
        let (f_w, f_h) = FONT.get_wh();

        for c in s.chars() {
            match c {
                '\n' => {
                    x = point.x;
                    y += f_h;
                }
                '\t' => {
                    x += f_w * 4;
                }
                _ => (),
            }

            self.draw_char(Point::new(x, y), c, fore_color, back_color)?;
            x += f_w;

            if x + f_w > res.width {
                x = point.x;
                y += f_h;
            }
        }

        Ok(())
    }

    fn draw_line(&mut self, start: Point, end: Point, color: ColorCode) -> Result<()> {
        let res = self.resolution()?;
        let format = self.format()?;
        let buf_ptr = self.buf_ptr_mut()?;
        let code = color.to_color_code(format);

        let (cx0, cy0, cx1, cy1) = match clip_line(
            start.x as isize,
            start.y as isize,
            end.x as isize,
            end.y as isize,
            res.width as isize - 1,
            res.height as isize - 1,
        ) {
            Some(coords) => coords,
            None => return Ok(()),
        };

        let (mut x0, mut y0) = (cx0, cy0);
        let (x1, y1) = (cx1, cy1);

        let dx = (x1 as isize - x0 as isize).abs();
        let dy = -(y1 as isize - y0 as isize).abs();
        let sx: isize = if x0 < x1 { 1 } else { -1 };
        let sy: isize = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        unsafe {
            loop {
                buf_ptr.add(y0 * res.width + x0).write(code);

                if x0 == x1 && y0 == y1 {
                    break;
                }
                let e2 = 2 * err;
                if e2 >= dy {
                    err += dy;
                    x0 = (x0 as isize + sx) as usize;
                }
                if e2 <= dx {
                    err += dx;
                    y0 = (y0 as isize + sy) as usize;
                }
            }
        }

        let min_x = cx0.min(cx1);
        let min_y = cy0.min(cy1);
        let max_x = cx0.max(cx1);
        let max_y = cy0.max(cy1);
        self.extend_dirty_rect(Rect::new(
            min_x,
            min_y,
            max_x - min_x + 1,
            max_y - min_y + 1,
        ));
        Ok(())
    }

    fn copy_rect_from(&mut self, src: &dyn Draw, src_rect: Rect, dst_point: Point) -> Result<()> {
        let (src_x, src_y) = src_rect.origin.xy();
        let (src_w, src_h) = src_rect.size.wh();
        let (dst_x, dst_y) = dst_point.xy();

        let res = self.resolution()?;
        let src_res = src.resolution()?;

        if src.format()? != self.format()? {
            return Err(DrawError::InvalidPixelFormat {
                src: src.format()?,
                dst: self.format()?,
            }
            .into());
        }

        let clip_src_x = src_x.min(src_res.width);
        let clip_src_y = src_y.min(src_res.height);
        let clip_src_w = (src_x + src_w).min(src_res.width) - clip_src_x;
        let clip_src_h = (src_y + src_h).min(src_res.height) - clip_src_y;

        let clip_dst_x = dst_x.min(res.width);
        let clip_dst_y = dst_y.min(res.height);

        let copy_w = clip_src_w.min(res.width - clip_dst_x);
        let copy_h = clip_src_h.min(res.height - clip_dst_y);

        if copy_w == 0 || copy_h == 0 {
            return Ok(());
        }

        let src_buf_ptr = src.buf_ptr()?;
        let dst_buf_ptr = self.buf_ptr_mut()?;
        let src_stride = src_res.width;
        let dst_stride = res.width;

        unsafe {
            for i in 0..copy_h {
                let src_offset = (clip_src_y + i) * src_stride + clip_src_x;
                let dst_offset = (clip_dst_y + i) * dst_stride + clip_dst_x;
                let src_ptr = src_buf_ptr.add(src_offset);
                let dst_ptr = dst_buf_ptr.add(dst_offset);
                src_ptr.copy_to(dst_ptr, copy_w);
            }
        }

        self.extend_dirty_rect(Rect::new(clip_dst_x, clip_dst_y, copy_w, copy_h));
        Ok(())
    }

    unsafe fn copy_from_slice_u32(&mut self, src: &[u32]) -> Result<()> {
        core::ptr::copy_nonoverlapping(src.as_ptr(), self.buf_ptr_mut()?, src.len());
        let res = self.resolution()?;
        self.extend_dirty_rect(Rect::new(0, 0, res.width, res.height));
        Ok(())
    }
}

fn clip_line(
    mut x0: isize,
    mut y0: isize,
    mut x1: isize,
    mut y1: isize,
    xmax: isize,
    ymax: isize,
) -> Option<(usize, usize, usize, usize)> {
    const INSIDE: u8 = 0;
    const LEFT: u8 = 1;
    const RIGHT: u8 = 2;
    const TOP: u8 = 4;
    const BOTTOM: u8 = 8;

    let outcode = |x: isize, y: isize| -> u8 {
        let mut code = INSIDE;
        if x < 0 {
            code |= LEFT;
        } else if x > xmax {
            code |= RIGHT;
        }
        if y < 0 {
            code |= TOP;
        } else if y > ymax {
            code |= BOTTOM;
        }
        code
    };

    let mut out0 = outcode(x0, y0);
    let mut out1 = outcode(x1, y1);

    loop {
        if out0 | out1 == 0 {
            return Some((x0 as usize, y0 as usize, x1 as usize, y1 as usize));
        }
        if out0 & out1 != 0 {
            return None;
        }

        let out_clip = if out0 != 0 { out0 } else { out1 };
        let dx = x1 - x0;
        let dy = y1 - y0;

        let (x, y) = if out_clip & BOTTOM != 0 {
            let x = if dy != 0 {
                x0 + dx * (ymax - y0) / dy
            } else {
                x0
            };
            (x, ymax)
        } else if out_clip & TOP != 0 {
            let x = if dy != 0 { x0 - dx * y0 / dy } else { x0 };
            (x, 0)
        } else if out_clip & RIGHT != 0 {
            let y = if dx != 0 {
                y0 + dy * (xmax - x0) / dx
            } else {
                y0
            };
            (xmax, y)
        } else {
            // LEFT
            let y = if dx != 0 { y0 - dy * x0 / dx } else { y0 };
            (0, y)
        };

        if out_clip == out0 {
            x0 = x;
            y0 = y;
            out0 = outcode(x0, y0);
        } else {
            x1 = x;
            y1 = y;
            out1 = outcode(x1, y1);
        }
    }
}
