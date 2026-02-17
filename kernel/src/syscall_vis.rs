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
    sync::mutex::Mutex,
};
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
};
use common::geometry::{Point, Rect, Size};
use core::sync::atomic::{AtomicU64, Ordering};

static SYSCALL_VIS_MAN: SyscallVisualizeManager = SyscallVisualizeManager::new();

const WINDOW_DEFAULT_POS: Point = Point::new(660, 0);
const WINDOW_SIZE: Size = Size::new(320, 300);
const CANVAS_SIZE: Size = Size::new(WINDOW_SIZE.width - 8, WINDOW_SIZE.height - 40);

const MARGIN: usize = 8;
const BAR_HEIGHT: usize = 10;
const BAR_GAP: usize = 2;
const CATEGORY_GAP: usize = 6;

// Colors
const COLOR_DARK_BG: ColorCode = ColorCode::new_rgb(20, 20, 30);
const COLOR_PANEL_BG: ColorCode = ColorCode::new_rgb(30, 30, 45);
const COLOR_GRAY: ColorCode = ColorCode::new_rgb(160, 160, 160);
const COLOR_DIM: ColorCode = ColorCode::new_rgb(100, 100, 100);
const COLOR_BAR_BG: ColorCode = ColorCode::new_rgb(40, 40, 55);

const COLOR_FILE_IO: ColorCode = ColorCode::new_rgb(80, 180, 120);
const COLOR_MEMORY: ColorCode = ColorCode::new_rgb(180, 130, 60);
const COLOR_PROCESS: ColorCode = ColorCode::new_rgb(100, 160, 220);
const COLOR_NETWORK: ColorCode = ColorCode::new_rgb(200, 100, 140);
const COLOR_SYSTEM: ColorCode = ColorCode::new_rgb(140, 120, 200);

const COLOR_FLASH: ColorCode = ColorCode::new_rgb(255, 255, 200);

// Format large numbers with SI-like suffixes
fn format_count(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{}.{}G", n / 1_000_000_000, (n / 100_000_000) % 10)
    } else if n >= 1_000_000 {
        format!("{}.{}M", n / 1_000_000, (n / 100_000) % 10)
    } else if n >= 10_000 {
        format!("{}.{}K", n / 1_000, (n / 100) % 10)
    } else {
        format!("{}", n)
    }
}

// Syscall categories
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyscallCategory {
    FileIO,
    Memory,
    Process,
    Network,
    System,
}

impl SyscallCategory {
    fn label(&self) -> &'static str {
        match self {
            Self::FileIO => "File I/O",
            Self::Memory => "Memory",
            Self::Process => "Process",
            Self::Network => "Network",
            Self::System => "System",
        }
    }

    fn color(&self) -> ColorCode {
        match self {
            Self::FileIO => COLOR_FILE_IO,
            Self::Memory => COLOR_MEMORY,
            Self::Process => COLOR_PROCESS,
            Self::Network => COLOR_NETWORK,
            Self::System => COLOR_SYSTEM,
        }
    }

    fn index(&self) -> usize {
        match self {
            Self::FileIO => 0,
            Self::Memory => 1,
            Self::Process => 2,
            Self::Network => 3,
            Self::System => 4,
        }
    }
}

const CATEGORY_COUNT: usize = 5;
const ALL_CATEGORIES: [SyscallCategory; CATEGORY_COUNT] = [
    SyscallCategory::FileIO,
    SyscallCategory::Memory,
    SyscallCategory::Process,
    SyscallCategory::Network,
    SyscallCategory::System,
];

#[derive(Debug, Clone, Copy)]
pub struct SyscallInfo {
    pub name: &'static str,
    pub category: SyscallCategory,
}

