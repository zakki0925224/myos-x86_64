const MAX_PARAMS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsiParams {
    buf: [u32; MAX_PARAMS],
    len: usize,
}

impl CsiParams {
    const fn new() -> Self {
        Self {
            buf: [0; MAX_PARAMS],
            len: 0,
        }
    }

    pub fn get(&self, i: usize) -> u32 {
        if i < self.len {
            self.buf[i]
        } else {
            0
        }
    }

    fn get_or(&self, i: usize, default: u32) -> u32 {
        match self.get(i) {
            0 => default,
            n => n,
        }
    }

    pub fn sgr(&self) -> SgrIter<'_> {
        SgrIter { params: self, i: 0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsiSequence {
    CursorUp(u32),
    CursorDown(u32),
    CursorRight(u32),
    CursorLeft(u32),
    CursorNextLineHead(u32),
    CursorPrevLineHead(u32),
    CursorColumn(u32),
    CursorPosition { row: u32, col: u32 },
    ScrollUp(u32),
    ScrollDown(u32),
    ClearScreenAfterCursor,
    ClearScreenBeforeCursor,
    ClearScreenAll,
    ClearRowAfterCursor,
    ClearRowBeforeCursor,
    ClearRowAll,
    Sgr(CsiParams),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiColor {
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SgrAttr {
    Reset,
    Reverse,
    Hidden,
    Fore(AnsiColor),
    Back(AnsiColor),
    DefaultFore,
    DefaultBack,
    Unsupported(u32),
}

pub struct SgrIter<'a> {
    params: &'a CsiParams,
    i: usize,
}

impl SgrIter<'_> {
    fn extended_color(&mut self) -> Option<AnsiColor> {
        let p = self.params;
        match p.get(self.i) {
            5 => {
                let n = p.get(self.i + 1);
                self.i += 2;
                Some(AnsiColor::Indexed(n.min(255) as u8))
            }
            2 => {
                let (r, g, b) = (p.get(self.i + 1), p.get(self.i + 2), p.get(self.i + 3));
                self.i += 4;
                Some(AnsiColor::Rgb(
                    r.min(255) as u8,
                    g.min(255) as u8,
                    b.min(255) as u8,
                ))
            }
            _ => None,
        }
    }
}

impl Iterator for SgrIter<'_> {
    type Item = SgrAttr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.params.len == 0 {
            if self.i == 0 {
                self.i = 1;
                return Some(SgrAttr::Reset); // "ESC[m" == "ESC[0m"
            }
            return None;
        }

        if self.i >= self.params.len {
            return None;
        }

        let code = self.params.buf[self.i];
        self.i += 1;

        let attr = match code {
            0 => SgrAttr::Reset,
            7 => SgrAttr::Reverse,
            8 => SgrAttr::Hidden,
            30..=37 => SgrAttr::Fore(AnsiColor::Indexed((code - 30) as u8)),
            40..=47 => SgrAttr::Back(AnsiColor::Indexed((code - 40) as u8)),
            90..=97 => SgrAttr::Fore(AnsiColor::Indexed((code - 90 + 8) as u8)),
            100..=107 => SgrAttr::Back(AnsiColor::Indexed((code - 100 + 8) as u8)),
            38 => self
                .extended_color()
                .map_or(SgrAttr::Unsupported(code), SgrAttr::Fore),
            48 => self
                .extended_color()
                .map_or(SgrAttr::Unsupported(code), SgrAttr::Back),
            39 => SgrAttr::DefaultFore,
            49 => SgrAttr::DefaultBack,
            _ => SgrAttr::Unsupported(code),
        };
        Some(attr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiEvent {
    Print,
    Consumed,
    Csi(CsiSequence),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscState {
    Normal,
    Esc,
    Csi,
}

pub struct AnsiEscapeStream {
    state: EscState,
    params: CsiParams,
    ignore: bool,
}

impl AnsiEscapeStream {
    pub const fn new() -> Self {
        Self {
            state: EscState::Normal,
            params: CsiParams::new(),
            ignore: false,
        }
    }

    pub fn push(&mut self, c: char) -> AnsiEvent {
        match self.state {
            EscState::Normal => match c {
                '\x1b' => {
                    self.state = EscState::Esc;
                    AnsiEvent::Consumed
                }
                '\0'..='\x1f' | '\x7f' => AnsiEvent::Consumed,
                _ => AnsiEvent::Print,
            },
            EscState::Esc => {
                match c {
                    '[' => {
                        self.state = EscState::Csi;
                        self.params = CsiParams::new();
                        self.ignore = false;
                    }
                    '\x1b' => (),
                    _ => self.state = EscState::Normal, // 2-byte ESC sequences are ignored
                }
                AnsiEvent::Consumed
            }
            EscState::Csi => self.push_csi(c),
        }
    }

    fn push_csi(&mut self, c: char) -> AnsiEvent {
        let p = &mut self.params;

        match c {
            '0'..='9' => {
                if p.len == 0 {
                    p.len = 1;
                }
                let v = &mut p.buf[p.len - 1];
                *v = v.saturating_mul(10).saturating_add(c as u32 - '0' as u32);
            }
            ';' => {
                if p.len == 0 {
                    p.len = 1;
                }
                if p.len < MAX_PARAMS {
                    p.len += 1;
                } else {
                    self.ignore = true;
                }
            }
            ':' | '<'..='?' | ' '..='/' => self.ignore = true, // sub-params, private markers, intermediates
            '\x1b' => self.state = EscState::Esc,
            '@'..='~' => {
                self.state = EscState::Normal;
                if !self.ignore {
                    if let Some(seq) = self.dispatch(c) {
                        return AnsiEvent::Csi(seq);
                    }
                }
            }
            '\0'..='\x1f' => (),
            _ => self.state = EscState::Normal,
        }

        AnsiEvent::Consumed
    }

    fn dispatch(&self, final_byte: char) -> Option<CsiSequence> {
        let p = &self.params;
        let n = p.get_or(0, 1);

        let seq = match final_byte {
            'A' => CsiSequence::CursorUp(n),
            'B' => CsiSequence::CursorDown(n),
            'C' => CsiSequence::CursorRight(n),
            'D' => CsiSequence::CursorLeft(n),
            'E' => CsiSequence::CursorNextLineHead(n),
            'F' => CsiSequence::CursorPrevLineHead(n),
            'G' => CsiSequence::CursorColumn(n),
            'H' | 'f' => CsiSequence::CursorPosition {
                row: p.get_or(0, 1),
                col: p.get_or(1, 1),
            },
            'J' => match p.get(0) {
                0 => CsiSequence::ClearScreenAfterCursor,
                1 => CsiSequence::ClearScreenBeforeCursor,
                2 | 3 => CsiSequence::ClearScreenAll,
                _ => return None,
            },
            'K' => match p.get(0) {
                0 => CsiSequence::ClearRowAfterCursor,
                1 => CsiSequence::ClearRowBeforeCursor,
                2 => CsiSequence::ClearRowAll,
                _ => return None,
            },
            'S' => CsiSequence::ScrollUp(n),
            'T' => CsiSequence::ScrollDown(n),
            'm' => CsiSequence::Sgr(*p),
            _ => return None,
        };
        Some(seq)
    }
}

#[cfg(test)]
fn push_str(stream: &mut AnsiEscapeStream, s: &str) -> AnsiEvent {
    let mut last = AnsiEvent::Consumed;
    for c in s.chars() {
        last = stream.push(c);
    }
    last
}

#[test_case]
fn test_new() {
    let stream = AnsiEscapeStream::new();
    assert_eq!(stream.state, EscState::Normal);
}

#[test_case]
fn test_simple_csi() {
    let mut stream = AnsiEscapeStream::new();
    assert_eq!(stream.push('a'), AnsiEvent::Print);
    assert_eq!(stream.push('\x1b'), AnsiEvent::Consumed);
    assert_eq!(stream.push('['), AnsiEvent::Consumed);
    assert_eq!(stream.push('1'), AnsiEvent::Consumed);
    assert_eq!(
        stream.push('A'),
        AnsiEvent::Csi(CsiSequence::CursorUp(1))
    );

    assert_eq!(
        push_str(&mut stream, "\x1b[24;51H"),
        AnsiEvent::Csi(CsiSequence::CursorPosition { row: 24, col: 51 })
    );
    assert_eq!(
        push_str(&mut stream, "\x1b[H"),
        AnsiEvent::Csi(CsiSequence::CursorPosition { row: 1, col: 1 })
    );
    assert_eq!(stream.push('a'), AnsiEvent::Print);
}

#[test_case]
fn test_unknown_and_private() {
    let mut stream = AnsiEscapeStream::new();
    assert_eq!(push_str(&mut stream, "\x1b[3~"), AnsiEvent::Consumed);
    assert_eq!(stream.push('a'), AnsiEvent::Print);
    assert_eq!(push_str(&mut stream, "\x1b[?25l"), AnsiEvent::Consumed);
    assert_eq!(stream.push('a'), AnsiEvent::Print);
}

#[test_case]
fn test_sgr() {
    let mut stream = AnsiEscapeStream::new();
    let AnsiEvent::Csi(CsiSequence::Sgr(p)) = push_str(&mut stream, "\x1b[1;31;38;2;10;20;30;48;5;200m")
    else {
        panic!();
    };

    let mut it = p.sgr();
    assert_eq!(it.next(), Some(SgrAttr::Unsupported(1)));
    assert_eq!(it.next(), Some(SgrAttr::Fore(AnsiColor::Indexed(1))));
    assert_eq!(it.next(), Some(SgrAttr::Fore(AnsiColor::Rgb(10, 20, 30))));
    assert_eq!(it.next(), Some(SgrAttr::Back(AnsiColor::Indexed(200))));
    assert_eq!(it.next(), None);

    let AnsiEvent::Csi(CsiSequence::Sgr(p)) = push_str(&mut stream, "\x1b[m") else {
        panic!();
    };
    let mut it = p.sgr();
    assert_eq!(it.next(), Some(SgrAttr::Reset));
    assert_eq!(it.next(), None);
}
