use super::{
    color::ColorCode,
    fb,
    font::FONT,
    multi_layer::{self, LayerId},
};
use crate::{
    error::Result,
    sync::mutex::Mutex,
    theme::GLOBAL_THEME,
    util::ansi::{AnsiColor, AnsiEscapeStream, AnsiEvent, CsiSequence, SgrAttr},
};
use common::geometry::{Point, Rect, Size};

const TAB_WIDTH: usize = 8;

static FB_CONSOLE: Mutex<FrameBufferConsole> = Mutex::new(FrameBufferConsole::new());

struct FrameBufferConsole {
    default_back_color: ColorCode,
    back_color: ColorCode,
    default_fore_color: ColorCode,
    fore_color: ColorCode,
    cursor_x: usize,
    cursor_y: usize,
    screen_size: Size,
    target_layer_id: Option<LayerId>,
    ansi_escape_stream: AnsiEscapeStream,
    is_hidden: bool,
    pending_scroll_lines: usize,
}

impl FrameBufferConsole {
    const fn new() -> Self {
        Self {
            default_back_color: ColorCode::default(),
            back_color: ColorCode::default(),
            default_fore_color: ColorCode::default(),
            fore_color: ColorCode::default(),
            cursor_x: 0,
            cursor_y: 0,
            screen_size: Size::new(0, 0),
            target_layer_id: None,
            ansi_escape_stream: AnsiEscapeStream::new(),
            is_hidden: false,
            pending_scroll_lines: 0,
        }
    }

    fn is_ready(&self) -> bool {
        let (f_w, f_h) = FONT.wh();
        self.screen_size.width >= f_w && self.screen_size.height >= f_h
    }

    fn cursor_max(&self) -> (usize, usize) {
        let (width, height) = self.screen_size.wh();
        let (f_w, f_h) = FONT.wh();
        (width / f_w - 1, height / f_h - 1)
    }

    fn init(&mut self, back_color: ColorCode, fore_color: ColorCode) -> Result<()> {
        self.default_back_color = back_color;
        self.back_color = back_color;
        self.default_fore_color = fore_color;
        self.fore_color = fore_color;

        self.screen_size = match &self.target_layer_id {
            Some(layer_id) => multi_layer::layer_info(*layer_id)?.size,
            None => fb::resolution()?,
        };

        self.cursor_x = 0;
        self.cursor_y = 2;

        self.fill(self.back_color)?;

        for (i, color) in GLOBAL_THEME.console.palette.iter().enumerate() {
            let rect = Rect::new(i * 20, 0, 20, 20);
            self.draw_rect(rect, *color)?;
        }

        Ok(())
    }

    fn set_target_layer_id(&mut self, layer_id: LayerId) -> Result<()> {
        self.target_layer_id = Some(layer_id);

        // update
        self.init(self.default_back_color, self.default_fore_color)
    }

    fn write_char(&mut self, c: char) -> Result<()> {
        if !self.is_ready() {
            return Ok(());
        }

        match c {
            '\n' => return self.new_line(),
            '\r' => {
                self.cursor_x = 0;
                return Ok(());
            }
            '\t' => return self.tab(),
            '\x08' | '\x7f' => return self.backspace(),
            _ => (),
        }

        match self.ansi_escape_stream.push(c) {
            AnsiEvent::Print => self.put_char(c),
            AnsiEvent::Consumed => Ok(()),
            AnsiEvent::Csi(seq) => self.exec_csi(seq),
        }
    }

    fn write_str(&mut self, s: &str) -> Result<()> {
        for c in s.chars() {
            self.write_char(c)?;
        }

        Ok(())
    }

    fn put_char(&mut self, c: char) -> Result<()> {
        if self.is_hidden {
            return Ok(());
        }

        let (f_w, f_h) = FONT.wh();
        self.flush_scroll()?;
        let point = Point::new(self.cursor_x * f_w, self.cursor_y * f_h);
        self.draw_font(point, c, self.fore_color, self.back_color)?;
        self.inc_cursor();
        Ok(())
    }

