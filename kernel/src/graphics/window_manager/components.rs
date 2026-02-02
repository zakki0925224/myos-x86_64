use crate::{
    arch::VirtualAddress,
    error::Result,
    fs::file::bitmap::BitmapImage,
    graphics::{
        color::ColorCode,
        draw::Draw,
        font::FONT,
        multi_layer::{self, *},
    },
    theme::GLOBAL_THEME,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use common::{
    geometry::{Point, Rect, Size},
    graphic_info::PixelFormat,
};

fn fill_back_color_and_draw_borders(l: &mut dyn Draw, size: Size) -> Result<()> {
    let (w, h) = size.wh();

    // back color
    l.fill(GLOBAL_THEME.wm.component_back)?;

    // borders
    let border_color1 = GLOBAL_THEME.wm.border_color1;
    let border_color2 = if GLOBAL_THEME.wm.border_flat {
        GLOBAL_THEME.wm.border_color1
    } else {
        GLOBAL_THEME.wm.border_color2
    };
    let border_width = if GLOBAL_THEME.wm.border_flat {
        w
    } else {
        w - 2
    };
    let border_height = if GLOBAL_THEME.wm.border_flat {
        h
    } else {
        h - 2
    };

    l.draw_rect(Rect::new(0, 0, 2, border_height), border_color1)?;
    l.draw_rect(Rect::new(2, h - 2, w - 2, 2), border_color2)?;

    l.draw_rect(Rect::new(w - 2, 2, 2, h - 2), border_color2)?;
    l.draw_rect(Rect::new(0, 0, border_width, 2), border_color1)?;

    Ok(())
}

pub trait Component {
    fn layer_id(&self) -> LayerId;
    fn get_layer_info(&self) -> Result<LayerInfo> {
        multi_layer::get_layer_info(self.layer_id())
    }
    fn move_by_root(&self, to_pos: Point) -> Result<()> {
        multi_layer::move_layer(self.layer_id(), to_pos)
    }
    fn move_by_parent(&self, parent: &dyn Component, to_pos: Point) -> Result<()> {
        let pos = self.get_layer_info()?.pos;
        let p_pos = parent.get_layer_info()?.pos;
        self.move_by_root(to_pos + (pos - p_pos))
    }
    fn draw_flush(&mut self) -> Result<()>;
}

pub struct Image {
    layer_id: LayerId,
    framebuf_virt_addr: Option<VirtualAddress>,
    pixel_format: Option<PixelFormat>,
    buf: Option<Vec<u32>>,
}

impl Drop for Image {
    fn drop(&mut self) {
        let _ = multi_layer::remove_layer(self.layer_id);
    }
}

impl Component for Image {
    fn layer_id(&self) -> LayerId {
        self.layer_id
    }

    fn draw_flush(&mut self) -> Result<()> {
        let framebuf_virt_addr = match self.framebuf_virt_addr {
            Some(addr) => addr,
            None => return Ok(()),
        };
        let pixel_format = match self.pixel_format {
            Some(fmt) => fmt,
            None => return Ok(()),
        };

        let LayerInfo {
            pos: _,
            size: Size {
                width: w,
                height: h,
            },
            format: layer_format,
        } = self.get_layer_info()?;
        let bytes = match pixel_format {
            PixelFormat::Rgb => 3,
            PixelFormat::Bgr => 3,
            PixelFormat::Bgra => 4,
        };

        // convert image to buffer
        let buf = self.buf.get_or_insert_with(|| Vec::with_capacity(w * h));
        if buf.len() != w * h {
            buf.resize(w * h, 0);
        }

        let framebuf_slice: &[u8] =
            unsafe { core::slice::from_raw_parts(framebuf_virt_addr.as_ptr(), w * h * bytes) };

        let buf_ptr = buf.as_mut_ptr();

        for y in 0..h {
            for x in 0..w {
                let offset = (y * w + x) * bytes;
                let pixel_color =
                    ColorCode::from_pixel_data(&framebuf_slice[offset..], pixel_format);
                unsafe {
                    buf_ptr
                        .add(y * w + x)
                        .write(pixel_color.to_color_code(layer_format));
                }
            }
        }

        // write to layer
        multi_layer::draw_layer(self.layer_id, |l| unsafe { l.copy_from_slice_u32(&buf) })?;

        Ok(())
    }
}

impl Image {
    pub fn create_and_push_from_bitmap_image(
        bitmap_image: &BitmapImage,
        pos: Point,
        always_on_top: bool,
    ) -> Result<Self> {
        if !bitmap_image.is_valid() {
            return Err("Invalid bitmap image".into());
        }

        let mut layer = multi_layer::create_layer_from_bitmap_image(pos, bitmap_image)?;
        layer.always_on_top = always_on_top;
        let layer_id = layer.id;
        multi_layer::push_layer(layer)?;
        Ok(Self {
            layer_id,
            framebuf_virt_addr: None,
            pixel_format: None,
            buf: None,
        })
    }

    pub fn create_and_push_from_framebuf(
        pos: Point,
        size: Size,
        framebuf_virt_addr: VirtualAddress,
        pixel_format: PixelFormat,
    ) -> Result<Self> {
        let framebuf_virt_addr = Some(framebuf_virt_addr);
        let pixel_format = Some(pixel_format);
        let layer = multi_layer::create_layer(pos, size)?;
        let layer_id = layer.id;
        multi_layer::push_layer(layer)?;
        Ok(Self {
            layer_id,
            framebuf_virt_addr,
            pixel_format,
            buf: None,
        })
    }
}

pub struct Window {
    layer_id: LayerId,
    title: String,
    close_button: Button,
    resize_button: Button,
    minimize_button: Button,
    children: Vec<Box<dyn Component>>,
    contents_base_rel_pos: Point,
    pub is_closed: bool,
    pub request_bring_to_front: bool,
}

impl Drop for Window {
    fn drop(&mut self) {
        let _ = multi_layer::remove_layer(self.layer_id);
    }
}

impl Component for Window {
    fn layer_id(&self) -> LayerId {
        self.layer_id
    }

    fn move_by_root(&self, to_pos: Point) -> Result<()> {
        self.close_button.move_by_parent(self, to_pos)?;
        self.resize_button.move_by_parent(self, to_pos)?;
        self.minimize_button.move_by_parent(self, to_pos)?;

        for child in &self.children {
            child.move_by_parent(self, to_pos)?;
        }

        multi_layer::move_layer(self.layer_id, to_pos)?;

        Ok(())
    }

    fn draw_flush(&mut self) -> Result<()> {
        let LayerInfo {
            pos: Point { x: w_x, y: w_y },
            size: Size {
                width: w_w,
                height: w_h,
            },
            format: _,
        } = self.get_layer_info()?;

        if self.request_bring_to_front {
            multi_layer::bring_layer_to_front(self.layer_id)?;
            multi_layer::bring_layer_to_front(self.close_button.layer_id())?;
            multi_layer::bring_layer_to_front(self.resize_button.layer_id())?;
            multi_layer::bring_layer_to_front(self.minimize_button.layer_id())?;
            for child in &self.children {
                multi_layer::bring_layer_to_front(child.layer_id())?;
            }

            self.request_bring_to_front = false;
        }

        multi_layer::draw_layer(self.layer_id, |l| {
            fill_back_color_and_draw_borders(l, Size::new(w_w, w_h))?;

            // titlebar
            l.draw_rect(Rect::new(4, 4, w_w - 8, 18), GLOBAL_THEME.wm.titlebar_back)?;

            // title
            l.draw_string_wrap(
                Point::new(7, 7),
                &format!("<{}> {}", self.layer_id, self.title),
                GLOBAL_THEME.wm.titlebar_fore,
                GLOBAL_THEME.wm.titlebar_back,
            )?;
            Ok(())
        })?;

        self.close_button.draw_flush()?;
        self.resize_button.draw_flush()?;
        self.minimize_button.draw_flush()?;

        let (contents_base_rel_x, mut contents_base_rel_y) = self.contents_base_rel_pos.xy();
        let mut max_width = 0;

        for child in &mut self.children {
            let Size {
                width: w,
                height: h,
            } = child.get_layer_info()?.size;
            child.move_by_root(Point::new(
                w_x + contents_base_rel_x,
                w_y + contents_base_rel_y,
            ))?;
            child.draw_flush()?;

            contents_base_rel_y += h + 4; // padding

            if max_width > w {
                max_width = w;
            }
        }

        Ok(())
    }
}

impl Window {
    pub fn create_and_push(title: String, pos: Point, size: Size) -> Result<Self> {
        let layer = multi_layer::create_layer(pos, size)?;
        let layer_id = layer.id.clone();
        multi_layer::push_layer(layer)?;

        let (w, _) = size.wh();

        let close_button = Button::create_and_push(
            "x".to_string(),
            pos + Point::new(w - 22, 6),
            Size::new(16, 14),
        )?;
        let resize_button = Button::create_and_push(
            "[]".to_string(),
            pos + Point::new(w - 40, 6),
            Size::new(16, 14),
        )?;
        let minimize_button = Button::create_and_push(
            "_".to_string(),
            pos + Point::new(w - 58, 6),
            Size::new(16, 14),
        )?;

        Ok(Self {
            layer_id,
            title,
            is_closed: false,
            close_button,
            resize_button,
            children: Vec::new(),
            minimize_button,
            contents_base_rel_pos: Point::new(4, 25),
            request_bring_to_front: false,
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn is_close_button_clickable(&self, point: Point) -> Result<bool> {
        let LayerInfo {
            pos: cb_pos,
            size: cb_size,
            format: _,
        } = self.close_button.get_layer_info()?;

        let rect = Rect::from_point_and_size(cb_pos, cb_size);
        Ok(rect.contains(point))
    }

    pub fn push_child(&mut self, child: Box<dyn Component>) -> Result<LayerId> {
        let child_layer_id = child.layer_id();
        self.children.push(child);
        Ok(child_layer_id)
    }

    pub fn remove_child(&mut self, layer_id: LayerId) -> Result<()> {
        if let Some(pos) = self.children.iter().position(|c| c.layer_id() == layer_id) {
            self.children.remove(pos);
            Ok(())
        } else {
            Err("Child component not found".into())
        }
    }
}

pub struct Panel {
    layer_id: LayerId,
}

impl Drop for Panel {
    fn drop(&mut self) {
        let _ = multi_layer::remove_layer(self.layer_id);
    }
}

impl Component for Panel {
    fn layer_id(&self) -> LayerId {
        self.layer_id
    }

    fn draw_flush(&mut self) -> Result<()> {
        let size = self.get_layer_info()?.size;

        multi_layer::draw_layer(self.layer_id, |l| fill_back_color_and_draw_borders(l, size))
    }
}

impl Panel {
    pub fn create_and_push(pos: Point, size: Size) -> Result<Self> {
        let layer = multi_layer::create_layer(pos, size)?;
        let layer_id = layer.id;
        multi_layer::push_layer(layer)?;
        Ok(Self { layer_id })
    }

    pub fn draw_string(&self, point: Point, s: &str) -> Result<()> {
        multi_layer::draw_layer(self.layer_id, |l| {
            l.draw_string_wrap(
                point,
                s,
                GLOBAL_THEME.wm.component_fore,
                GLOBAL_THEME.wm.component_back,
            )
        })
    }
}

pub struct Button {
    layer_id: LayerId,
    title: String,
}

impl Drop for Button {
    fn drop(&mut self) {
        let _ = multi_layer::remove_layer(self.layer_id);
    }
}

impl Component for Button {
    fn layer_id(&self) -> LayerId {
        self.layer_id
    }

    fn draw_flush(&mut self) -> Result<()> {
        let size = self.get_layer_info()?.size;

        multi_layer::draw_layer(self.layer_id, |l| {
            fill_back_color_and_draw_borders(l, size)?;

            // title
            let (f_w, f_h) = FONT.get_wh();
            l.draw_string_wrap(
                Point::new(
                    size.width / 2 - f_w * self.title.len() / 2,
                    size.height / 2 - f_h / 2,
                ),
                &self.title,
                GLOBAL_THEME.wm.component_fore,
                GLOBAL_THEME.wm.component_back,
            )?;

            Ok(())
        })
    }
}

impl Button {
    pub fn create_and_push(title: String, pos: Point, size: Size) -> Result<Self> {
        let layer = multi_layer::create_layer(pos, size)?;
        let layer_id = layer.id;
        multi_layer::push_layer(layer)?;
        Ok(Self { layer_id, title })
    }
}

pub struct Label {
    layer_id: LayerId,
    label: String,
    back_color: ColorCode,
    fore_color: ColorCode,
}

impl Drop for Label {
    fn drop(&mut self) {
        let _ = multi_layer::remove_layer(self.layer_id);
    }
}

impl Component for Label {
    fn layer_id(&self) -> LayerId {
        self.layer_id
    }

    fn draw_flush(&mut self) -> Result<()> {
        multi_layer::draw_layer(self.layer_id, |l| {
            // back color
            l.fill(self.back_color)?;

            // label
            let (_, font_h) = FONT.get_wh();
            let c_x = 0;
            let mut c_y = 0;

            for line in self.label.lines() {
                l.draw_string_wrap(Point::new(c_x, c_y), line, self.fore_color, self.back_color)?;
                c_y += font_h;
            }

            Ok(())
        })
    }
}

impl Label {
    pub fn create_and_push(
        pos: Point,
        label: String,
        back_color: ColorCode,
        fore_color: ColorCode,
    ) -> Result<Self> {
        // calc width and height
        let (f_w, f_h) = FONT.get_wh();
        let w = label.lines().map(|s| s.len()).max().unwrap_or(0) * f_w;
        let h = label.lines().count() * f_h;

        let layer = multi_layer::create_layer(pos, Size::new(w, h))?;
        let layer_id = layer.id;
        multi_layer::push_layer(layer)?;
        Ok(Self {
            layer_id,
            label,
            back_color,
            fore_color,
        })
    }
}

pub struct Canvas {
    layer_id: LayerId,
}

impl Drop for Canvas {
    fn drop(&mut self) {
        let _ = multi_layer::remove_layer(self.layer_id);
    }
}

impl Component for Canvas {
    fn layer_id(&self) -> LayerId {
        self.layer_id
    }

    fn draw_flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Canvas {
    pub fn create_and_push(pos: Point, size: Size) -> Result<Self> {
        let layer = multi_layer::create_layer(pos, size)?;
        let layer_id = layer.id;
        multi_layer::push_layer(layer)?;
        Ok(Self { layer_id })
    }
}
