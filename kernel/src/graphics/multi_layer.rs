use super::{draw::Draw, frame_buf};
use crate::{error::Result, fs::file::bitmap::BitmapImage, sync::mutex::Mutex};
use alloc::vec::Vec;
use common::geometry::{Point, Rect, Size};
use common::graphic_info::PixelFormat;
use core::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

static LAYER_MAN: Mutex<LayerManager> = Mutex::new(LayerManager::new());

#[derive(Debug)]
pub enum LayerError {
    OutsideBufferArea { layer_id: usize, point: Point },
    InvalidLayerId(usize),
}

impl core::fmt::Display for LayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideBufferArea { layer_id, point } => {
                write!(
                    f,
                    "Point {:?} is outside buffer area of layer {}",
                    point, layer_id
                )
            }
            Self::InvalidLayerId(id) => {
                write!(f, "Invalid layer ID: {}", id)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub pos: Point,
    pub size: Size,
    pub format: PixelFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId(usize);

impl fmt::Display for LayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl LayerId {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

impl From<usize> for LayerId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
pub struct Layer {
    pub id: LayerId,
    pos: Point,
    size: Size,
    buf: Vec<u32>,
    pub disabled: bool,
    format: PixelFormat,
    pub always_on_top: bool,
    dirty_rect: Option<Rect>,
    pos_moved: bool,
    old_pos: Option<Point>,
}

impl Draw for Layer {
    fn resolution(&self) -> Result<Size> {
        Ok(self.size)
    }

    fn format(&self) -> Result<PixelFormat> {
        Ok(self.format)
    }

    fn buf_ptr(&self) -> Result<*const u32> {
        Ok(self.buf.as_ptr())
    }

    fn buf_ptr_mut(&mut self) -> Result<*mut u32> {
        Ok(self.buf.as_mut_ptr())
    }

    fn dirty(&self) -> bool {
        self.dirty_rect.is_some()
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.dirty_rect = if dirty {
            Some(Rect::new(0, 0, self.size.width, self.size.height))
        } else {
            None
        };
    }

    fn extend_dirty_rect(&mut self, rect: Rect) {
        self.dirty_rect = Some(match self.dirty_rect {
            Some(curr) => {
                let min_x = curr.origin.x.min(rect.origin.x);
                let min_y = curr.origin.y.min(rect.origin.y);
                let max_x = (curr.origin.x + curr.size.width).max(rect.origin.x + rect.size.width);
                let max_y =
                    (curr.origin.y + curr.size.height).max(rect.origin.y + rect.size.height);
                Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
            }
            None => rect,
        });
    }
}

impl Layer {
    pub fn new(pos: Point, size: Size, format: PixelFormat) -> Self {
        Self {
            id: LayerId::new(),
            pos,
            size,
            buf: vec![0; size.width * size.height],
            disabled: false,
            format,
            always_on_top: false,
            dirty_rect: None,
            pos_moved: false,
            old_pos: None,
        }
    }

    pub fn move_to(&mut self, point: Point) {
        if self.pos == point {
            return;
        }

        if !self.pos_moved {
            self.old_pos = Some(self.pos);
        }

        self.pos = point;
        self.pos_moved = true;
    }

    pub fn layer_info(&self) -> LayerInfo {
        LayerInfo {
            pos: self.pos,
            size: self.size,
            format: self.format,
        }
    }
}

struct LayerManager {
    layers: Vec<Layer>,
}

impl LayerManager {
    const fn new() -> Self {
        Self { layers: Vec::new() }
    }

    fn push_layer(&mut self, layer: Layer) {
        if layer.always_on_top {
            self.layers.push(layer);
        } else {
            let insert_at = self
                .layers
                .iter()
                .position(|l| l.always_on_top)
                .unwrap_or(self.layers.len());
            self.layers.insert(insert_at, layer);
        }
    }

    fn remove_layer(&mut self, layer_id: LayerId) -> Result<()> {
        let removed = match self.layers.iter().find(|l| l.id == layer_id) {
            Some(l) => l,
            None => return Err(LayerError::InvalidLayerId(layer_id.0).into()),
        };

        let r_x1 = removed.pos.x;
        let r_y1 = removed.pos.y;
        let r_x2 = removed.pos.x + removed.size.width;
        let r_y2 = removed.pos.y + removed.size.height;

        self.layers.retain(|l| l.id != layer_id);

        for layer in &mut self.layers {
            if layer.disabled {
                continue;
            }

            let l_x1 = layer.pos.x;
            let l_y1 = layer.pos.y;
            let l_x2 = layer.pos.x + layer.size.width;
            let l_y2 = layer.pos.y + layer.size.height;

            let ix1 = r_x1.max(l_x1);
            let iy1 = r_y1.max(l_y1);
            let ix2 = r_x2.min(l_x2);
            let iy2 = r_y2.min(l_y2);

            if ix2 > ix1 && iy2 > iy1 {
                layer.extend_dirty_rect(Rect::new(ix1 - l_x1, iy1 - l_y1, ix2 - ix1, iy2 - iy1));
            }
        }

        Ok(())
    }

    fn bring_layer_to_front(&mut self, layer_id: LayerId) -> Result<()> {
        let index = match self.layers.iter().position(|l| l.id == layer_id) {
            Some(i) => i,
            None => return Err(LayerError::InvalidLayerId(layer_id.0).into()),
        };
        let layer = self.layers.remove(index);

        if layer.always_on_top {
            self.layers.push(layer);
        } else {
            let insert_at = self
                .layers
                .iter()
                .position(|l| l.always_on_top)
                .unwrap_or(self.layers.len());
            self.layers.insert(insert_at, layer);
        }

        for l in &mut self.layers {
            l.set_dirty(true);
        }

        Ok(())
    }

    fn layer(&mut self, layer_id: LayerId) -> Result<&mut Layer> {
        self.layers
            .iter_mut()
            .find(|l| l.id == layer_id)
            .ok_or(LayerError::InvalidLayerId(layer_id.0).into())
    }

    fn draw_to_frame_buf(&mut self) -> Result<()> {
        let mut invalid_rect: Option<Rect> = None;

        for layer in &self.layers {
            if layer.disabled {
                continue;
            }

            if let Some(local_dirty) = layer.dirty_rect {
                let screen_rect = Rect::new(
                    layer.pos.x + local_dirty.origin.x,
                    layer.pos.y + local_dirty.origin.y,
                    local_dirty.size.width,
                    local_dirty.size.height,
                );
                invalid_rect = merge_rect(invalid_rect, screen_rect);
            }

            if layer.pos_moved {
                let rect = Rect::from_point_and_size(layer.pos, layer.size);
                invalid_rect = merge_rect(invalid_rect, rect);

                if let Some(old_pos) = layer.old_pos {
                    let old_rect = Rect::from_point_and_size(old_pos, layer.size);
                    invalid_rect = merge_rect(invalid_rect, old_rect);
                }
            }
        }

        let rect = match invalid_rect {
            Some(r) => r,
            None => return Ok(()),
        };

        for layer in &mut self.layers {
            if layer.disabled {
                continue;
            }

            frame_buf::apply_layer_buf(layer, Some(rect))?;

            layer.set_dirty(false);
            layer.pos_moved = false;
            layer.old_pos = None;
        }

        frame_buf::flush_rect_to_vram(rect)?;
        Ok(())
    }
}

fn merge_rect(r1: Option<Rect>, r2: Rect) -> Option<Rect> {
    match r1 {
        Some(rect1) => {
            let min_x = rect1.origin.x.min(r2.origin.x);
            let min_y = rect1.origin.y.min(r2.origin.y);
            let max_x = (rect1.origin.x + rect1.size.width).max(r2.origin.x + r2.size.width);
            let max_y = (rect1.origin.y + rect1.size.height).max(r2.origin.y + r2.size.height);
            Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
        }
        None => Some(r2),
    }
}

pub fn create_layer(pos: Point, size: Size) -> Result<Layer> {
    let format = frame_buf::format()?;
    let layer = Layer::new(pos, size, format);
    Ok(layer)
}

pub fn create_layer_from_bitmap_image(pos: Point, bitmap_image: &BitmapImage) -> Result<Layer> {
    let bitmap_image_info_header = bitmap_image.info_header();
    let bitmap_image_data = bitmap_image.bitmap_to_color_code();
    let b_w = bitmap_image_info_header.width as usize;
    let b_h = bitmap_image_info_header.height as usize;
    let mut layer = Layer::new(pos, Size::new(b_w, b_h), PixelFormat::Bgr);

    for h in 0..b_h {
        for w in 0..b_w {
            let pixel_data = bitmap_image_data[h * b_w + w];
            layer.draw_pixel(Point::new(w, h), pixel_data)?;
        }
    }

    Ok(layer)
}

pub fn push_layer(layer: Layer) -> Result<()> {
    LAYER_MAN.try_lock()?.push_layer(layer);
    Ok(())
}

pub fn draw_to_frame_buf() -> Result<()> {
    LAYER_MAN.try_lock()?.draw_to_frame_buf()
}

pub fn draw_layer<F: FnMut(&mut dyn Draw) -> Result<()>>(
    layer_id: LayerId,
    mut draw: F,
) -> Result<()> {
    draw(LAYER_MAN.try_lock()?.layer(layer_id)?)
}

pub fn layer_info(layer_id: LayerId) -> Result<LayerInfo> {
    let mut layer_man = LAYER_MAN.try_lock()?;
    let layer = layer_man.layer(layer_id)?;
    let layer_info = layer.layer_info();
    Ok(layer_info)
}

pub fn move_layer(layer_id: LayerId, to_pos: Point) -> Result<()> {
    LAYER_MAN.try_lock()?.layer(layer_id)?.move_to(to_pos);
    Ok(())
}

pub fn remove_layer(layer_id: LayerId) -> Result<()> {
    LAYER_MAN.try_lock()?.remove_layer(layer_id)
}

pub fn bring_layer_to_front(layer_id: LayerId) -> Result<()> {
    LAYER_MAN.try_lock()?.bring_layer_to_front(layer_id)
}
