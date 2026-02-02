use super::{color::ColorCode, font::FONT};
use crate::error::Result;
use common::graphic_info::PixelFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawError {
    SourcePositionOutOfBounds { x: usize, y: usize },
    DestinationPositionOutOfBounds { x: usize, y: usize },
    RectSizeOutOfBounds { w: usize, h: usize },
    InvalidPixelFormat { src: PixelFormat, dst: PixelFormat },
}

pub trait Draw {
    // pixel resolution (width, height)
    fn resolution(&self) -> Result<(usize, usize)>;

    fn format(&self) -> Result<PixelFormat>;

    fn buf_ptr(&self) -> Result<*const u32>;

    fn buf_ptr_mut(&mut self) -> Result<*mut u32>;

    fn dirty(&self) -> bool;

    fn set_dirty(&mut self, dirty: bool);

    fn draw_pixel(&mut self, xy: (usize, usize), color: ColorCode) -> Result<()> {
        let (res_w, res_h) = self.resolution()?;
        let (x, y) = xy;
        let format = self.format()?;
        let buf_ptr = self.buf_ptr_mut()?;
        let code = color.to_color_code(format);

        if x > res_w || y > res_h {
            return Err(DrawError::SourcePositionOutOfBounds { x, y }.into());
        }

        unsafe {
            let pixel_ptr = buf_ptr.add(y * res_w + x);
            pixel_ptr.write(code);
        }

        self.set_dirty(true);
        Ok(())
    }

    fn draw_rect(
        &mut self,
        xy: (usize, usize),
        wh: (usize, usize),
        color: ColorCode,
    ) -> Result<()> {
        let (x, y) = xy;
        let (w, h) = wh;
        let (res_w, res_h) = self.resolution()?;
        let format = self.format()?;
        let buf_ptr = self.buf_ptr_mut()?;
        let code = color.to_color_code(format);

        if x > res_w || y > res_h {
            return Err(DrawError::SourcePositionOutOfBounds { x, y }.into());
        }

        if x + w > res_w || y + h > res_h {
            return Err(DrawError::RectSizeOutOfBounds { w, h }.into());
        }

        unsafe {
            let mut ptr = buf_ptr.add(y * res_w + x);

            // write the first line
            core::slice::from_raw_parts_mut(ptr, w).fill(code);

            // copy the first line to the rest
            // SAFETY: We already checked bounds. The rect fits in the buffer.
            for _ in 1..h {
                let src = ptr;
                ptr = ptr.add(res_w);
                src.copy_to_nonoverlapping(ptr, w);
            }
        }

        self.set_dirty(true);
        Ok(())
    }

    fn copy_rect(
        &mut self,
        src_xy: (usize, usize),
        dst_xy: (usize, usize),
        wh: (usize, usize),
    ) -> Result<()> {
        let (src_x, src_y) = src_xy;
        let (dst_x, dst_y) = dst_xy;
        let (w, h) = wh;

        let (res_w, res_h) = self.resolution()?;
        let buf_ptr = self.buf_ptr_mut()?;

        if src_x > res_w || src_y > res_h {
            return Err(DrawError::SourcePositionOutOfBounds { x: src_x, y: src_y }.into());
        }

        if dst_x > res_w || dst_y > res_h {
            return Err(DrawError::DestinationPositionOutOfBounds { x: dst_x, y: dst_y }.into());
        }

        if src_x + w > res_w || src_y + h > res_h {
            return Err(DrawError::RectSizeOutOfBounds { w, h }.into());
        }

        unsafe {
            let src_buf_ptr = buf_ptr.add(src_y * res_w + src_x);
            let dst_buf_ptr = buf_ptr.add(dst_y * res_w + dst_x);

            for i in 0..h {
                let src_line_ptr = src_buf_ptr.add(i * res_w);
                let dst_line_ptr = dst_buf_ptr.add(i * res_w);
                src_line_ptr.copy_to(dst_line_ptr, w);
            }
        }

        self.set_dirty(true);
        Ok(())
    }

