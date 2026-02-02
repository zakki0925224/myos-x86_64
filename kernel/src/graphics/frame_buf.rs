use crate::{
    arch::VirtualAddress,
    error::{Error, Result},
    graphics::{color::ColorCode, draw::Draw, multi_layer::Layer},
    sync::mutex::Mutex,
};
use alloc::vec::Vec;
use common::{
    geometry::{Point, Rect, Size},
    graphic_info::{GraphicInfo, PixelFormat},
};

static FB: Mutex<FrameBuffer> = Mutex::new(FrameBuffer::new());

struct FrameBuffer {
    resolution: Option<Size>,
    stride: Option<usize>,
    format: Option<PixelFormat>,
    frame_buf_virt_addr: Option<VirtualAddress>,
    shadow_buf: Option<Vec<u32>>,
    dirty: bool,
    updated_rect: Option<Rect>,
}

impl Draw for FrameBuffer {
    fn resolution(&self) -> Result<Size> {
        let res = self.resolution.ok_or_else(|| Error::NotInitialized)?;
        let stride = self.stride.ok_or_else(|| Error::NotInitialized)?;
        Ok(Size::new(stride, res.height))
    }

    fn format(&self) -> Result<PixelFormat> {
        self.format.ok_or_else(|| Error::NotInitialized)
    }

    fn buf_ptr(&self) -> Result<*const u32> {
        if let Some(shadow_buf) = &self.shadow_buf {
            Ok(shadow_buf.as_ptr())
        } else {
            let addr = self
                .frame_buf_virt_addr
                .ok_or_else(|| Error::NotInitialized)?;
            Ok(addr.as_ptr())
        }
    }

    fn buf_ptr_mut(&mut self) -> Result<*mut u32> {
        if let Some(shadow_buf) = &mut self.shadow_buf {
            Ok(shadow_buf.as_mut_ptr())
        } else {
            let addr = self
                .frame_buf_virt_addr
                .ok_or_else(|| Error::NotInitialized)?;
            Ok(addr.as_ptr_mut())
        }
    }

    fn dirty(&self) -> bool {
        self.dirty
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
}

impl FrameBuffer {
    const fn new() -> Self {
        Self {
            resolution: None,
            stride: None,
            format: None,
            frame_buf_virt_addr: None,
            shadow_buf: None,
            dirty: false,
            updated_rect: None,
        }
    }

    fn init(&mut self, graphic_info: &GraphicInfo) -> Result<()> {
        self.resolution = Some(graphic_info.resolution);
        self.stride = Some(graphic_info.stride);
        self.format = Some(graphic_info.format);
        self.frame_buf_virt_addr = Some(graphic_info.framebuf_addr.into());

        Ok(())
    }

    fn enable_shadow_buf(&mut self) -> Result<()> {
        let res = self.resolution()?;
        let buf = vec![0; res.width * res.height];
        self.shadow_buf = Some(buf);

        // copy the current framebuffer to shadow buffer
        let buf_ptr: *mut u32 = self
            .frame_buf_virt_addr
            .ok_or_else(|| Error::NotInitialized)?
            .as_ptr_mut();
        let shadow_buf_ptr = self.buf_ptr_mut()?;

        unsafe {
            buf_ptr.copy_to(shadow_buf_ptr, res.width * res.height);
        }

        Ok(())
    }

    fn apply_shadow_buf(&mut self) -> Result<()> {
        let shadow_buf = match &self.shadow_buf {
            Some(buf) => buf,
            None => return Ok(()),
        };

        if !self.dirty || self.updated_rect.is_none() {
            return Ok(());
        }

        let res = self.resolution()?;
        let rect = self.updated_rect.take().unwrap();

        let draw_x = rect.origin.x.min(res.width);
        let draw_y = rect.origin.y.min(res.height);
        let draw_w = rect.size.width.min(res.width - draw_x);
        let draw_h = rect.size.height.min(res.height - draw_y);

        if draw_w == 0 || draw_h == 0 {
            self.dirty = false;
            return Ok(());
        }

        let fb_ptr: *mut u32 = self
            .frame_buf_virt_addr
            .ok_or_else(|| Error::NotInitialized)?
            .as_ptr_mut();

        unsafe {
            for i in 0..draw_h {
                let offset = (draw_y + i) * res.width + draw_x;
                let src_ptr = shadow_buf.as_ptr().add(offset);
                let dst_ptr = fb_ptr.add(offset);
                core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, draw_w);
            }
        }

