use super::{
    frame_buf,
    multi_layer::{LayerId, LayerInfo},
};
use crate::{
    device::{ps2_mouse::Ps2MouseEvent, usb::hid_tablet::UsbHidMouseEvent},
    error::{Error, Result},
    fs::{file::bitmap::BitmapImage, vfs},
    sync::mutex::Mutex,
    util,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use common::geometry::{Point, Rect, Size};
use components::*;

pub mod components;

static WINDOW_MAN: Mutex<WindowManager> = Mutex::new(WindowManager::new());

pub enum MouseEvent {
    Ps2Mouse(Ps2MouseEvent),
    UsbHidMouse(UsbHidMouseEvent),
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowManagerError {
    MousePointerLayerWasNotFound,
    TaskbarLayerWasNotFound,
    WindowWasNotFound { layer_id: usize },
}

struct WindowManager {
    windows: Vec<Window>,
    taskbar: Option<Panel>,
    mouse_pointer: Option<Image>,
    res: Option<Size>,
    mouse_pointer_bmp_path: String,
    dragging_window_id: Option<LayerId>,
    dragging_offset: Option<Point>,
}

impl WindowManager {
    const PS2_MOUSE_MAX_REL_MOVEMENT: isize = 100;

    const fn new() -> Self {
        Self {
            windows: Vec::new(),
            taskbar: None,
            mouse_pointer: None,
            res: None,
            mouse_pointer_bmp_path: String::new(),
            dragging_window_id: None,
            dragging_offset: None,
        }
    }

    fn create_mouse_pointer(&mut self, pointer_bmp: &BitmapImage) -> Result<()> {
        self.mouse_pointer = Some(Image::create_and_push_from_bitmap_image(
            pointer_bmp,
            Point::default(),
            true,
        )?);

        Ok(())
    }

    fn create_taskbar(&mut self) -> Result<()> {
        let res = self.res.ok_or(Error::NotInitialized)?;

        let h = 30;
        let panel = Panel::create_and_push(Point::new(0, res.height - h), Size::new(res.width, h))?;
        self.taskbar = Some(panel);
        Ok(())
    }

    fn mouse_pointer_event(&mut self, mouse_event: MouseEvent) -> Result<()> {
        let res = self.res.ok_or(Error::NotInitialized)?;

        // create mouse pointer layer if not created
        if self.mouse_pointer.is_none() {
            let mouse_pointer_bmp_fd =
                vfs::open_file(&((&self.mouse_pointer_bmp_path).into()), false)?;
            let bmp_data = vfs::read_file(mouse_pointer_bmp_fd)?;
            let pointer_bmp = BitmapImage::new(&bmp_data);
            vfs::close_file(mouse_pointer_bmp_fd)?;
            self.create_mouse_pointer(&pointer_bmp)?;
        }

        let mouse_pointer = self
            .mouse_pointer
            .as_mut()
            .ok_or(WindowManagerError::MousePointerLayerWasNotFound)?;

        let LayerInfo {
            pos: Point {
                x: m_x_before,
                y: m_y_before,
            },
            size: Size {
                width: m_w,
                height: m_h,
            },
            format: _,
        } = mouse_pointer.get_layer_info()?;

        let m_pos_after = match &mouse_event {
            MouseEvent::Ps2Mouse(e) => {
                let rel_x = (e.rel_x as isize).clamp(
                    -Self::PS2_MOUSE_MAX_REL_MOVEMENT,
                    Self::PS2_MOUSE_MAX_REL_MOVEMENT,
                );
                let rel_y = (e.rel_y as isize).clamp(
                    -Self::PS2_MOUSE_MAX_REL_MOVEMENT,
                    Self::PS2_MOUSE_MAX_REL_MOVEMENT,
                );
                let m_x_after = (m_x_before as isize + rel_x)
                    .clamp(0, res.width as isize - m_w as isize)
                    as usize;
                let m_y_after = (m_y_before as isize + rel_y)
                    .clamp(0, res.height as isize - m_h as isize)
                    as usize;
                Point::new(m_x_after, m_y_after)
            }
            MouseEvent::UsbHidMouse(e) => {
                let m_x_after = e.abs_x.clamp(0, res.width.saturating_sub(m_w));
                let m_y_after = e.abs_y.clamp(0, res.height.saturating_sub(m_h));
                Point::new(m_x_after, m_y_after)
            }
        };

        // move mouse pointer
        mouse_pointer.move_by_root(m_pos_after)?;

        let e_left = match &mouse_event {
            MouseEvent::Ps2Mouse(e) => e.left,
            MouseEvent::UsbHidMouse(e) => e.left,
        };

        // click window event
        if e_left {
            if self.dragging_window_id.is_none() {
                // when clicked the window, bring it to the front
                for i in (0..self.windows.len()).rev() {
                    let w = &mut self.windows[i];
                    let LayerInfo {
                        pos: w_pos,
                        size: w_size,
                        format: _,
                    } = w.get_layer_info()?;

                    // mouse pointer is inside the window
                    let w_rect = Rect::from_point_and_size(w_pos, w_size);
                    if w_rect.contains(m_pos_after) {
                        let mut w = self.windows.remove(i);
                        w.request_bring_to_front = true;
                        let offset_x = m_pos_after.x - w_pos.x;
                        let offset_y = m_pos_after.y - w_pos.y;
                        let id = w.layer_id();
                        self.windows.push(w);
                        self.dragging_window_id = Some(id);
                        self.dragging_offset = Some(Point::new(offset_x, offset_y));
                        break;
                    }
                }

                // when clicked the close button of a window, remove the window
                for w in self.windows.iter_mut().rev() {
                    if w.is_close_button_clickable(m_pos_after)? {
                        w.is_closed = true;
                        self.windows.retain(|w| !w.is_closed);
                        self.dragging_window_id = None;
                        self.dragging_offset = None;
                        break;
                    }
                }
            }

            // drag the window
            if let (Some(window_id), Some(offset)) =
                (&self.dragging_window_id, &self.dragging_offset)
            {
                let w = self
                    .windows
                    .iter_mut()
                    .find(|w| w.layer_id() == *window_id)
                    .ok_or(WindowManagerError::WindowWasNotFound {
                        layer_id: window_id.get(),
                    })?;

                let LayerInfo {
                    pos: _,
                    size:
                        Size {
                            width: w_w,
                            height: w_h,
                        },
                    format: _,
                } = w.get_layer_info()?;

                let max_w_x = res.width.saturating_sub(w_w);
                let max_w_y = res.height.saturating_sub(w_h);
                let new_w_x = (m_pos_after.x as isize - offset.x as isize)
                    .clamp(0, max_w_x as isize) as usize;
                let new_w_y = (m_pos_after.y as isize - offset.y as isize)
                    .clamp(0, max_w_y as isize) as usize;
                w.move_by_root(Point::new(new_w_x, new_w_y))?;
            } else {
                for w in self.windows.iter_mut().rev() {
                    let LayerInfo {
                        pos: w_pos,
                        size: w_size,
                        format: _,
                    } = w.get_layer_info()?;

                    let w_rect = Rect::from_point_and_size(w_pos, w_size);
                    if w_rect.contains(m_pos_after) {
                        let delta_x = m_pos_after.x as isize - m_x_before as isize;
                        let delta_y = m_pos_after.y as isize - m_y_before as isize;
                        let max_w_x = res.width.saturating_sub(w_size.width);
                        let max_w_y = res.height.saturating_sub(w_size.height);
                        let new_w_x =
                            (w_pos.x as isize + delta_x).clamp(0, max_w_x as isize) as usize;
                        let new_w_y =
                            (w_pos.y as isize + delta_y).clamp(0, max_w_y as isize) as usize;

                        w.move_by_root(Point::new(new_w_x, new_w_y))?;
                        self.dragging_window_id = Some(w.layer_id());
                        break;
                    }
                }
            }
        } else {
            self.dragging_window_id = None;
            self.dragging_offset = None;
        }

        Ok(())
    }

    fn create_window(&mut self, title: String, pos: Point, size: Size) -> Result<LayerId> {
        if self.res.is_none() {
            return Err(Error::NotInitialized);
        }

        let window = Window::create_and_push(title, pos, size)?;
        let layer_id = window.layer_id();
        self.windows.push(window);

        Ok(layer_id)
    }

    fn add_component_to_window(
        &mut self,
        layer_id: LayerId,
        component: Box<dyn Component>,
    ) -> Result<LayerId> {
        if self.res.is_none() {
            return Err(Error::NotInitialized);
        }

        let window = self
            .windows
            .iter_mut()
            .find(|w| w.layer_id() == layer_id)
            .ok_or(WindowManagerError::WindowWasNotFound {
                layer_id: layer_id.get(),
            })?;
        window.push_child(component)
    }

    fn remove_component(&mut self, layer_id: LayerId) -> Result<()> {
        if self.res.is_none() {
            return Err(Error::NotInitialized);
        }

        // try remove window
        if let Some(index) = self.windows.iter().position(|w| w.layer_id() == layer_id) {
            self.windows.remove(index);
            return Ok(());
        }

        // try remove component from window
        for window in self.windows.iter_mut() {
            if window.remove_child(layer_id).is_ok() {
                return Ok(());
            }
        }

        Err(WindowManagerError::WindowWasNotFound {
            layer_id: layer_id.get(),
        }
        .into())
    }

    fn flush_taskbar(&mut self) -> Result<()> {
        if self.res.is_none() {
            return Err(Error::NotInitialized);
        }

        let taskbar = self
            .taskbar
            .as_mut()
            .ok_or(WindowManagerError::TaskbarLayerWasNotFound)?;
        let size = taskbar.get_layer_info()?.size;
        taskbar.draw_flush()?;

        let window_titles: Vec<&str> = self.windows.iter().map(|w| w.title()).collect();
        let s = format!("{:?}", window_titles);
        taskbar.draw_string(Point::new(7, size.height / 2 - 8), &s)?;

        let uptime = util::time::global_uptime();
        let s = if uptime.is_zero() {
            "??????.???".to_string()
        } else {
            format!(
                "{:06}.{:03}",
                uptime.as_millis() / 1000,
                uptime.as_millis() % 1000
            )
        };
        taskbar.draw_string(
            Point::new(size.width - s.len() * 8, size.height / 2 - 8),
            &s,
        )?;

        Ok(())
    }

    fn flush_components(&mut self) -> Result<()> {
        if self.res.is_none() {
            return Err(Error::NotInitialized);
        }

        for window in self.windows.iter_mut() {
            window.draw_flush()?;
        }

        if self.taskbar.is_some() {
            self.flush_taskbar()?;
        }

        Ok(())
    }
}

pub fn init(mouse_pointer_bmp_path: String) -> Result<()> {
    let mut window_man = WINDOW_MAN.try_lock()?;
    let res = frame_buf::resolution()?;
    window_man.res = Some(res);
    window_man.mouse_pointer_bmp_path = mouse_pointer_bmp_path;
    Ok(())
}

pub fn create_taskbar() -> Result<()> {
    WINDOW_MAN.try_lock()?.create_taskbar()
}

pub fn mouse_pointer_event(mouse_event: MouseEvent) -> Result<()> {
    WINDOW_MAN.try_lock()?.mouse_pointer_event(mouse_event)
}

pub fn create_window(title: String, pos: Point, size: Size) -> Result<LayerId> {
    WINDOW_MAN.try_lock()?.create_window(title, pos, size)
}

pub fn add_component_to_window(
    layer_id: LayerId,
    component: Box<dyn Component>,
) -> Result<LayerId> {
    WINDOW_MAN
        .try_lock()?
        .add_component_to_window(layer_id, component)
}

pub fn remove_component(layer_id: LayerId) -> Result<()> {
    WINDOW_MAN.try_lock()?.remove_component(layer_id)
}

pub fn flush_components() -> Result<()> {
    WINDOW_MAN.try_lock()?.flush_components()
}