    fn exec_csi(&mut self, seq: CsiSequence) -> Result<()> {
        let (f_w, f_h) = FONT.wh();
        let (s_w, s_h) = self.screen_size.wh();
        let (cursor_max_x, cursor_max_y) = self.cursor_max();

        match seq {
            CsiSequence::CursorUp(n) => {
                self.cursor_y = self.cursor_y.saturating_sub(n as usize);
            }
            CsiSequence::CursorDown(n) => {
                self.cursor_y = (self.cursor_y + n as usize).min(cursor_max_y);
            }
            CsiSequence::CursorRight(n) => {
                self.cursor_x = (self.cursor_x + n as usize).min(cursor_max_x);
            }
            CsiSequence::CursorLeft(n) => {
                self.cursor_x = self.cursor_x.saturating_sub(n as usize);
            }
            CsiSequence::CursorNextLineHead(n) => {
                self.cursor_x = 0;
                self.cursor_y = (self.cursor_y + n as usize).min(cursor_max_y);
            }
            CsiSequence::CursorPrevLineHead(n) => {
                self.cursor_x = 0;
                self.cursor_y = self.cursor_y.saturating_sub(n as usize);
            }
            CsiSequence::CursorColumn(n) => {
                self.cursor_x = (n as usize - 1).min(cursor_max_x);
            }
            CsiSequence::CursorPosition { row, col } => {
                self.cursor_x = (col as usize - 1).min(cursor_max_x);
                self.cursor_y = (row as usize - 1).min(cursor_max_y);
            }
            CsiSequence::ScrollUp(n) => {
                self.pending_scroll_lines += n as usize;
                self.flush_scroll()?;
            }
            CsiSequence::ScrollDown(_) => (),
            CsiSequence::ClearScreenAfterCursor => {
                let x = self.cursor_x * f_w;
                let y = self.cursor_y * f_h;
                self.draw_rect(Rect::new(x, y, s_w - x, f_h), self.back_color)?;
                let y = y + f_h;
                self.draw_rect(Rect::new(0, y, s_w, s_h.saturating_sub(y)), self.back_color)?;
            }
            CsiSequence::ClearScreenBeforeCursor => {
                let y = self.cursor_y * f_h;
                self.draw_rect(Rect::new(0, 0, s_w, y), self.back_color)?;
                let w = (self.cursor_x + 1) * f_w;
                self.draw_rect(Rect::new(0, y, w.min(s_w), f_h), self.back_color)?;
            }
            CsiSequence::ClearScreenAll => {
                self.fill(self.back_color)?;
            }
            CsiSequence::ClearRowAfterCursor => {
                let x = self.cursor_x * f_w;
                let y = self.cursor_y * f_h;
                self.draw_rect(Rect::new(x, y, s_w - x, f_h), self.back_color)?;
            }
            CsiSequence::ClearRowBeforeCursor => {
                let y = self.cursor_y * f_h;
                let w = (self.cursor_x + 1) * f_w;
                self.draw_rect(Rect::new(0, y, w.min(s_w), f_h), self.back_color)?;
            }
            CsiSequence::ClearRowAll => {
                let y = self.cursor_y * f_h;
                self.draw_rect(Rect::new(0, y, s_w, f_h), self.back_color)?;
            }
            CsiSequence::Sgr(params) => {
                for attr in params.sgr() {
                    self.exec_sgr(attr);
                }
            }
        }

        Ok(())
    }

    fn exec_sgr(&mut self, attr: SgrAttr) {
        match attr {
            SgrAttr::Reset => {
                self.back_color = self.default_back_color;
                self.fore_color = self.default_fore_color;
                self.is_hidden = false;
            }
            SgrAttr::Reverse => {
                core::mem::swap(&mut self.fore_color, &mut self.back_color);
            }
            SgrAttr::Hidden => {
                self.is_hidden = true;
            }
            SgrAttr::Fore(color) => {
                self.fore_color = ansi_color(color);
            }
            SgrAttr::Back(color) => {
                self.back_color = ansi_color(color);
            }
            SgrAttr::DefaultFore => {
                self.fore_color = self.default_fore_color;
            }
            SgrAttr::DefaultBack => {
                self.back_color = self.default_back_color;
            }
            SgrAttr::Unsupported(_) => (),
        }
    }

    fn inc_cursor(&mut self) {
        let (cursor_max_x, cursor_max_y) = self.cursor_max();

        self.cursor_x += 1;

        if self.cursor_x > cursor_max_x {
            self.cursor_x = 0;
            self.cursor_y += 1;
        }

        if self.cursor_y > cursor_max_y {
            self.scroll();
            self.cursor_x = 0;
            self.cursor_y = cursor_max_y;
        }
    }

    fn dec_cursor(&mut self) {
        let (cursor_max_x, _) = self.cursor_max();

        if self.cursor_x == 0 {
            if self.cursor_y > 0 {
                self.cursor_x = cursor_max_x;
                self.cursor_y -= 1;
            }
        } else {
            self.cursor_x -= 1;
        }
    }