        self.dirty = false;

        Ok(())
    }

    fn apply_layer_buf(&mut self, layer: &Layer, keep_rect: Option<Rect>) -> Result<()> {
        let layer_info = layer.layer_info();
        let (layer_x, layer_y) = (layer_info.pos.x, layer_info.pos.y);
        let (layer_w, layer_h) = (layer_info.size.width, layer_info.size.height);
        let res = self.resolution()?;

        let (rect_x, rect_y, rect_w, rect_h) = if let Some(r) = keep_rect {
            (r.origin.x, r.origin.y, r.size.width, r.size.height)
        } else {
            (0, 0, res.width, res.height)
        };

        let intersect_x = layer_x.max(rect_x);
        let intersect_y = layer_y.max(rect_y);
        let intersect_right = (layer_x + layer_w).min(rect_x + rect_w).min(res.width);
        let intersect_bottom = (layer_y + layer_h).min(rect_y + rect_h).min(res.height);

        if intersect_x >= intersect_right || intersect_y >= intersect_bottom {
            return Ok(());
        }

        let draw_w = intersect_right - intersect_x;
        let draw_h = intersect_bottom - intersect_y;

        self.copy_rect_from(
            layer,
            Rect::new(intersect_x - layer_x, intersect_y - layer_y, draw_w, draw_h),
            Point::new(intersect_x, intersect_y),
        )?;

        let new_rect = Rect::new(intersect_x, intersect_y, draw_w, draw_h);
        self.updated_rect = match self.updated_rect {
            Some(curr) => {
                let min_x = curr.origin.x.min(new_rect.origin.x);
                let min_y = curr.origin.y.min(new_rect.origin.y);
                let max_x =
                    (curr.origin.x + curr.size.width).max(new_rect.origin.x + new_rect.size.width);
                let max_y = (curr.origin.y + curr.size.height)
                    .max(new_rect.origin.y + new_rect.size.height);
                Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
            }
            None => Some(new_rect),
        };

        self.dirty = true;
        Ok(())
    }
}

pub fn init(graphic_info: &GraphicInfo) -> Result<()> {
    let mut fb = FB.try_lock()?;
    fb.init(graphic_info)?;
    Ok(())
}

pub fn resolution() -> Result<Size> {
    let fb = FB.try_lock()?;
    fb.resolution()
}

pub fn format() -> Result<PixelFormat> {
    let fb = FB.try_lock()?;
    fb.format()
}

pub fn fill(color: ColorCode) -> Result<()> {
    let mut fb = FB.try_lock()?;
    fb.fill(color)
}

pub fn draw_rect(rect: Rect, color: ColorCode) -> Result<()> {
    let mut fb = FB.try_lock()?;
    fb.draw_rect(rect, color)
}

pub fn copy_rect(src_point: Point, dst_point: Point, size: Size) -> Result<()> {
    let mut fb = FB.try_lock()?;
    fb.copy_rect(src_point, dst_point, size)
}

pub fn draw_char(
    point: Point,
    c: char,
    fore_color: ColorCode,
    back_color: ColorCode,
) -> Result<()> {
    let mut fb = FB.try_lock()?;
    fb.draw_char(point, c, fore_color, back_color)
}

pub fn enable_shadow_buf() -> Result<()> {
    let mut fb = FB.try_lock()?;
    fb.enable_shadow_buf()
}

pub fn apply_shadow_buf() -> Result<()> {
    let mut fb = FB.try_lock()?;
    fb.apply_shadow_buf()
}

pub fn apply_layer_buf(layer: &Layer, keep_rect: Option<Rect>) -> Result<()> {
    let mut fb = FB.try_lock()?;
    fb.apply_layer_buf(layer, keep_rect)
}