// Map syscall number to info. Index = SN_* constant value.
const MAX_SYSCALL_NUM: usize = 28;
const SYSCALL_TABLE: [Option<SyscallInfo>; MAX_SYSCALL_NUM] = [
    /* 0  SN_READ     */
    Some(SyscallInfo {
        name: "read",
        category: SyscallCategory::FileIO,
    }),
    /* 1  SN_WRITE    */
    Some(SyscallInfo {
        name: "write",
        category: SyscallCategory::FileIO,
    }),
    /* 2  SN_OPEN     */
    Some(SyscallInfo {
        name: "open",
        category: SyscallCategory::FileIO,
    }),
    /* 3  SN_CLOSE    */
    Some(SyscallInfo {
        name: "close",
        category: SyscallCategory::FileIO,
    }),
    /* 4  SN_EXIT     */
    Some(SyscallInfo {
        name: "exit",
        category: SyscallCategory::Process,
    }),
    /* 5  SN_SBRK     */
    Some(SyscallInfo {
        name: "sbrk",
        category: SyscallCategory::Memory,
    }),
    /* 6  SN_UNAME    */
    Some(SyscallInfo {
        name: "uname",
        category: SyscallCategory::System,
    }),
    /* 7  SN_BREAK    */
    Some(SyscallInfo {
        name: "break",
        category: SyscallCategory::System,
    }),
    /* 8  SN_STAT     */
    Some(SyscallInfo {
        name: "stat",
        category: SyscallCategory::FileIO,
    }),
    /* 9  SN_UPTIME   */
    Some(SyscallInfo {
        name: "uptime",
        category: SyscallCategory::System,
    }),
    /* 10 SN_EXEC     */
    Some(SyscallInfo {
        name: "exec",
        category: SyscallCategory::Process,
    }),
    /* 11 SN_GETCWD   */
    Some(SyscallInfo {
        name: "getcwd",
        category: SyscallCategory::FileIO,
    }),
    /* 12 SN_CHDIR    */
    Some(SyscallInfo {
        name: "chdir",
        category: SyscallCategory::FileIO,
    }),
    /* 13 SN_FREE     */
    Some(SyscallInfo {
        name: "free",
        category: SyscallCategory::Memory,
    }),
    /* 14             */ None,
    /* 15 SN_SBRKSZ   */
    Some(SyscallInfo {
        name: "sbrksz",
        category: SyscallCategory::Memory,
    }),
    /* 16             */ None,
    /* 17 SN_GETENAMES*/
    Some(SyscallInfo {
        name: "getenames",
        category: SyscallCategory::FileIO,
    }),
    /* 18 SN_IOMSG    */
    Some(SyscallInfo {
        name: "iomsg",
        category: SyscallCategory::System,
    }),
    /* 19 SN_SOCKET   */
    Some(SyscallInfo {
        name: "socket",
        category: SyscallCategory::Network,
    }),
    /* 20 SN_BIND     */
    Some(SyscallInfo {
        name: "bind",
        category: SyscallCategory::Network,
    }),
    /* 21 SN_SENDTO   */
    Some(SyscallInfo {
        name: "sendto",
        category: SyscallCategory::Network,
    }),
    /* 22 SN_RECVFROM */
    Some(SyscallInfo {
        name: "recvfrom",
        category: SyscallCategory::Network,
    }),
    /* 23 SN_SEND     */
    Some(SyscallInfo {
        name: "send",
        category: SyscallCategory::Network,
    }),
    /* 24 SN_RECV     */
    Some(SyscallInfo {
        name: "recv",
        category: SyscallCategory::Network,
    }),
    /* 25 SN_CONNECT  */
    Some(SyscallInfo {
        name: "connect",
        category: SyscallCategory::Network,
    }),
    /* 26 SN_LISTEN   */
    Some(SyscallInfo {
        name: "listen",
        category: SyscallCategory::Network,
    }),
    /* 27 SN_ACCEPT   */
    Some(SyscallInfo {
        name: "accept",
        category: SyscallCategory::Network,
    }),
];