    fn tab(&mut self) -> Result<()> {
        for _ in 0..TAB_WIDTH - self.cursor_x % TAB_WIDTH {
            self.put_char(' ')?;
        }

        Ok(())
    }

    fn new_line(&mut self) -> Result<()> {
        let (_, cursor_max_y) = self.cursor_max();

        self.cursor_x = 0;
        self.cursor_y += 1;

        if self.cursor_y > cursor_max_y {
            self.scroll();
            self.cursor_y = cursor_max_y;
        }

        Ok(())
    }

    fn scroll(&mut self) {
        self.pending_scroll_lines += 1;
    }

    fn flush_scroll(&mut self) -> Result<()> {
        if self.pending_scroll_lines == 0 {
            return Ok(());
        }

        let (_, f_h) = FONT.wh();
        let (w, h) = self.screen_size.wh();
        let scroll_px = self.pending_scroll_lines * f_h;
        self.pending_scroll_lines = 0;

        if scroll_px >= h {
            self.fill(self.back_color)
        } else {
            let src = Point::new(0, scroll_px);
            let copy_size = Size::new(w, h - scroll_px);
            self.copy_rect(src, Point::default(), copy_size)?;

            let rect = Rect::new(0, h - scroll_px, w, scroll_px);
            self.draw_rect(rect, self.back_color)
        }
    }

    fn fill(&self, color: ColorCode) -> Result<()> {
        if let Some(layer_id) = &self.target_layer_id {
            multi_layer::draw_layer(*layer_id, |l| l.fill(color))?;
        } else {
            fb::fill(color)?;
        }

        Ok(())
    }

    fn draw_rect(&self, rect: Rect, color: ColorCode) -> Result<()> {
        if let Some(layer_id) = &self.target_layer_id {
            multi_layer::draw_layer(*layer_id, |l| l.draw_rect(rect, color))?;
        } else {
            fb::draw_rect(rect, color)?;
        }

        Ok(())
    }

    fn copy_rect(&self, src: Point, dst: Point, size: Size) -> Result<()> {
        if let Some(layer_id) = &self.target_layer_id {
            multi_layer::draw_layer(*layer_id, |l| l.copy_rect(src, dst, size))?;
        } else {
            fb::copy_rect(src, dst, size)?;
        }

        Ok(())
    }

    fn draw_font(
        &self,
        point: Point,
        c: char,
        fore_color: ColorCode,
        back_color: ColorCode,
    ) -> Result<()> {
        if let Some(layer_id) = &self.target_layer_id {
            multi_layer::draw_layer(*layer_id, |l| l.draw_char(point, c, fore_color, back_color))?;
        } else {
            fb::draw_char(point, c, fore_color, back_color)?;
        }

        Ok(())
    }

    fn backspace(&mut self) -> Result<()> {
        let (f_w, f_h) = FONT.wh();

        self.dec_cursor();
        let rect = Rect::new(self.cursor_x * f_w, self.cursor_y * f_h, f_w, f_h);
        self.draw_rect(rect, self.back_color)?;

        Ok(())
    }
}

fn ansi_color(color: AnsiColor) -> ColorCode {
    const BASIC: [ColorCode; 8] = [
        ColorCode::BLACK,
        ColorCode::RED,
        ColorCode::GREEN,
        ColorCode::YELLOW,
        ColorCode::BLUE,
        ColorCode::MAGENTA,
        ColorCode::CYAN,
        ColorCode::WHITE,
    ];
    const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

    match color {
        AnsiColor::Rgb(r, g, b) => ColorCode::new_rgb(r, g, b),
        AnsiColor::Indexed(n @ 0..=15) => BASIC[n as usize % 8],
        AnsiColor::Indexed(n @ 16..=231) => {
            let n = (n - 16) as usize;
            ColorCode::new_rgb(
                CUBE_LEVELS[n / 36],
                CUBE_LEVELS[n / 6 % 6],
                CUBE_LEVELS[n % 6],
            )
        }
        AnsiColor::Indexed(n) => {
            let v = 8 + (n - 232) * 10; // grayscale ramp
            ColorCode::new_rgb(v, v, v)
        }
    }
}

pub fn init(back_color: ColorCode, fore_color: ColorCode) -> Result<()> {
    FB_CONSOLE.try_lock()?.init(back_color, fore_color)
}

pub fn set_target_layer_id(layer_id: LayerId) -> Result<()> {
    FB_CONSOLE.try_lock()?.set_target_layer_id(layer_id)
}

pub fn write_str(s: &str) -> Result<()> {
    let _ = FB_CONSOLE.try_lock()?.write_str(s);
    Ok(())
}
