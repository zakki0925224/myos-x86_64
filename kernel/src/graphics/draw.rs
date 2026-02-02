use super::{color::ColorCode, font::FONT};
use crate::error::Result;
use common::geometry::{Point, Rect, Size};
use common::graphic_info::PixelFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawError {
    SourcePositionOutOfBounds { point: Point },
    DestinationPositionOutOfBounds { point: Point },
    RectSizeOutOfBounds { size: Size },
    InvalidPixelFormat { src: PixelFormat, dst: PixelFormat },
}

pub trait Draw {
    // pixel resolution
    fn resolution(&self) -> Result<Size>;

    fn format(&self) -> Result<PixelFormat>;

    fn buf_ptr(&self) -> Result<*const u32>;

    fn buf_ptr_mut(&mut self) -> Result<*mut u32>;

    fn dirty(&self) -> bool;

    fn set_dirty(&mut self, dirty: bool);

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

        self.set_dirty(true);
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

        self.set_dirty(true);
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

        self.set_dirty(true);
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

        self.set_dirty(true);
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

            for h in 0..draw_h {
                let line = f_glyph[h];
                for w in 0..draw_w {
                    let color_code = if (line << w) & 0x80 != 0 {
                        fore_code
                    } else {
                        back_code
                    };
                    ptr.add(w).write(color_code);
                }
                ptr = ptr.add(res.width);
            }
        }

        self.set_dirty(true);
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

        self.set_dirty(true);
        Ok(())
    }

    fn draw_line(&mut self, start: Point, end: Point, color: ColorCode) -> Result<()> {
        let (mut x0, mut y0) = start.xy();
        let (x1, y1) = end.xy();
        let res = self.resolution()?;

        // Clipping: Skip if both start and end are completely out of visible area (rough check)
        // Note: Ideally, we should use line clipping algorithm like Cohen-Sutherland.
        // For now, allow drawing as long as we check bounds per pixel or allow partial out-of-bounds.

        let format = self.format()?;
        let buf_ptr = self.buf_ptr_mut()?;
        let code = color.to_color_code(format);

        let dx = (x1 as isize - x0 as isize).abs();
        let dy = -(y1 as isize - y0 as isize).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        unsafe {
            loop {
                // Check bounds for each pixel to allow clipping
                if x0 < res.width && y0 < res.height {
                    buf_ptr.add(y0 * res.width + x0).write(code);
                }

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

        self.set_dirty(true);
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

        self.set_dirty(true);
        Ok(())
    }

    unsafe fn copy_from_slice_u32(&mut self, src: &[u32]) -> Result<()> {
        core::ptr::copy_nonoverlapping(src.as_ptr(), self.buf_ptr_mut()?, src.len());
        self.set_dirty(true);
        Ok(())
    }
}