fn lookup_syscall(num: u32) -> Option<&'static SyscallInfo> {
    if (num as usize) < MAX_SYSCALL_NUM {
        SYSCALL_TABLE[num as usize].as_ref()
    } else {
        None
    }
}

struct SyscallCounters {
    counts: [AtomicU64; MAX_SYSCALL_NUM],
    category_counts: [AtomicU64; CATEGORY_COUNT],
}

impl SyscallCounters {
    const fn new() -> Self {
        // Work around const fn limitations
        const ZERO: AtomicU64 = AtomicU64::new(0);
        Self {
            counts: [
                ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO,
                ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO, ZERO,
            ],
            category_counts: [ZERO, ZERO, ZERO, ZERO, ZERO],
        }
    }

    fn increment(&self, syscall_num: u32) {
        if let Some(info) = lookup_syscall(syscall_num) {
            self.counts[syscall_num as usize].fetch_add(1, Ordering::Relaxed);
            self.category_counts[info.category.index()].fetch_add(1, Ordering::Relaxed);
        }
    }

    fn get(&self, syscall_num: usize) -> u64 {
        self.counts[syscall_num].load(Ordering::Relaxed)
    }

    fn get_category(&self, cat: SyscallCategory) -> u64 {
        self.category_counts[cat.index()].load(Ordering::Relaxed)
    }

    fn total(&self) -> u64 {
        let mut sum = 0u64;
        for c in &self.category_counts {
            sum += c.load(Ordering::Relaxed);
        }
        sum
    }
}

// Previous snapshot for rate computation
struct CategorySnapshot {
    counts: [u64; CATEGORY_COUNT],
    total: u64,
}

impl CategorySnapshot {
    const fn new() -> Self {
        Self {
            counts: [0; CATEGORY_COUNT],
            total: 0,
        }
    }
}

struct SyscallVisualizeManager {
    counters: SyscallCounters,
    prev_snapshot: Mutex<CategorySnapshot>,
    rates: Mutex<[u64; CATEGORY_COUNT]>,
    window_layer_id: Mutex<Option<LayerId>>,
    canvas_layer_id: Mutex<Option<LayerId>>,
}

impl SyscallVisualizeManager {
    const fn new() -> Self {
        Self {
            counters: SyscallCounters::new(),
            prev_snapshot: Mutex::new(CategorySnapshot::new()),
            rates: Mutex::new([0; CATEGORY_COUNT]),
            window_layer_id: Mutex::new(None),
            canvas_layer_id: Mutex::new(None),
        }
    }

    fn update_rates(&self) {
        let mut snap = match self.prev_snapshot.try_lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let mut rates = match self.rates.try_lock() {
            Ok(r) => r,
            Err(_) => return,
        };

        for cat in &ALL_CATEGORIES {
            let current = self.counters.get_category(*cat);
            let prev = snap.counts[cat.index()];
            rates[cat.index()] = current.saturating_sub(prev);
            snap.counts[cat.index()] = current;
        }

        let current_total = self.counters.total();
        snap.total = current_total;
    }

    fn render_header(&self, l: &mut dyn Draw) -> Result<()> {
        let total = self.counters.total();
        let header = format!("Total syscalls: {}", format_count(total));
        l.draw_string_wrap(
            Point::new(MARGIN, MARGIN),
            &header,
            ColorCode::WHITE,
            COLOR_DARK_BG,
        )?;
        Ok(())
    }

