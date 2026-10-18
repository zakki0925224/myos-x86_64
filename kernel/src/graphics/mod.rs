use self::{color::ColorCode, font::FONT};
use crate::{error::Result, kinfo};
use common::{
    geometry::{Point, Size},
    graphic_info::GraphicInfo,
};

pub mod color;
pub mod draw;
pub mod fb;
pub mod fb_console;
pub mod font;
pub mod multi_layer;
pub mod window_manager;

pub fn init(
    graphic_info: &GraphicInfo,
    console_back_color: ColorCode,
    console_fore_color: ColorCode,
) -> Result<()> {
    fb::init(graphic_info)?;
    FONT.init_cache();
    fb_console::init(console_back_color, console_fore_color)?;

    kinfo!("graphics: Frame buffer initialized");
    Ok(())
}

pub fn enable_shadow_buf() -> Result<()> {
    fb::enable_shadow_buf()?;

    kinfo!("graphics: Shadow buffer enabled");
    Ok(())
}

pub fn init_layer_man(graphic_info: &GraphicInfo) -> Result<()> {
    let (res_x, res_y) = graphic_info.resolution.wh();
    let console_layer = multi_layer::create_layer(Point::default(), Size::new(res_x, res_y - 30))?;
    let console_layer_id = console_layer.id;

    multi_layer::push_layer(console_layer)?;
    fb_console::set_target_layer_id(console_layer_id)?;

    kinfo!("graphics: Layer manager initialized");
    Ok(())
}

pub fn init_window_man() -> Result<()> {
    window_manager::init()?;
    window_manager::create_taskbar()?;

    kinfo!("graphics: Window manager initialized");
    Ok(())
}
