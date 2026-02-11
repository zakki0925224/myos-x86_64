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
    mem::{bitmap, paging::PAGE_SIZE},
    sync::mutex::Mutex,
};
use alloc::{boxed::Box, format, string::ToString};
use common::geometry::{Point, Rect, Size};
use core::sync::atomic::{AtomicBool, Ordering};

static MEM_VIS_MAN: MemoryVisualizeManager = MemoryVisualizeManager::new();

const WINDOW_DEFAULT_POS: Point = Point::new(350, 0);
const WINDOW_SIZE: Size = Size::new(300, 400);
const CANVAS_SIZE: Size = Size::new(WINDOW_SIZE.width - 8, WINDOW_SIZE.height - 40);

const MINIMAP_X: usize = 8;
const MINIMAP_Y: usize = 8;
const MINIMAP_WIDTH: usize = CANVAS_SIZE.width - 16;
const MINIMAP_HEIGHT: usize = 80;

const STATS_Y: usize = MINIMAP_Y + MINIMAP_HEIGHT + 8;
const USAGE_BAR_Y: usize = STATS_Y + 48;
const USAGE_BAR_HEIGHT: usize = 14;

const EVENT_AREA_Y: usize = USAGE_BAR_Y + USAGE_BAR_HEIGHT + 12;
const EVENT_LINE_HEIGHT: usize = 16;
const MAX_EVENTS: usize = 16;
const EVENT_FLASH_FRAMES: usize = 20;

const COLOR_DARK_BG: ColorCode = ColorCode::new_rgb(20, 20, 30);
const COLOR_PANEL_BG: ColorCode = ColorCode::new_rgb(30, 30, 45);
const COLOR_GRAY: ColorCode = ColorCode::new_rgb(160, 160, 160);
const COLOR_FREE: ColorCode = ColorCode::new_rgb(30, 50, 30);
const COLOR_USED: ColorCode = ColorCode::new_rgb(60, 130, 80);
const COLOR_ALLOC_FLASH: ColorCode = ColorCode::new_rgb(255, 220, 80);
const COLOR_DEALLOC_FLASH: ColorCode = ColorCode::new_rgb(100, 180, 255);
const COLOR_BAR_BG: ColorCode = ColorCode::new_rgb(40, 40, 55);
const COLOR_BAR_FILL: ColorCode = ColorCode::new_rgb(60, 160, 100);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemEvent {
    Alloc,
    Dealloc,
}

#[derive(Debug, Clone, Copy)]
struct MemEventEntry {
    event: MemEvent,
    frame_index: usize,
    frame_count: usize,
    age: usize,
}

impl MemEventEntry {
    const EMPTY: Self = Self {
        event: MemEvent::Alloc,
        frame_index: 0,
        frame_count: 0,
        age: 0,
    };

    fn addr(&self) -> u64 {
        (self.frame_index * PAGE_SIZE) as u64
    }

    fn size_bytes(&self) -> usize {
        self.frame_count * PAGE_SIZE
    }
}

// Fixed-size ring buffer — zero heap allocation on push.
// Safe to call from inside the bitmap memory allocator.
struct EventRingBuffer {
    buf: [MemEventEntry; MAX_EVENTS],
    head: usize,
    count: usize,
}

impl EventRingBuffer {
    const fn new() -> Self {
        Self {
            buf: [MemEventEntry::EMPTY; MAX_EVENTS],
            head: 0,
            count: 0,
        }
    }

    fn push(&mut self, entry: MemEventEntry) {
        self.buf[self.head] = entry;
        self.head = (self.head + 1) % MAX_EVENTS;
        if self.count < MAX_EVENTS {
            self.count += 1;
        }
    }

    fn iter_recent(&self) -> RingIter<'_> {
        RingIter {
            buf: &self.buf,
            remaining: self.count,
            pos: (self.head + MAX_EVENTS - 1) % MAX_EVENTS,
        }
    }

    fn age_all(&mut self) {
        if self.count == 0 {
            return;
        }
        let start = if self.count < MAX_EVENTS {
            0
        } else {
            self.head
        };
        for i in 0..self.count {
            let idx = (start + i) % MAX_EVENTS;
            self.buf[idx].age += 1;
        }
    }
}

struct RingIter<'a> {
    buf: &'a [MemEventEntry; MAX_EVENTS],
    remaining: usize,
    pos: usize,
}

impl<'a> Iterator for RingIter<'a> {
    type Item = &'a MemEventEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let entry = &self.buf[self.pos];
        self.pos = (self.pos + MAX_EVENTS - 1) % MAX_EVENTS;
        self.remaining -= 1;
        Some(entry)
    }
}

struct MemoryVisualizeManager {
    events: Mutex<EventRingBuffer>,
    window_layer_id: Mutex<Option<LayerId>>,
    canvas_layer_id: Mutex<Option<LayerId>>,
    // Re-entrancy guard: prevents push_event from being called recursively
    pushing: AtomicBool,
}