    fn render_category_bars(&self, l: &mut dyn Draw) -> Result<()> {
        let rates = self.rates.try_lock()?;
        let max_rate = rates.iter().copied().max().unwrap_or(1).max(1);
        let bar_area_width = CANVAS_SIZE.width - MARGIN * 2 - 80;

        let mut y = MARGIN + 20;

        for cat in &ALL_CATEGORIES {
            let count = self.counters.get_category(*cat);
            let rate = rates[cat.index()];

            // Category label
            l.draw_string_wrap(
                Point::new(MARGIN, y),
                cat.label(),
                cat.color(),
                COLOR_DARK_BG,
            )?;

            // Count
            let count_str = format_count(count);
            l.draw_string_wrap(
                Point::new(MARGIN + 72, y),
                &count_str,
                COLOR_GRAY,
                COLOR_DARK_BG,
            )?;

            // Activity bar (rate-based)
            let bar_y = y + 12;
            l.draw_rect(
                Rect::new(MARGIN, bar_y, bar_area_width, BAR_HEIGHT),
                COLOR_BAR_BG,
            )?;

            let fill_width = if max_rate > 0 {
                ((rate as usize) * bar_area_width) / (max_rate as usize)
            } else {
                0
            };

            if fill_width > 0 {
                l.draw_rect(
                    Rect::new(MARGIN, bar_y, fill_width, BAR_HEIGHT),
                    cat.color(),
                )?;

                // Flash effect on the leading edge
                if rate > 0 {
                    let flash_x = MARGIN + fill_width.saturating_sub(2);
                    let flash_w = 2.min(fill_width);
                    l.draw_rect(Rect::new(flash_x, bar_y, flash_w, BAR_HEIGHT), COLOR_FLASH)?;
                }
            }

            // Rate indicator
            let rate_str = format!("+{}/t", format_count(rate));
            l.draw_string_wrap(
                Point::new(MARGIN + bar_area_width + 4, bar_y),
                &rate_str,
                if rate > 0 { cat.color() } else { COLOR_DIM },
                COLOR_DARK_BG,
            )?;

            y += BAR_HEIGHT + BAR_GAP + 14 + CATEGORY_GAP;
        }

        Ok(())
    }

    fn render_top_syscalls(&self, l: &mut dyn Draw) -> Result<()> {
        let y_base = 190;

        l.draw_string_wrap(
            Point::new(MARGIN, y_base),
            "Top syscalls:",
            COLOR_GRAY,
            COLOR_DARK_BG,
        )?;

        // Collect (index, count) pairs
        let mut entries: [(usize, u64); MAX_SYSCALL_NUM] = [(0, 0); MAX_SYSCALL_NUM];
        for i in 0..MAX_SYSCALL_NUM {
            entries[i] = (i, self.counters.get(i));
        }

        // Simple insertion sort by count descending (no alloc)
        for i in 1..MAX_SYSCALL_NUM {
            let mut j = i;
            while j > 0 && entries[j].1 > entries[j - 1].1 {
                entries.swap(j, j - 1);
                j -= 1;
            }
        }

        let max_show = 5;
        let mut y = y_base + 16;

        for i in 0..max_show {
            let (idx, count) = entries[i];
            if count == 0 {
                break;
            }

            if let Some(info) = lookup_syscall(idx as u32) {
                let line = format!("{:<10} {:>8}", info.name, format_count(count));
                l.draw_string_wrap(
                    Point::new(MARGIN + 4, y),
                    &line,
                    info.category.color(),
                    COLOR_DARK_BG,
                )?;
                y += 14;
            }
        }

        Ok(())
    }

    fn render_canvas(&self, l: &mut dyn Draw) -> Result<()> {
        l.fill(COLOR_DARK_BG)?;
        self.render_header(l)?;
        self.render_category_bars(l)?;
        self.render_top_syscalls(l)?;
        Ok(())
    }

    fn update_render(&self) -> Result<()> {
        {
            let mut window_layer_id = self.window_layer_id.try_lock()?;
            if window_layer_id.is_none() {
                let layer_id = window_manager::create_window(
                    "Syscall visualize".to_string(),
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

        // Update rate deltas
        self.update_rates();

        // Render
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

/// Called from syscall_handler after each syscall completes.
pub fn hook(syscall_num: u32, _result: i64) {
    SYSCALL_VIS_MAN.counters.increment(syscall_num);
}

/// Called from async render loop.
pub fn update_render() -> Result<()> {
    SYSCALL_VIS_MAN.update_render()
}
