use super::{frame_buf, multi_layer::LayerId};
use crate::{
    error::{Error, Result},
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

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum WindowManagerError {
    MousePointerLayerWasNotFound,
    TaskbarLayerWasNotFound,
    WindowWasNotFound { layer_id: usize },
}

impl core::fmt::Display for WindowManagerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MousePointerLayerWasNotFound => write!(f, "Mouse pointer layer was not found"),
            Self::TaskbarLayerWasNotFound => write!(f, "Taskbar layer was not found"),
            Self::WindowWasNotFound { layer_id } => {
                write!(f, "Window was not found: layer_id={}", layer_id)
            }
        }
    }
}

struct WindowManager {
    windows: Vec<Window>,
    taskbar: Option<Panel>,
    res: Option<Size>,
    last_taskbar_uptime: String,
    last_taskbar_titles: String,
}

impl WindowManager {
    const fn new() -> Self {
        Self {
            windows: Vec::new(),
            taskbar: None,
            res: None,
            last_taskbar_uptime: String::new(),
            last_taskbar_titles: String::new(),
        }
    }

    fn create_taskbar(&mut self) -> Result<()> {
        let res = self.res.ok_or(Error::NotInitialized)?;

        let h = 30;
        let panel = Panel::create_and_push(Point::new(0, res.height - h), Size::new(res.width, h))?;
        self.taskbar = Some(panel);
        Ok(())
    }

    fn create_window(&mut self, title: String, pos: Point, size: Size) -> Result<LayerId> {
        if self.res.is_none() {
            return Err(Error::NotInitialized.into());
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
            return Err(Error::NotInitialized.into());
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
            return Err(Error::NotInitialized.into());
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
            return Err(Error::NotInitialized.into());
        }

        let taskbar = self
            .taskbar
            .as_mut()
            .ok_or(WindowManagerError::TaskbarLayerWasNotFound)?;
        let size = taskbar.layer_info()?.size;

        taskbar.draw_flush()?;

        let (f_w, f_h) = crate::graphics::font::FONT.wh();
        let text_y = size.height / 2 - f_h / 2;

        // window titles
        let window_titles: Vec<&str> = self.windows.iter().map(|w| w.title()).collect();
        let new_titles = format!("{:?}", window_titles);
        if new_titles != self.last_taskbar_titles {
            let old_w = self.last_taskbar_titles.len() * f_w;
            if old_w > 0 {
                taskbar.clear_rect(Rect::new(7, text_y, old_w, f_h))?;
            }
            taskbar.draw_string(Point::new(7, text_y), &new_titles)?;
            self.last_taskbar_titles = new_titles;
        }

        // uptime
        let uptime = util::time::global_uptime();
        let new_uptime = if uptime.is_zero() {
            "??????.???".to_string()
        } else {
            format!(
                "{:06}.{:03}",
                uptime.as_millis() / 1000,
                uptime.as_millis() % 1000
            )
        };
        if new_uptime != self.last_taskbar_uptime {
            let uptime_w = new_uptime.len() * f_w;
            let uptime_x = size.width.saturating_sub(uptime_w + 8);
            taskbar.clear_rect(Rect::new(uptime_x, text_y, uptime_w, f_h))?;
            taskbar.draw_string(Point::new(uptime_x, text_y), &new_uptime)?;
            self.last_taskbar_uptime = new_uptime;
        }

        Ok(())
    }

    fn flush_components(&mut self) -> Result<()> {
        if self.res.is_none() {
            return Err(Error::NotInitialized.into());
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

pub fn init() -> Result<()> {
    let mut window_man = WINDOW_MAN.try_lock()?;
    let res = frame_buf::resolution()?;
    window_man.res = Some(res);
    Ok(())
}

pub fn create_taskbar() -> Result<()> {
    WINDOW_MAN.try_lock()?.create_taskbar()
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
