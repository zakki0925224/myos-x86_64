use crate::{
    error::{Error, Result},
    graphics::{
        color::ColorCode,
        draw::Draw,
        multi_layer::{self, LayerError, LayerId},
        window_manager::{
            self,
            components::{self, Component},
        },
    },
    kdebug,
    sync::mutex::Mutex,
};
use alloc::{
    boxed::Box,
    collections::vec_deque::VecDeque,
    string::{String, ToString},
    vec::Vec,
};
use common::geometry::{Point, Rect, Size};

static NET_VIS_MAN: NetworkVisualizeManager = NetworkVisualizeManager::new();

const WINDOW_DEFAULT_POS: Point = Point::new(0, 0);
const WINDOW_SIZE: Size = Size::new(340, 500);
const CANVAS_SIZE: Size = Size::new(WINDOW_SIZE.width - 8, WINDOW_SIZE.height - 40);

// Layout constants
const LAYER_COUNT: usize = 3;
const LAYER_HEIGHT: usize = 100;
const LAYER_AREA_HEIGHT: usize = LAYER_COUNT * LAYER_HEIGHT;
const LAYER_LABEL_X: usize = 5;
const RX_LANE_X: usize = 100;
const TX_LANE_X: usize = 210;
const PACKET_SIZE: usize = 10;
const ANIM_SPEED: usize = 30;
const PAUSE_FRAMES: usize = 5;
const MAX_ANIMATED_PACKETS: usize = 32;
const MAX_LOG_ENTRIES: usize = 8;
const LOG_AREA_Y: usize = LAYER_AREA_HEIGHT + 10;
const LOG_LINE_HEIGHT: usize = 16;

// Custom colors
const COLOR_DARK_GRAY: ColorCode = ColorCode::new_rgb(80, 80, 80);
const COLOR_GRAY: ColorCode = ColorCode::new_rgb(160, 160, 160);
const COLOR_ORANGE: ColorCode = ColorCode::new_rgb(255, 165, 0);
const COLOR_LIGHT_BLUE: ColorCode = ColorCode::new_rgb(100, 180, 255);
const COLOR_DARK_BG: ColorCode = ColorCode::new_rgb(20, 20, 30);
const COLOR_LAYER_BG: ColorCode = ColorCode::new_rgb(30, 30, 45);

const LAYER_LABELS: [&str; LAYER_COUNT] = ["L1 Ethernet", "L2 ARP/IPv4", "L3 TCP/UDP/ICMP"];

#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
enum PacketDirection {
    Rx,
    Tx,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum NetworkLayer {
    L1Ethernet,
    L2Network,
    L3Transport,
}

impl FunctionHook {
    fn direction(&self) -> PacketDirection {
        match self {
            Self::ReceiveEthPayload
            | Self::ReceiveArpPacket
            | Self::ReceiveIpv4Paket
            | Self::ReceiveIcmpPacket
            | Self::ReceiveTcpPacket
            | Self::ReceiveUdpPacket => PacketDirection::Rx,
            _ => PacketDirection::Tx,
        }
    }

    fn layer(&self) -> NetworkLayer {
        match self {
            Self::ReceiveEthPayload | Self::SendEthPayload => NetworkLayer::L1Ethernet,
            Self::ReceiveArpPacket | Self::SendArpPacket | Self::ReceiveIpv4Paket => {
                NetworkLayer::L2Network
            }
            _ => NetworkLayer::L3Transport,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::ReceiveEthPayload | Self::SendEthPayload => "ETH",
            Self::ReceiveArpPacket | Self::SendArpPacket => "ARP",
            Self::ReceiveIpv4Paket => "IPv4",
            Self::ReceiveIcmpPacket => "ICMP",
            Self::ReceiveTcpPacket | Self::SendTcpPacket => "TCP",
            Self::ReceiveUdpPacket | Self::SendUdpPacket => "UDP",
        }
    }

