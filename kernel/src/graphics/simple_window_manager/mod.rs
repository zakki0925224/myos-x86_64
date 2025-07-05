use super::{
    frame_buf,
    multi_layer::{LayerId, LayerInfo},
};
use crate::{
    device::ps2_mouse::MouseEvent,
    error::{Error, Result},
    fs::file::bitmap::BitmapImage,
    util::{self, mutex::Mutex},
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
    vec::Vec,
};
use components::*;

pub mod components;

static mut SIMPLE_WM: Mutex<SimpleWindowManager> = Mutex::new(SimpleWindowManager::new());

#[derive(Debug, Clone, PartialEq)]
pub enum SimpleWindowManagerError {
    MousePointerLayerWasNotFound,
    TaskbarLayerWasNotFound,
    WindowWasNotFound { layer_id: usize },
}

struct SimpleWindowManager {
    windows: Vec<Window>,
    taskbar: Option<Panel>,
    mouse_pointer: Option<Image>,
    res_xy: Option<(usize, usize)>,
}

impl SimpleWindowManager {
    const MAX_REL_MOVEMENT: isize = 100;

    const fn new() -> Self {
        Self {
            windows: Vec::new(),
            taskbar: None,
            mouse_pointer: None,
            res_xy: None,
        }
    }

    fn create_mouse_pointer(&mut self, pointer_bmp: &BitmapImage) -> Result<()> {
        self.mouse_pointer = Some(Image::create_and_push_from_bitmap_image(
            pointer_bmp,
            (0, 0),
            true,
        )?);

        Ok(())
    }

    fn create_taskbar(&mut self) -> Result<()> {
        let (res_x, res_y) = self.res_xy.ok_or(Error::NotInitialized)?;

        let w = res_x;
        let h = 30;
        let panel = Panel::create_and_push((0, res_y - h), (w, h))?;
        self.taskbar = Some(panel);
        Ok(())
    }

    fn mouse_pointer_event(&mut self, mouse_event: MouseEvent) -> Result<()> {
        let (res_x, res_y) = self.res_xy.ok_or(Error::NotInitialized)?;

        let mouse_pointer = self
            .mouse_pointer
            .as_mut()
            .ok_or(SimpleWindowManagerError::MousePointerLayerWasNotFound)?;

        let LayerInfo {
            xy: (m_x_before, m_y_before),
            wh: (m_w, m_h),
            format: _,
        } = mouse_pointer.get_layer_info()?;

        let rel_x =
            (mouse_event.rel_x as isize).clamp(-Self::MAX_REL_MOVEMENT, Self::MAX_REL_MOVEMENT);
        let rel_y =
            (mouse_event.rel_y as isize).clamp(-Self::MAX_REL_MOVEMENT, Self::MAX_REL_MOVEMENT);

        let m_x_after =
            (m_x_before as isize + rel_x).clamp(0, res_x as isize - m_w as isize) as usize;
        let m_y_after =
            (m_y_before as isize + rel_y).clamp(0, res_y as isize - m_h as isize) as usize;

        // move mouse pointer
        mouse_pointer.move_by_root(m_x_after, m_y_after)?;

        if mouse_event.left {
            for w in self.windows.iter_mut().rev() {
                let LayerInfo {
                    xy: (w_x, w_y),
                    wh: (w_w, w_h),
                    format: _,
                } = w.get_layer_info()?;

                // click close button event
                if w.is_close_button_clickable(m_x_before, m_y_before)? {
                    w.is_closed = true;
                    self.windows.retain(|w| !w.is_closed);
                    break;
                }

                // drag window event
                if m_x_before >= w_x
                    && m_x_before < w_x + w_w
                    && m_y_before >= w_y
                    && m_y_before < w_y + w_h
                // pointer is in window
                && m_x_before != m_x_after
                    || m_y_before != m_y_after
                // pointer moved
                {
                    let new_w_x =
                        (w_x as isize + m_x_after as isize - m_x_before as isize).max(0) as usize;
                    let new_w_y =
                        (w_y as isize + m_y_after as isize - m_y_before as isize).max(0) as usize;

                    w.move_by_root(new_w_x, new_w_y)?;
                    break;
                }
            }
        }

        Ok(())
    }