    fn fill(&mut self, color: ColorCode) -> Result<()> {
        let (res_w, res_h) = self.resolution()?;
        let count = res_w * res_h;
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
        xy: (usize, usize),
        c: char,
        fore_color: ColorCode,
        back_color: ColorCode,
    ) -> Result<()> {
        let (res_w, res_h) = self.resolution()?;
        let (x, y) = xy;
        let (f_w, f_h) = FONT.get_wh();
        let f_glyph = FONT.get_glyph(c)?;

        if x >= res_w || y >= res_h {
            return Ok(());
        }

        let format = self.format()?;
        let buf_ptr = self.buf_ptr_mut()?;
        let fore_code = fore_color.to_color_code(format);
        let back_code = back_color.to_color_code(format);

        // clipping
        let draw_w = (f_w).min(res_w - x);
        let draw_h = (f_h).min(res_h - y);

        if draw_w == 0 || draw_h == 0 {
            return Ok(());
        }

        unsafe {
            let mut ptr = buf_ptr.add(y * res_w + x);

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
                ptr = ptr.add(res_w);
            }
        }

        self.set_dirty(true);
        Ok(())
    }

    fn draw_string_wrap(
        &mut self,
        xy: (usize, usize),
        s: &str,
        fore_color: ColorCode,
        back_color: ColorCode,
    ) -> Result<()> {
        let (res_w, _) = self.resolution()?;
        let (mut x, mut y) = xy;
        let (f_w, f_h) = FONT.get_wh();

        for c in s.chars() {
            match c {
                '\n' => {
                    x = xy.0;
                    y += f_h;
                }
                '\t' => {
                    x += f_w * 4;
                }
                _ => (),
            }

            self.draw_char((x, y), c, fore_color, back_color)?;
            x += f_w;

            if x + f_w > res_w {
                x = xy.0;
                y += f_h;
            }
        }

        self.set_dirty(true);
        Ok(())
    }

    fn draw_line(
        &mut self,
        start_xy: (usize, usize),
        end_xy: (usize, usize),
        color: ColorCode,
    ) -> Result<()> {
        let (mut x0, mut y0) = start_xy;
        let (x1, y1) = end_xy;
        let (res_w, res_h) = self.resolution()?;

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
                if x0 < res_w && y0 < res_h {
                    buf_ptr.add(y0 * res_w + x0).write(code);
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

    fn copy_rect_from(
        &mut self,
        src: &dyn Draw,
        src_rect: (usize, usize, usize, usize),
        dst_xy: (usize, usize),
    ) -> Result<()> {
        let (src_x, src_y, src_w, src_h) = src_rect;
        let (dst_x, dst_y) = dst_xy;

        let (res_w, res_h) = self.resolution()?;
        let (src_res_w, src_res_h) = src.resolution()?;

        if src.format()? != self.format()? {
            return Err(DrawError::InvalidPixelFormat {
                src: src.format()?,
                dst: self.format()?,
            }
            .into());
        }

        let clip_src_x = src_x.min(src_res_w);
        let clip_src_y = src_y.min(src_res_h);
        let clip_src_w = (src_x + src_w).min(src_res_w) - clip_src_x;
        let clip_src_h = (src_y + src_h).min(src_res_h) - clip_src_y;

        let clip_dst_x = dst_x.min(res_w);
        let clip_dst_y = dst_y.min(res_h);

        let copy_w = clip_src_w.min(res_w - clip_dst_x);
        let copy_h = clip_src_h.min(res_h - clip_dst_y);

        if copy_w == 0 || copy_h == 0 {
            return Ok(());
        }

        let src_buf_ptr = src.buf_ptr()?;
        let dst_buf_ptr = self.buf_ptr_mut()?;
        let src_stride = src_res_w;
        let dst_stride = res_w;

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