    fn color(&self) -> ColorCode {
        match self {
            Self::ReceiveEthPayload | Self::SendEthPayload => COLOR_GRAY,
            Self::ReceiveArpPacket | Self::SendArpPacket => ColorCode::CYAN,
            Self::ReceiveIpv4Paket => ColorCode::GREEN,
            Self::ReceiveIcmpPacket => ColorCode::YELLOW,
            Self::ReceiveTcpPacket | Self::SendTcpPacket => COLOR_ORANGE,
            Self::ReceiveUdpPacket | Self::SendUdpPacket => COLOR_LIGHT_BLUE,
        }
    }
}

fn layer_center_y(layer: NetworkLayer) -> usize {
    let idx = match layer {
        NetworkLayer::L1Ethernet => 0,
        NetworkLayer::L2Network => 1,
        NetworkLayer::L3Transport => 2,
    };
    idx * LAYER_HEIGHT + LAYER_HEIGHT / 2
}

struct PacketPhase {
    label: &'static str,
    color: ColorCode,
    target_y: usize,
}

struct AnimatedPacket {
    direction: PacketDirection,
    x: usize,
    y: usize,
    phases: Vec<PacketPhase>,
    current_phase: usize,
    pause_remaining: usize,
    age: usize,
    done: bool,
    flow_complete: bool,
}

impl AnimatedPacket {
    fn current_label(&self) -> &'static str {
        self.phases
            .get(self.current_phase)
            .map(|p| p.label)
            .unwrap_or("?")
    }

    fn current_color(&self) -> ColorCode {
        self.phases
            .get(self.current_phase)
            .map(|p| p.color)
            .unwrap_or(COLOR_GRAY)
    }
}

struct LogEntry {
    direction: PacketDirection,
    label: &'static str,
    color: ColorCode,
    detail: String,
}

struct NetworkVisualizeManager {
    fn_hooks: Mutex<VecDeque<(FunctionHook, String)>>,
    window_layer_id: Mutex<Option<LayerId>>,
    canvas_layer_id: Mutex<Option<LayerId>>,
    animated_packets: Mutex<Vec<AnimatedPacket>>,
    log_entries: Mutex<VecDeque<LogEntry>>,
    rx_slot: Mutex<usize>,
    tx_slot: Mutex<usize>,
}

impl NetworkVisualizeManager {
    const fn new() -> Self {
        Self {
            fn_hooks: Mutex::new(VecDeque::new()),
            window_layer_id: Mutex::new(None),
            canvas_layer_id: Mutex::new(None),
            animated_packets: Mutex::new(Vec::new()),
            log_entries: Mutex::new(VecDeque::new()),
            rx_slot: Mutex::new(0),
            tx_slot: Mutex::new(0),
        }
    }

    fn hook(&self, fn_hook: FunctionHook, detail: String) {
        kdebug!("net_vis: {:?} {}", fn_hook, detail);
        self.fn_hooks.spin_lock().push_back((fn_hook, detail));
    }