    fn create_window(
        &mut self,
        title: String,
        xy: (usize, usize),
        wh: (usize, usize),
    ) -> Result<LayerId> {
        if self.res_xy.is_none() {
            return Err(Error::NotInitialized);
        }

        let window = Window::create_and_push(title, xy, wh)?;

        // let button1 = Button::create_and_push("button 1".to_string(), (0, 0), (100, 25))?;
        // let button2 = Button::create_and_push("button 2".to_string(), (0, 0), (100, 25))?;
        // let button3 = Button::create_and_push("button 3".to_string(), (0, 0), (100, 25))?;
        // let button4 = Button::create_and_push("button 4".to_string(), (0, 0), (100, 25))?;
        // let button5 = Button::create_and_push("button 5".to_string(), (0, 0), (100, 25))?;
        // let button6 = Button::create_and_push("button 6".to_string(), (0, 0), (100, 25))?;
        // let button7 = Button::create_and_push("button 7".to_string(), (0, 0), (100, 25))?;
        // let label = Label::create_and_push((0, 0),
        //     "[32] Sed ut perspiciatis, unde omnis iste natus error sit voluptatem\naccusantium doloremque laudantium, totam rem aperiam eaque ipsa, quae\nab illo inventore veritatis et quasi architecto beatae vitae dicta sunt,\nexplicabo.\nNemo enim ipsam voluptatem, quia voluptas sit, aspernatur aut\nodit aut fugit, sed quia consequuntur magni dolores eos, qui ratione\nvoluptatem sequi nesciunt, neque porro quisquam est, qui dolorem ipsum,\nquia dolor sit, amet, consectetur, adipisci velit, sed quia non numquam\neius modi tempora incidunt, ut labore et dolore magnam aliquam quaerat\nvoluptatem.".to_string(),
        //     GLOBAL_THEME.fore_color,
        //     GLOBAL_THEME.back_color,
        // )?;

        // window.push_child(Box::new(button1))?;
        // window.push_child(Box::new(button2))?;
        // window.push_child(Box::new(button3))?;
        // window.push_child(Box::new(button4))?;
        // window.push_child(Box::new(button5))?;
        // window.push_child(Box::new(button6))?;
        // window.push_child(Box::new(button7))?;
        // window.push_child(Box::new(label))?;

        let layer_id = window.layer_id();
        self.windows.push(window);

        Ok(layer_id)
    }

    fn add_component_to_window(
        &mut self,
        layer_id: &LayerId,
        component: Box<dyn Component>,
    ) -> Result<LayerId> {
        if self.res_xy.is_none() {
            return Err(Error::NotInitialized);
        }

        let window = self
            .windows
            .iter_mut()
            .find(|w| w.layer_id().get() == layer_id.get())
            .ok_or(SimpleWindowManagerError::WindowWasNotFound {
                layer_id: layer_id.get(),
            })?;
        window.push_child(component)
    }

    fn remove_component(&mut self, layer_id: &LayerId) -> Result<()> {
        if self.res_xy.is_none() {
            return Err(Error::NotInitialized);
        }

        // try remove window
        if let Some(index) = self
            .windows
            .iter()
            .position(|w| w.layer_id().get() == layer_id.get())
        {
            self.windows.remove(index);
            return Ok(());
        }

        // try remove component from window
        for window in self.windows.iter_mut() {
            if window.remove_child(layer_id).is_ok() {
                return Ok(());
            }
        }

        Err(SimpleWindowManagerError::WindowWasNotFound {
            layer_id: layer_id.get(),
        }
        .into())
    }

    fn flush_taskbar(&mut self) -> Result<()> {
        if self.res_xy.is_none() {
            return Err(Error::NotInitialized);
        }

        let taskbar = self
            .taskbar
            .as_mut()
            .ok_or(SimpleWindowManagerError::TaskbarLayerWasNotFound)?;
        let (w, h) = taskbar.get_layer_info()?.wh;
        taskbar.draw_flush()?;

        let window_titles: Vec<&str> = self.windows.iter().map(|w| w.title()).collect();
        let s = format!("{:?}", window_titles);
        taskbar.draw_string((7, h / 2 - 8), &s)?;

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
        taskbar.draw_string((w - s.len() * 8, h / 2 - 8), &s)?;

        Ok(())
    }

    fn flush_components(&mut self) -> Result<()> {
        if self.res_xy.is_none() {
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

pub fn init() -> Result<()> {
    let mut simple_wm = unsafe { SIMPLE_WM.try_lock() }?;
    let res_xy = frame_buf::resolution()?;
    simple_wm.res_xy = Some(res_xy);
    Ok(())
}

pub fn create_mouse_pointer(pointer_bmp: &BitmapImage) -> Result<()> {
    unsafe { SIMPLE_WM.try_lock() }?.create_mouse_pointer(pointer_bmp)
}

pub fn create_taskbar() -> Result<()> {
    unsafe { SIMPLE_WM.try_lock() }?.create_taskbar()
}

pub fn mouse_pointer_event(mouse_event: MouseEvent) -> Result<()> {
    unsafe { SIMPLE_WM.try_lock() }?.mouse_pointer_event(mouse_event)
}

pub fn create_window(title: String, xy: (usize, usize), wh: (usize, usize)) -> Result<LayerId> {
    unsafe { SIMPLE_WM.try_lock() }?.create_window(title, xy, wh)
}

pub fn add_component_to_window(
    layer_id: &LayerId,
    component: Box<dyn Component>,
) -> Result<LayerId> {
    unsafe { SIMPLE_WM.try_lock() }?.add_component_to_window(layer_id, component)
}

pub fn remove_component(layer_id: &LayerId) -> Result<()> {
    unsafe { SIMPLE_WM.try_lock() }?.remove_component(layer_id)
}

pub fn flush_components() -> Result<()> {
    unsafe { SIMPLE_WM.try_lock() }?.flush_components()
}
