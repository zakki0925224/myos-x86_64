use crate::{
    error::{Error, Result},
    graphics::{
        color::ColorCode,
        multi_layer::{self, LayerError, LayerId},
        window_manager::{
            self,
            components::{self, Component},
        },
    },
    kdebug,
    sync::mutex::Mutex,
};
use alloc::{boxed::Box, collections::vec_deque::VecDeque, string::ToString};
use common::geometry::{Point, Rect, Size};

static NET_VIS_MAN: NetworkVisualizeManager = NetworkVisualizeManager::new();

const WINDOW_DEFAULT_POS: Point = Point::new(0, 0);
const WINDOW_SIZE: Size = Size::new(300, 500);
const CANVAS_SIZE: Size = Size::new(WINDOW_SIZE.width - 8, WINDOW_SIZE.height - 40);

#[derive(Debug, Clone, Copy)]
pub enum FunctionHook {
    ReceiveEthPayload,
    ReceiveArpPacket,
    ReceiveIpv4Paket,
    ReceiveIcmpPacket,
    ReceiveTcpPacket,
    ReceiveUdpPacket,
    SendEthPayload,
    SendArpPacket,
    SendTcpPacket,
    SendUdpPacket,
}

struct NetworkVisualizeManager {
    fn_hooks: Mutex<VecDeque<FunctionHook>>,
    window_layer_id: Mutex<Option<LayerId>>,
    canvas_layer_id: Mutex<Option<LayerId>>,
    rect_y: Mutex<usize>,
}

impl NetworkVisualizeManager {
    const fn new() -> Self {
        Self {
            fn_hooks: Mutex::new(VecDeque::new()),
            window_layer_id: Mutex::new(None),
            canvas_layer_id: Mutex::new(None),
            rect_y: Mutex::new(0),
        }
    }

    fn hook(&self, fn_hook: FunctionHook) {
        kdebug!("net_vis: {:?}", fn_hook);
        self.fn_hooks.spin_lock().push_back(fn_hook);
    }

    fn update_render(&self) -> Result<()> {
        // create window
        {
            let mut window_layer_id = self.window_layer_id.try_lock()?;
            if window_layer_id.is_none() {
                let layer_id = window_manager::create_window(
                    "Network packet visualize".to_string(),
                    WINDOW_DEFAULT_POS,
                    WINDOW_SIZE,
                )?;
                *window_layer_id = Some(layer_id);
            }
        }

        // create canvas and add into window
        {
            let window_layer_id = self.window_layer_id.try_lock()?.unwrap();
            let mut canvas_layer_id = self.canvas_layer_id.try_lock()?;

            if canvas_layer_id.is_none() {
                let canvas = components::Canvas::create_and_push(WINDOW_DEFAULT_POS, CANVAS_SIZE)?;
                *canvas_layer_id = Some(canvas.layer_id());
                window_manager::add_component_to_window(window_layer_id, Box::new(canvas))?;
            }
        }

        // canvas rendering
        let mut rect_y = self.rect_y.try_lock()?;
        *rect_y = (*rect_y + 10) % (CANVAS_SIZE.height - 20); // -20 for rect height
        let y = *rect_y;

        let draw_result = if let Some(canvas_layer_id) = *self.canvas_layer_id.try_lock()? {
            multi_layer::draw_layer(canvas_layer_id, |l| {
                l.fill(ColorCode::BLACK)?;
                let rect = Rect::new(2, 2, CANVAS_SIZE.width - 4, CANVAS_SIZE.height - 4);
                l.draw_rect(rect, ColorCode::GREEN)?;
                l.draw_line(Point::default(), CANVAS_SIZE.wh().into(), ColorCode::RED)?;

                l.draw_string_wrap(
                    Point::new(10, 10),
                    "Testing NetVis Canvas...",
                    ColorCode::WHITE,
                    ColorCode::BLACK,
                )?;

                l.draw_rect(Rect::new(50, y, 20, 20), ColorCode::YELLOW)?;
                Ok(())
            })
        } else {
            Ok(())
        };

        if let Err(Error::LayerError(LayerError::InvalidLayerIdError(_))) = draw_result {
            *self.window_layer_id.try_lock()? = None;
            *self.canvas_layer_id.try_lock()? = None;
        }

        Ok(())
    }
}

pub fn hook(fn_hook: FunctionHook) {
    NET_VIS_MAN.hook(fn_hook);
    let _ = NET_VIS_MAN.update_render();
}

pub fn update_render() -> Result<()> {
    NET_VIS_MAN.update_render()
}
