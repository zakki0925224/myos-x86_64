use super::{draw::Draw, frame_buf};
use crate::{
    error::{Error, Result},
    fs::file::bitmap::BitmapImage,
    util::mutex::Mutex,
};
use alloc::vec::Vec;
use common::graphic_info::PixelFormat;
use core::sync::atomic::{AtomicUsize, Ordering};

static mut LAYER_MAN: Mutex<LayerManager> = Mutex::new(LayerManager::new());

#[derive(Debug, Clone, PartialEq)]
pub enum LayerError {
    OutsideBufferAreaError { layer_id: usize, x: usize, y: usize },
    InvalidLayerIdError(usize),
}

#[derive(Debug, Clone)]
pub struct LayerPositionInfo {
    pub xy: (usize, usize),
    pub wh: (usize, usize),
}

#[derive(Debug, Clone)]
pub struct LayerId(usize);
impl LayerId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub const fn new_val(value: i32) -> Result<Self> {
        if value < 0 {
            return Err(Error::Failed("Invalid layer id"));
        }

        Ok(Self(value as usize))
    }

    pub fn get(&self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub struct Layer {
    pub id: LayerId,
    xy: (usize, usize),
    wh: (usize, usize),
    buf: Vec<u32>,
    pub disabled: bool,
    format: PixelFormat,
    pub always_on_top: bool,
}

impl Draw for Layer {
    fn resolution(&self) -> Result<(usize, usize)> {
        Ok(self.wh)
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
}

impl Layer {
    pub fn new(xy: (usize, usize), wh: (usize, usize), format: PixelFormat) -> Self {
        Self {
            id: LayerId::new(),
            xy,
            wh,
            buf: vec![0; wh.0 * wh.1],
            disabled: false,
            format,
            always_on_top: false,
        }
    }

    pub fn move_to(&mut self, x: usize, y: usize) {
        self.xy = (x, y);
    }

    pub fn layer_pos_info(&self) -> LayerPositionInfo {
        LayerPositionInfo {
            xy: self.xy,
            wh: self.wh,
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
        self.layers.push(layer);
    }

    fn remove_layer(&mut self, layer_id: &LayerId) -> Result<()> {
        if self.get_layer(layer_id).is_err() {
            return Err(LayerError::InvalidLayerIdError(layer_id.get()).into());
        }

        self.layers.retain(|l| l.id.get() != layer_id.get());

        Ok(())
    }

    fn get_layer(&mut self, layer_id: &LayerId) -> Result<&mut Layer> {
        self.layers
            .iter_mut()
            .find(|l| l.id.get() == layer_id.get())
            .ok_or(LayerError::InvalidLayerIdError(layer_id.get()).into())
    }

    fn draw_to_frame_buf(&mut self) -> Result<()> {
        self.layers
            .sort_by(|a, b| a.always_on_top.cmp(&b.always_on_top));

        for layer in &mut self.layers {
            if layer.disabled {
                continue;
            }

            frame_buf::apply_layer_buf(layer)?;
        }

        Ok(())
    }
}

pub fn create_layer(xy: (usize, usize), wh: (usize, usize)) -> Result<Layer> {
    let format = frame_buf::format()?;
    let layer = Layer::new(xy, wh, format);
    Ok(layer)
}

pub fn create_layer_from_bitmap_image(
    xy: (usize, usize),
    bitmap_image: &BitmapImage,
) -> Result<Layer> {
    let bitmap_image_info_header = bitmap_image.info_header();
    let bitmap_image_data = bitmap_image.bitmap_to_color_code();
    let b_w = bitmap_image_info_header.width as usize;
    let b_h = bitmap_image_info_header.height as usize;
    let mut layer = Layer::new(xy, (b_w, b_h), PixelFormat::Bgr);

    for h in 0..b_h {
        for w in 0..b_w {
            let pixel_data = bitmap_image_data[h * b_w + w];
            layer.draw_pixel((w, h), pixel_data)?;
        }
    }

    Ok(layer)
}

pub fn push_layer(layer: Layer) -> Result<()> {
    unsafe { LAYER_MAN.try_lock() }?.push_layer(layer);
    Ok(())
}

pub fn draw_to_frame_buf() -> Result<()> {
    unsafe { LAYER_MAN.try_lock() }?.draw_to_frame_buf()
}

pub fn draw_layer<F: Fn(&mut dyn Draw) -> Result<()>>(layer_id: &LayerId, draw: F) -> Result<()> {
    draw(unsafe { LAYER_MAN.try_lock() }?.get_layer(layer_id)?)
}

pub fn get_layer_pos_info(layer_id: &LayerId) -> Result<LayerPositionInfo> {
    let mut layer_man = unsafe { LAYER_MAN.try_lock() }?;
    let layer = layer_man.get_layer(layer_id)?;
    let layer_pos_info = layer.layer_pos_info();
    Ok(layer_pos_info)
}

pub fn move_layer(layer_id: &LayerId, to_x: usize, to_y: usize) -> Result<()> {
    unsafe { LAYER_MAN.try_lock() }?
        .get_layer(layer_id)?
        .move_to(to_x, to_y);
    Ok(())
}

pub fn remove_layer(layer_id: &LayerId) -> Result<()> {
    unsafe { LAYER_MAN.try_lock() }?.remove_layer(layer_id)
}