    fn process_hooks(&self) -> Result<()> {
        let mut hooks = self.fn_hooks.try_lock()?;
        let mut animated = self.animated_packets.try_lock()?;
        let mut logs = self.log_entries.try_lock()?;
        let mut rx_slot = self.rx_slot.try_lock()?;
        let mut tx_slot = self.tx_slot.try_lock()?;

        while let Some((fh, detail)) = hooks.pop_front() {
            let dir = fh.direction();
            let layer = fh.layer();
            let target_y = layer_center_y(layer);

            let phase = PacketPhase {
                label: fh.label(),
                color: fh.color(),
                target_y,
            };

            // RX: ReceiveEthPayload starts a new flow
            // TX: Send{Tcp,Udp,Arp}Packet starts a new flow
            let is_flow_start = matches!(
                fh,
                FunctionHook::ReceiveEthPayload
                    | FunctionHook::SendTcpPacket
                    | FunctionHook::SendUdpPacket
                    | FunctionHook::SendArpPacket
            );

            if is_flow_start {
                let (x_base, slot) = match dir {
                    PacketDirection::Rx => {
                        let s = *rx_slot;
                        *rx_slot = (*rx_slot + 1) % 4;
                        (RX_LANE_X, s)
                    }
                    PacketDirection::Tx => {
                        let s = *tx_slot;
                        *tx_slot = (*tx_slot + 1) % 4;
                        (TX_LANE_X, s)
                    }
                };
                let x = x_base + slot * 20;

                let start_y = match dir {
                    PacketDirection::Rx => 0,
                    PacketDirection::Tx => target_y,
                };

                animated.push(AnimatedPacket {
                    direction: dir,
                    x,
                    y: start_y,
                    phases: vec![phase],
                    current_phase: 0,
                    pause_remaining: if dir == PacketDirection::Tx {
                        PAUSE_FRAMES
                    } else {
                        0
                    },
                    age: 0,
                    done: false,
                    flow_complete: false,
                });

                if animated.len() > MAX_ANIMATED_PACKETS {
                    animated.remove(0);
                }
            } else {
                // Append phase to the most recent incomplete flow of the same direction
                if let Some(pkt) = animated
                    .iter_mut()
                    .rev()
                    .find(|p| p.direction == dir && !p.flow_complete && !p.done)
                {
                    pkt.phases.push(phase);
                    if matches!(fh, FunctionHook::SendEthPayload) {
                        pkt.flow_complete = true;
                    }
                }
            }

            logs.push_back(LogEntry {
                direction: dir,
                label: fh.label(),
                color: fh.color(),
                detail,
            });
            while logs.len() > MAX_LOG_ENTRIES {
                logs.pop_front();
            }
        }

        Ok(())
    }

    fn update_animation(&self) -> Result<()> {
        let mut animated = self.animated_packets.try_lock()?;

        for pkt in animated.iter_mut() {
            if pkt.done {
                pkt.age += 1;
                continue;
            }

            if pkt.phases.is_empty() {
                continue;
            }

            let target = pkt.phases[pkt.current_phase].target_y;

            // Move toward current phase target
            if pkt.y != target {
                if pkt.y < target {
                    pkt.y = (pkt.y + ANIM_SPEED).min(target);
                } else {
                    pkt.y = pkt.y.saturating_sub(ANIM_SPEED);
                    if pkt.y < target {
                        pkt.y = target;
                    }
                }
                // Just arrived at target
                if pkt.y == target {
                    pkt.pause_remaining = PAUSE_FRAMES;
                }
                continue;
            }

            // At target, pausing
            if pkt.pause_remaining > 0 {
                pkt.pause_remaining -= 1;
                continue;
            }

            // Pause done: advance to next phase or finish
            if pkt.current_phase + 1 < pkt.phases.len() {
                pkt.current_phase += 1;
            } else {
                // No more phases: mark done
                pkt.done = true;
            }
        }

        // Remove packets that have faded out
        animated.retain(|p| !p.done || p.age < 30);

        Ok(())
    }

