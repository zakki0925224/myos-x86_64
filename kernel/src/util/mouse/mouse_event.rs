#[derive(Default, Debug)]
pub struct Ps2MouseEvent {
    pub middle: bool,
    pub right: bool,
    pub left: bool,
    pub rel_x: i16,
    pub rel_y: i16,
}

#[derive(Default, Debug)]
pub struct UsbHidMouseEvent {
    pub middle: bool,
    pub right: bool,
    pub left: bool,
    pub abs_x: usize,
    pub abs_y: usize,
}

pub enum MouseEvent {
    Ps2MouseDevice(Ps2MouseEvent),
    UsbHidMouse(UsbHidMouseEvent),
}
