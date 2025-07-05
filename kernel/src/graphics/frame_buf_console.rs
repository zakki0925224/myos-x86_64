use super::{
    font::FONT,
    frame_buf,
    multi_layer::{self, LayerId},
};
use crate::{
    error::Result,
    theme::GLOBAL_THEME,
    util::{
        ansi::{AnsiEscapeStream, AnsiEvent, CsiSequence},
        mutex::Mutex,
    },
    ColorCode,
};
use core::fmt;

static mut FRAME_BUF_CONSOLE: Mutex<FrameBufferConsole> = Mutex::new(FrameBufferConsole::new());

struct FrameBufferConsole {
    default_back_color: ColorCode,
    back_color: ColorCode,
    default_fore_color: ColorCode,
    fore_color: ColorCode,
    cursor_x: usize,
    cursor_y: usize,
    target_layer_id: Option<LayerId>,
    ansi_escape_stream: AnsiEscapeStream,
    is_hidden: bool,
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
            target_layer_id: None,
            ansi_escape_stream: AnsiEscapeStream::new(),
            is_hidden: false,
        }
    }

    fn screen_wh(&self) -> Result<(usize, usize)> {
        if let Some(layer_id) = &self.target_layer_id {
            let wh = multi_layer::get_layer_info(layer_id)?.wh;
            Ok(wh)
        } else {
            frame_buf::resolution()
        }
    }

    fn cursor_max(&self) -> Result<(usize, usize)> {
        let (s_w, s_h) = self.screen_wh()?;
        let (f_w, f_h) = FONT.get_wh();
        let cursor_max_x = s_w / f_w - 1;
        let cursor_max_y = s_h / f_h - 1;
        Ok((cursor_max_x, cursor_max_y))
    }

    fn init(&mut self, back_color: ColorCode, fore_color: ColorCode) -> Result<()> {
        self.default_back_color = back_color;
        self.back_color = back_color;
        self.default_fore_color = fore_color;
        self.fore_color = fore_color;

        self.cursor_x = 0;
        self.cursor_y = 2;

        self.fill(self.back_color)?;

        for (i, color) in GLOBAL_THEME.sample_rect_colors.iter().enumerate() {
            let xy = (i * 20, 0);
            let wh = (20, 20);
            self.draw_rect(xy, wh, *color)?;
        }

        Ok(())
    }

    fn set_target_layer_id(&mut self, layer_id: &LayerId) -> Result<()> {
        self.target_layer_id = Some(layer_id.clone());

        // update
        self.init(self.back_color, self.fore_color)
    }

    fn set_back_color(&mut self, back_color: ColorCode) {
        self.back_color = back_color;
    }

    fn reset_back_color(&mut self) {
        self.back_color = self.default_back_color;
    }

    fn set_fore_color(&mut self, fore_color: ColorCode) {
        self.fore_color = fore_color;
    }

    fn reset_fore_color(&mut self) {
        self.fore_color = self.default_fore_color;
    }

    fn write_char(&mut self, c: char) -> Result<()> {
        let (f_w, f_h) = FONT.get_wh();

        match c {
            '\n' => return self.new_line(),
            '\t' => return self.tab(),
            '\x08' | '\x7f' => return self.backspace(),
            _ => (),
        }

        match self.ansi_escape_stream.push(c) {
            Ok(Some(e)) => match e {
                // unprintable char
                AnsiEvent::AnsiControlChar(_) => {
                    return Ok(());
                }
                AnsiEvent::CsiSequence(seq) => {
                    match seq {
                        CsiSequence::CursorUp(n) => {
                            self.cursor_y = self.cursor_y.saturating_sub(n as usize);
                        }
                        CsiSequence::CursorDown(n) => {
                            let (_, cursor_max_y) = self.cursor_max()?;
                            self.cursor_y = (self.cursor_y + n as usize).min(cursor_max_y);
                        }
                        CsiSequence::CursorRight(n) => {
                            let (cursor_max_x, _) = self.cursor_max()?;
                            self.cursor_x = (self.cursor_x + n as usize).min(cursor_max_x);
                        }
                        CsiSequence::CursorLeft(n) => {
                            self.cursor_x = self.cursor_x.saturating_sub(n as usize);
                        }
                        CsiSequence::CursorNextLineHead(n) => {
                            let (_, cursor_max_y) = self.cursor_max()?;
                            self.cursor_x = 0;
                            self.cursor_y = (self.cursor_y + n as usize).min(cursor_max_y);
                        }
                        CsiSequence::CursorPrevLineHead(n) => {
                            self.cursor_x = 0;
                            self.cursor_y = self.cursor_y.saturating_sub(n as usize);
                        }
                        CsiSequence::CursorColumn(n) => {
                            if n > 0 {
                                let (cursor_max_x, _) = self.cursor_max()?;
                                self.cursor_x = ((n - 1) as usize).min(cursor_max_x);
                            }
                        }
                        CsiSequence::CursorPosition { row, col } => {
                            if row > 0 && col > 0 {
                                let (cursor_max_x, cursor_max_y) = self.cursor_max()?;
                                self.cursor_x = ((col - 1) as usize).min(cursor_max_x);
                                self.cursor_y = ((row - 1) as usize).min(cursor_max_y);
                            }
                        }
                        CsiSequence::ScrollUp(n) => {
                            for _ in 0..n {
                                self.scroll()?;
                            }
                        }
                        CsiSequence::ScrollDown(_) => {
                            unimplemented!()
                        }
                        CsiSequence::ClearScreenAfterCursor => {
                            let (s_w, s_h) = self.screen_wh()?;
                            let xy = (self.cursor_x * f_w, self.cursor_y * f_h);
                            let wh = (s_w - xy.0, s_h - xy.1);
                            self.draw_rect(xy, wh, self.back_color)?;
                        }
                        CsiSequence::ClearScreenBeforeCursor => {
                            let xy = (0, 0);
                            let wh = (self.cursor_x * f_w, self.cursor_y * f_h);
                            self.draw_rect(xy, wh, self.back_color)?;
                        }
                        CsiSequence::ClearScreenAll => {
                            self.fill(self.back_color)?;
                        }
                        CsiSequence::ClearRowAfterCursor => {
                            let (s_w, f_h) = self.screen_wh()?;
                            let xy = (self.cursor_x * f_w, self.cursor_y * f_h);
                            let wh = (s_w - xy.0, f_h);
                            self.draw_rect(xy, wh, self.back_color)?;
                        }
                        CsiSequence::ClearRowBeforeCursor => {
                            let xy = (0, self.cursor_y * f_h);
                            let wh = (self.cursor_x * f_w, f_h);
                            self.draw_rect(xy, wh, self.back_color)?;
                        }
                        CsiSequence::ClearRowAll => {
                            let xy = (0, self.cursor_y * f_h);
                            let wh = (self.screen_wh()?.0, f_h);
                            self.draw_rect(xy, wh, self.back_color)?;
                        }
                        CsiSequence::CharReset => {
                            self.reset_back_color();
                            self.reset_fore_color();
                            self.is_hidden = false;
                        }
                        CsiSequence::CharBold => {
                            unimplemented!()
                        }
                        CsiSequence::CharDim => {
                            unimplemented!()
                        }
                        CsiSequence::CharItalic => {
                            unimplemented!()
                        }
                        CsiSequence::CharUnderline => {
                            unimplemented!()
                        }
                        CsiSequence::CharBlink => {
                            unimplemented!()
                        }
                        CsiSequence::CharBlinkFast => {
                            unimplemented!()
                        }
                        CsiSequence::CharReverseColor => {
                            let tmp = self.fore_color;
                            self.set_fore_color(self.back_color);
                            self.set_back_color(tmp);
                        }
                        CsiSequence::CharHidden => {
                            self.is_hidden = true;
                        }
                        CsiSequence::CharCancel => {
                            unimplemented!()
                        }
                        CsiSequence::ForeColorBlack => {
                            self.set_fore_color(ColorCode::BLACK);
                        }
                        CsiSequence::ForeColorRed => {
                            self.set_fore_color(ColorCode::RED);
                        }
                        CsiSequence::ForeColorGreen => {
                            self.set_fore_color(ColorCode::GREEN);
                        }
                        CsiSequence::ForeColorYellow => {
                            self.set_fore_color(ColorCode::YELLOW);
                        }
                        CsiSequence::ForeColorBlue => {
                            self.set_fore_color(ColorCode::BLUE);
                        }
                        CsiSequence::ForeColorMagenta => {
                            self.set_fore_color(ColorCode::MAGENTA);
                        }
                        CsiSequence::ForeColorCyan => {
                            self.set_fore_color(ColorCode::CYAN);
                        }
                        CsiSequence::ForeColorWhite => {
                            self.set_fore_color(ColorCode::WHITE);
                        }
                        CsiSequence::BackColorBlack => {
                            self.set_back_color(ColorCode::BLACK);
                        }
                        CsiSequence::BackColorRed => {
                            self.set_back_color(ColorCode::RED);
                        }
                        CsiSequence::BackColorGreen => {
                            self.set_back_color(ColorCode::GREEN);
                        }
                        CsiSequence::BackColorYellow => {
                            self.set_back_color(ColorCode::YELLOW);
                        }
                        CsiSequence::BackColorBlue => {
                            self.set_back_color(ColorCode::BLUE);
                        }
                        CsiSequence::BackColorMagenta => {
                            self.set_back_color(ColorCode::MAGENTA);
                        }
                        CsiSequence::BackColorCyan => {
                            self.set_back_color(ColorCode::CYAN);
                        }
                        CsiSequence::BackColorWhite => {
                            self.set_back_color(ColorCode::WHITE);
                        }
                    }

                    return Ok(());
                }
            },
            Ok(None) => (),
            Err(_) => {
                self.ansi_escape_stream.reset();
            }
        }

        if !self.is_hidden {
            let xy = (self.cursor_x * f_w, self.cursor_y * f_h);
            self.draw_font(xy, c, self.fore_color, self.back_color)?;
            self.inc_cursor()?;
        }

        Ok(())
    }

    fn write_str(&mut self, s: &str) -> Result<()> {
        for c in s.chars() {
            self.write_char(c)?;
        }

        Ok(())
    }

    fn inc_cursor(&mut self) -> Result<()> {
        let (cursor_max_x, cursor_max_y) = self.cursor_max()?;

        self.cursor_x += 1;

        if self.cursor_x > cursor_max_x {
            self.cursor_x = 0;
            self.cursor_y += 1;
        }

        if self.cursor_y > cursor_max_y {
            self.scroll()?;
            self.cursor_x = 0;
            self.cursor_y = cursor_max_y;
        }

        Ok(())
    }

    fn dec_cursor(&mut self) -> Result<()> {
        let (cursor_max_x, _) = self.cursor_max()?;

        if self.cursor_x == 0 {
            if self.cursor_y > 0 {
                self.cursor_x = cursor_max_x;
                self.cursor_y -= 1;
            }
        } else {
            self.cursor_x -= 1;
        }

        Ok(())
    }

    fn tab(&mut self) -> Result<()> {
        for _ in 0..4 {
            self.write_char(' ')?;
        }

        Ok(())
    }

    fn new_line(&mut self) -> Result<()> {
        let (_, cursor_max_y) = self.cursor_max()?;

        self.cursor_x = 0;
        self.cursor_y += 1;

        if self.cursor_y > cursor_max_y {
            self.scroll()?;
            self.cursor_y = cursor_max_y;
        }

        Ok(())
    }

    fn scroll(&self) -> Result<()> {
        let (_, f_h) = FONT.get_wh();
        let (s_w, s_h) = self.screen_wh()?;
        // copy
        self.copy_rect((0, f_h), (0, 0), (s_w, s_h - f_h))?;

        // fill last line
        self.draw_rect((0, s_h - f_h), (s_w, f_h), self.back_color)?;

        Ok(())
    }

    fn fill(&self, color: ColorCode) -> Result<()> {
        if let Some(layer_id) = &self.target_layer_id {
            multi_layer::draw_layer(layer_id, |l| l.fill(color))?;
        } else {
            frame_buf::fill(color)?;
        }

        Ok(())
    }

    fn draw_rect(&self, xy: (usize, usize), wh: (usize, usize), color: ColorCode) -> Result<()> {
        if let Some(layer_id) = &self.target_layer_id {
            multi_layer::draw_layer(layer_id, |l| l.draw_rect(xy, wh, color))?;
        } else {
            frame_buf::draw_rect(xy, wh, color)?;
        }

        Ok(())
    }

    fn copy_rect(
        &self,
        src_xy: (usize, usize),
        dst_xy: (usize, usize),
        wh: (usize, usize),
    ) -> Result<()> {
        if let Some(layer_id) = &self.target_layer_id {
            multi_layer::draw_layer(layer_id, |l| l.copy_rect(src_xy, dst_xy, wh))?;
        } else {
            frame_buf::copy_rect(src_xy, dst_xy, wh)?;
        }

        Ok(())
    }

    fn draw_font(
        &self,
        xy: (usize, usize),
        c: char,
        fore_color: ColorCode,
        back_color: ColorCode,
    ) -> Result<()> {
        if let Some(layer_id) = &self.target_layer_id {
            multi_layer::draw_layer(layer_id, |l| l.draw_char(xy, c, fore_color, back_color))?;
        } else {
            frame_buf::draw_char(xy, c, fore_color, back_color)?;
        }

        Ok(())
    }

    fn backspace(&mut self) -> Result<()> {
        let (f_w, f_h) = FONT.get_wh();

        self.dec_cursor()?;
        self.draw_rect(
            (self.cursor_x * f_w, self.cursor_y * f_h),
            (f_w, f_h),
            self.back_color,
        )?;

        Ok(())
    }
}

impl fmt::Write for FrameBufferConsole {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _ = self.write_str(s);
        Ok(())
    }
}

pub fn init(back_color: ColorCode, fore_color: ColorCode) -> Result<()> {
    unsafe { FRAME_BUF_CONSOLE.try_lock() }?.init(back_color, fore_color)
}

pub fn set_target_layer_id(layer_id: &LayerId) -> Result<()> {
    unsafe { FRAME_BUF_CONSOLE.try_lock() }?.set_target_layer_id(layer_id)
}

pub fn set_fore_color(fore_color: ColorCode) -> Result<()> {
    unsafe { FRAME_BUF_CONSOLE.try_lock() }?.set_fore_color(fore_color);
    Ok(())
}

pub fn reset_fore_color() -> Result<()> {
    unsafe { FRAME_BUF_CONSOLE.try_lock() }?.reset_fore_color();
    Ok(())
}

pub fn write_char(c: char) -> Result<()> {
    let _ = unsafe { FRAME_BUF_CONSOLE.try_lock() }?.write_char(c);
    Ok(())
}
