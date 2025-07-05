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
            let pixel_ptr = buf_ptr.add(y * res_w + x);

            // write the first line
            for i in 0..w {
                pixel_ptr.add(i).write(code);
            }

            // copy the first line
            for i in 1..h {
                self.copy_rect(xy, (x, y + i), (w, 1))?;
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
            let src_ptr = buf_ptr.add(src_y * res_w + src_x);
            let dst_ptr = buf_ptr.add(dst_y * res_w + dst_x);
            src_ptr.copy_to(dst_ptr, w * h);
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

        if x > res_w || y > res_h {
            return Err(DrawError::SourcePositionOutOfBounds { x, y }.into());
        }

        if x + f_w > res_w || y + f_h > res_h {
            return Err(DrawError::RectSizeOutOfBounds { w: f_w, h: f_h }.into());
        }

        for h in 0..f_h {
            let line = f_glyph[h];
            for w in 0..f_w {
                let color = if (line << w) & 0x80 != 0 {
                    fore_color
                } else {
                    back_color
                };
                self.draw_pixel((x + w, y + h), color)?;
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
        let dx = (x1 as isize - x0 as isize).abs();
        let dy = -(y1 as isize - y0 as isize).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.draw_pixel((x0, y0), color)?;
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

        self.set_dirty(true);
        Ok(())
    }

    fn copy_to(&self, dst: &mut dyn Draw, dst_xy: (usize, usize)) -> Result<()> {
        let (dst_x, dst_y) = dst_xy;
        let (src_w, src_h) = self.resolution()?;
        let (dst_w, dst_h) = dst.resolution()?;
        let src_buf_ptr = self.buf_ptr()?;
        let dst_buf_ptr = dst.buf_ptr_mut()?;
        let src_format = self.format()?;
        let dst_format = dst.format()?;

        if src_format != dst_format {
            return Err(DrawError::InvalidPixelFormat {
                src: src_format,
                dst: dst_format,
            }
            .into());
        }

        if dst_x + src_w > dst_w || dst_y + src_h > dst_h {
            return Err(DrawError::DestinationPositionOutOfBounds { x: dst_x, y: dst_y }.into());
        }

        unsafe {
            for i in 0..src_h {
                let src_line_ptr = src_buf_ptr.add(i * src_w);
                let dst_line_ptr = dst_buf_ptr.add((dst_y + i) * dst_w + dst_x);
                src_line_ptr.copy_to(dst_line_ptr, src_w);
            }
        }

        dst.set_dirty(true);
        Ok(())
    }

    unsafe fn copy_from_slice_u32(&mut self, src: &[u32]) -> Result<()> {
        core::ptr::copy_nonoverlapping(src.as_ptr(), self.buf_ptr_mut()?, src.len());
        self.set_dirty(true);
        Ok(())
    }
}