impl MemoryVisualizeManager {
    const fn new() -> Self {
        Self {
            events: Mutex::new(EventRingBuffer::new()),
            window_layer_id: Mutex::new(None),
            canvas_layer_id: Mutex::new(None),
            pushing: AtomicBool::new(false),
        }
    }

    fn push_event(&self, event: MemEvent, frame_index: usize, frame_count: usize) {
        // Guard: if we're already inside push_event (or the lock is held), bail out.
        if self.pushing.swap(true, Ordering::SeqCst) {
            return;
        }

        if let Ok(mut events) = self.events.try_lock() {
            events.push(MemEventEntry {
                event,
                frame_index,
                frame_count,
                age: 0,
            });
        }

        self.pushing.store(false, Ordering::SeqCst);
    }

    fn render_minimap(&self, l: &mut dyn Draw) -> Result<()> {
        l.draw_rect(
            Rect::new(MINIMAP_X, MINIMAP_Y, MINIMAP_WIDTH, MINIMAP_HEIGHT),
            COLOR_PANEL_BG,
        )?;

        let (virt_addr, bitmap_len) = match bitmap::get_bitmap_region() {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        if bitmap_len == 0 {
            return Ok(());
        }

        let bytes_per_col = if bitmap_len > MINIMAP_WIDTH {
            bitmap_len / MINIMAP_WIDTH
        } else {
            1
        };
        let cols = (bitmap_len / bytes_per_col).min(MINIMAP_WIDTH);

        let events = self.events.try_lock();
        let bitmap_ptr = virt_addr.as_ptr::<u8>();

        for col in 0..cols {
            let byte_start = col * bytes_per_col;
            let byte_end = ((col + 1) * bytes_per_col).min(bitmap_len);
            let chunk_len = byte_end - byte_start;

            if chunk_len == 0 {
                continue;
            }

            let mut alloc_bits: usize = 0;
            let total_bits: usize = chunk_len * 8;
            for i in byte_start..byte_end {
                let byte_val = unsafe { bitmap_ptr.add(i).read_volatile() };
                alloc_bits += byte_val.count_ones() as usize;
            }

            let density = if total_bits > 0 {
                (alloc_bits * 255) / total_bits
            } else {
                0
            };

            let r = (COLOR_FREE.r as usize
                + (COLOR_USED.r as usize - COLOR_FREE.r as usize) * density / 255)
                as u8;
            let g = (COLOR_FREE.g as usize
                + (COLOR_USED.g as usize - COLOR_FREE.g as usize) * density / 255)
                as u8;
            let b = (COLOR_FREE.b as usize
                + (COLOR_USED.b as usize - COLOR_FREE.b as usize) * density / 255)
                as u8;
            let mut color = ColorCode::new_rgb(r, g, b);

            // Flash overlay for recent alloc/dealloc events
            let col_frame_start = byte_start * 8;
            let col_frame_end = byte_end * 8;
            if let Ok(ref evts) = events {
                for evt in evts.iter_recent() {
                    if evt.age >= EVENT_FLASH_FRAMES {
                        continue;
                    }
                    let evt_start = evt.frame_index;
                    let evt_end = evt.frame_index + evt.frame_count;
                    if evt_start < col_frame_end && evt_end > col_frame_start {
                        let intensity = 255 - (evt.age * 255 / EVENT_FLASH_FRAMES).min(255);
                        color = match evt.event {
                            MemEvent::Alloc => ColorCode::new_rgb(
                                (COLOR_ALLOC_FLASH.r as usize * intensity / 255) as u8,
                                (COLOR_ALLOC_FLASH.g as usize * intensity / 255) as u8,
                                (COLOR_ALLOC_FLASH.b as usize * intensity / 255) as u8,
                            ),
                            MemEvent::Dealloc => ColorCode::new_rgb(
                                (COLOR_DEALLOC_FLASH.r as usize * intensity / 255) as u8,
                                (COLOR_DEALLOC_FLASH.g as usize * intensity / 255) as u8,
                                (COLOR_DEALLOC_FLASH.b as usize * intensity / 255) as u8,
                            ),
                        };
                        break;
                    }
                }
            }

            let x = MINIMAP_X + col;
            l.draw_rect(Rect::new(x, MINIMAP_Y, 1, MINIMAP_HEIGHT), color)?;
        }

        Ok(())
    }

    fn render_stats(&self, l: &mut dyn Draw) -> Result<()> {
        let (used, total) = match bitmap::get_mem_size() {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let used_mib = used / (1024 * 1024);
        let total_mib = total / (1024 * 1024);
        let percent = if total > 0 { (used * 100) / total } else { 0 };

        let used_frames = used / PAGE_SIZE;
        let total_frames = total / PAGE_SIZE;

        let line1 = format!("Used: {}MiB / {}MiB ({}%)", used_mib, total_mib, percent);
        let line2 = format!("Frames: {} / {}", used_frames, total_frames);

        l.draw_string_wrap(
            Point::new(MINIMAP_X, STATS_Y),
            &line1,
            ColorCode::WHITE,
            COLOR_DARK_BG,
        )?;
        l.draw_string_wrap(
            Point::new(MINIMAP_X, STATS_Y + 16),
            &line2,
            COLOR_GRAY,
            COLOR_DARK_BG,
        )?;

        let bar_width = MINIMAP_WIDTH;
        l.draw_rect(
            Rect::new(MINIMAP_X, USAGE_BAR_Y, bar_width, USAGE_BAR_HEIGHT),
            COLOR_BAR_BG,
        )?;
        let fill_width = if total > 0 {
            (used * bar_width) / total
        } else {
            0
        };
        if fill_width > 0 {
            l.draw_rect(
                Rect::new(MINIMAP_X, USAGE_BAR_Y, fill_width, USAGE_BAR_HEIGHT),
                COLOR_BAR_FILL,
            )?;
        }

        let bar_text = format!("{}%", percent);
        let text_x = MINIMAP_X + bar_width / 2 - 8;
        l.draw_string_wrap(
            Point::new(text_x, USAGE_BAR_Y + 1),
            &bar_text,
            ColorCode::WHITE,
            if (text_x - MINIMAP_X) < fill_width {
                COLOR_BAR_FILL
            } else {
                COLOR_BAR_BG
            },
        )?;

        Ok(())
    }

    fn render_events(&self, l: &mut dyn Draw) -> Result<()> {
        l.draw_string_wrap(
            Point::new(MINIMAP_X, EVENT_AREA_Y),
            "Recent:",
            COLOR_GRAY,
            COLOR_DARK_BG,
        )?;

        let events = self.events.try_lock()?;
        for (i, entry) in events.iter_recent().enumerate() {
            let y = EVENT_AREA_Y + EVENT_LINE_HEIGHT + i * EVENT_LINE_HEIGHT;
            if y + EVENT_LINE_HEIGHT > CANVAS_SIZE.height {
                break;
            }

            let (marker, color) = match entry.event {
                MemEvent::Alloc => ("ALLOC", COLOR_ALLOC_FLASH),
                MemEvent::Dealloc => ("FREE ", COLOR_DEALLOC_FLASH),
            };

            let dimmed = if entry.age > EVENT_FLASH_FRAMES {
                ColorCode::new_rgb(color.r / 2, color.g / 2, color.b / 2)
            } else {
                color
            };

            l.draw_string_wrap(Point::new(MINIMAP_X, y), marker, dimmed, COLOR_DARK_BG)?;

            let size_bytes = entry.size_bytes();
            let size_str = if size_bytes >= 1024 * 1024 {
                format!("{}MiB", size_bytes / (1024 * 1024))
            } else if size_bytes >= 1024 {
                format!("{}KiB", size_bytes / 1024)
            } else {
                format!("{}B", size_bytes)
            };
            let detail = format!("0x{:X} {}", entry.addr(), size_str);
            l.draw_string_wrap(
                Point::new(MINIMAP_X + 48, y),
                &detail,
                COLOR_GRAY,
                COLOR_DARK_BG,
            )?;
        }

        Ok(())
    }

    fn render_canvas(&self, l: &mut dyn Draw) -> Result<()> {
        l.fill(COLOR_DARK_BG)?;
        self.render_minimap(l)?;
        self.render_stats(l)?;
        self.render_events(l)?;
        Ok(())
    }

    fn update_render(&self) -> Result<()> {
        {
            let mut window_layer_id = self.window_layer_id.try_lock()?;
            if window_layer_id.is_none() {
                let layer_id = window_manager::create_window(
                    "Memory visualize".to_string(),
                    WINDOW_DEFAULT_POS,
                    WINDOW_SIZE,
                )?;
                *window_layer_id = Some(layer_id);
            }
        }

        {
            let window_layer_id = self.window_layer_id.try_lock()?.unwrap();
            let mut canvas_layer_id = self.canvas_layer_id.try_lock()?;

            if canvas_layer_id.is_none() {
                let canvas = components::Canvas::create_and_push(WINDOW_DEFAULT_POS, CANVAS_SIZE)?;
                *canvas_layer_id = Some(canvas.layer_id());
                window_manager::add_component_to_window(window_layer_id, Box::new(canvas))?;
            }
        }

        if let Ok(mut events) = self.events.try_lock() {
            events.age_all();
        }

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

pub fn hook(event: MemEvent, frame_index: usize, frame_count: usize) {
    MEM_VIS_MAN.push_event(event, frame_index, frame_count);
}

pub fn update_render() -> Result<()> {
    MEM_VIS_MAN.update_render()
}