    fn render_canvas(&self, l: &mut dyn Draw) -> Result<()> {
        l.fill(COLOR_DARK_BG)?;

        // draw layer backgrounds and separators
        for i in 0..LAYER_COUNT {
            let y = i * LAYER_HEIGHT;

            // layer background (slightly lighter)
            if i % 2 == 0 {
                l.draw_rect(
                    Rect::new(0, y, CANVAS_SIZE.width, LAYER_HEIGHT),
                    COLOR_LAYER_BG,
                )?;
            }

            // separator line
            if i > 0 {
                // draw dashed line by drawing short segments
                let mut dx = 0;
                while dx < CANVAS_SIZE.width {
                    let seg_len = 6.min(CANVAS_SIZE.width - dx);
                    l.draw_rect(Rect::new(dx, y, seg_len, 1), COLOR_DARK_GRAY)?;
                    dx += 12;
                }
            }

            // layer label
            l.draw_string_wrap(
                Point::new(LAYER_LABEL_X, y + 4),
                LAYER_LABELS[i],
                COLOR_DARK_GRAY,
                if i % 2 == 0 {
                    COLOR_LAYER_BG
                } else {
                    COLOR_DARK_BG
                },
            )?;
        }

        // draw lane labels at top
        l.draw_string_wrap(
            Point::new(RX_LANE_X + 10, 4),
            "RX v",
            ColorCode::GREEN,
            COLOR_LAYER_BG,
        )?;
        l.draw_string_wrap(
            Point::new(TX_LANE_X + 10, 4),
            "TX ^",
            COLOR_ORANGE,
            COLOR_LAYER_BG,
        )?;

        // draw lane guide lines (vertical, thin)
        for y in 20..LAYER_AREA_HEIGHT {
            if y % 4 < 2 {
                l.draw_rect(
                    Rect::new(RX_LANE_X + 30, y, 1, 1),
                    ColorCode::new_rgb(40, 60, 40),
                )?;
                l.draw_rect(
                    Rect::new(TX_LANE_X + 30, y, 1, 1),
                    ColorCode::new_rgb(60, 40, 40),
                )?;
            }
        }

        // draw animated packets
        let animated = self.animated_packets.try_lock()?;
        for pkt in animated.iter() {
            let base_color = pkt.current_color();

            // fade out effect
            let color = if pkt.done && pkt.age > 15 {
                ColorCode::new_rgb(base_color.r / 3, base_color.g / 3, base_color.b / 3)
            } else if pkt.done {
                ColorCode::new_rgb(base_color.r / 2, base_color.g / 2, base_color.b / 2)
            } else {
                base_color
            };

            // packet dot
            if pkt.y + PACKET_SIZE <= CANVAS_SIZE.height && pkt.x + PACKET_SIZE <= CANVAS_SIZE.width
            {
                l.draw_rect(Rect::new(pkt.x, pkt.y, PACKET_SIZE, PACKET_SIZE), color)?;

                // label next to the dot
                let label_x = pkt.x + PACKET_SIZE + 2;
                let bg = if (pkt.y / LAYER_HEIGHT) % 2 == 0 {
                    COLOR_LAYER_BG
                } else {
                    COLOR_DARK_BG
                };
                if label_x + 40 <= CANVAS_SIZE.width {
                    l.draw_string_wrap(Point::new(label_x, pkt.y), pkt.current_label(), color, bg)?;
                }
            }
        }

        // draw log area separator
        if LOG_AREA_Y + 2 <= CANVAS_SIZE.height {
            l.draw_rect(
                Rect::new(0, LOG_AREA_Y - 2, CANVAS_SIZE.width, 1),
                COLOR_DARK_GRAY,
            )?;
            l.draw_string_wrap(
                Point::new(2, LOG_AREA_Y),
                "Recent:",
                COLOR_GRAY,
                COLOR_DARK_BG,
            )?;
        }

        // draw log entries
        let logs = self.log_entries.try_lock()?;
        for (i, entry) in logs.iter().rev().enumerate() {
            let y = LOG_AREA_Y + LOG_LINE_HEIGHT + i * LOG_LINE_HEIGHT;
            if y + LOG_LINE_HEIGHT > CANVAS_SIZE.height {
                break;
            }

            let dir_str = match entry.direction {
                PacketDirection::Rx => "RX",
                PacketDirection::Tx => "TX",
            };
            let dir_color = match entry.direction {
                PacketDirection::Rx => ColorCode::GREEN,
                PacketDirection::Tx => COLOR_ORANGE,
            };

            l.draw_string_wrap(Point::new(4, y), dir_str, dir_color, COLOR_DARK_BG)?;
            l.draw_string_wrap(Point::new(30, y), entry.label, entry.color, COLOR_DARK_BG)?;
            l.draw_string_wrap(Point::new(70, y), &entry.detail, COLOR_GRAY, COLOR_DARK_BG)?;
        }

        Ok(())
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

        // process new hooks into animated packets
        self.process_hooks()?;

        // update animation state
        self.update_animation()?;

        // render
        let draw_result = if let Some(canvas_layer_id) = *self.canvas_layer_id.try_lock()? {
            multi_layer::draw_layer(canvas_layer_id, |l| self.render_canvas(l))
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

pub fn hook(fn_hook: FunctionHook, detail: String) {
    NET_VIS_MAN.hook(fn_hook, detail);
    let _ = NET_VIS_MAN.update_render();
}

pub fn update_render() -> Result<()> {
    NET_VIS_MAN.update_render()
}
