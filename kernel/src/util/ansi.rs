use crate::{error::Result, util::fifo::Fifo};
use alloc::string::String;

#[derive(Debug, Clone, PartialEq, Eq)]
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
    CharReset,
    CharBold,
    CharDim,
    CharItalic,
    CharUnderline,
    CharBlink,
    CharBlinkFast,
    CharReverseColor,
    CharHidden,
    CharCancel,
    ForeColorBlack,
    ForeColorRed,
    ForeColorGreen,
    ForeColorYellow,
    ForeColorBlue,
    ForeColorMagenta,
    ForeColorCyan,
    ForeColorWhite,
    BackColorBlack,
    BackColorRed,
    BackColorGreen,
    BackColorYellow,
    BackColorBlue,
    BackColorMagenta,
    BackColorCyan,
    BackColorWhite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnsiEvent {
    AnsiControlChar(char),
    CsiSequence(CsiSequence),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    Esc,
    Csi,
}

pub struct AnsiEscapeStream {
    state: State,
    buf: Fifo<char, 16>,
}

impl AnsiEscapeStream {
    pub const fn new() -> Self {
        Self {
            state: State::Normal,
            buf: Fifo::new('\0'),
        }
    }

    pub fn reset(&mut self) {
        self.state = State::Normal;
        self.buf.reset_ptr();
    }

    fn parse_csi_seq(&mut self) -> Option<AnsiEvent> {
        let mut seq = String::new();
        while let Ok(ch) = self.buf.dequeue() {
            seq.push(ch);
        }
        self.reset();

        let mut num_buf1 = String::new();
        let mut num_buf2 = String::new();
        for ch in seq.as_str().chars() {
            match ch {
                '0'..='9' => num_buf1.push(ch),
                'A' | 'B' | 'C' | 'D' | 'E' | 'F' | 'G' | 'S' | 'T' => {
                    let num = num_buf1.parse().unwrap_or(1);
                    let seq = match ch {
                        'A' => CsiSequence::CursorUp(num),
                        'B' => CsiSequence::CursorDown(num),
                        'C' => CsiSequence::CursorRight(num),
                        'D' => CsiSequence::CursorLeft(num),
                        'E' => CsiSequence::CursorNextLineHead(num),
                        'F' => CsiSequence::CursorPrevLineHead(num),
                        'G' => CsiSequence::CursorColumn(num),
                        'S' => CsiSequence::ScrollUp(num),
                        'T' => CsiSequence::ScrollDown(num),
                        _ => unreachable!(),
                    };
                    return Some(AnsiEvent::CsiSequence(seq));
                }
                'H' | 'f' => {
                    if let (Ok(num1), Ok(num2)) = (num_buf1.parse(), num_buf2.parse()) {
                        return Some(AnsiEvent::CsiSequence(CsiSequence::CursorPosition {
                            row: num2,
                            col: num1,
                        }));
                    }
                }
                'J' | 'K' => {
                    let seq = match ch {
                        'J' => {
                            if let Ok(num) = num_buf1.parse() {
                                match num {
                                    0 => CsiSequence::ClearScreenAfterCursor,
                                    1 => CsiSequence::ClearScreenBeforeCursor,
                                    2 => CsiSequence::ClearScreenAll,
                                    _ => continue,
                                }
                            } else {
                                CsiSequence::ClearScreenAfterCursor
                            }
                        }
                        'K' => {
                            if let Ok(num) = num_buf1.parse() {
                                match num {
                                    0 => CsiSequence::ClearRowAfterCursor,
                                    1 => CsiSequence::ClearRowBeforeCursor,
                                    2 => CsiSequence::ClearRowAll,
                                    _ => continue,
                                }
                            } else {
                                CsiSequence::ClearRowAfterCursor
                            }
                        }
                        _ => unreachable!(),
                    };
                    return Some(AnsiEvent::CsiSequence(seq));
                }
                'm' => {
                    let seq = if let Ok(num) = num_buf1.parse() {
                        match num {
                            0 => CsiSequence::CharReset,
                            1 => CsiSequence::CharBold,
                            2 => CsiSequence::CharDim,
                            3 => CsiSequence::CharItalic,
                            4 => CsiSequence::CharUnderline,
                            5 => CsiSequence::CharBlink,
                            6 => CsiSequence::CharBlinkFast,
                            7 => CsiSequence::CharReverseColor,
                            8 => CsiSequence::CharHidden,
                            9 => CsiSequence::CharCancel,
                            30 => CsiSequence::ForeColorBlack,
                            31 => CsiSequence::ForeColorRed,
                            32 => CsiSequence::ForeColorGreen,
                            33 => CsiSequence::ForeColorYellow,
                            34 => CsiSequence::ForeColorBlue,
                            35 => CsiSequence::ForeColorMagenta,
                            36 => CsiSequence::ForeColorCyan,
                            37 => CsiSequence::ForeColorWhite,
                            40 => CsiSequence::BackColorBlack,
                            41 => CsiSequence::BackColorRed,
                            42 => CsiSequence::BackColorGreen,
                            43 => CsiSequence::BackColorYellow,
                            44 => CsiSequence::BackColorBlue,
                            45 => CsiSequence::BackColorMagenta,
                            46 => CsiSequence::BackColorCyan,
                            47 => CsiSequence::BackColorWhite,
                            _ => continue,
                        }
                    } else {
                        CsiSequence::CharReset
                    };

                    return Some(AnsiEvent::CsiSequence(seq));
                }
                ';' => {
                    if !num_buf1.is_empty() {
                        num_buf2 = num_buf1.clone();
                        num_buf1.clear();
                    }
                }
                _ => (),
            }
        }

        None
    }

    pub fn push(&mut self, c: char) -> Result<Option<AnsiEvent>> {
        match self.state {
            State::Normal => {
                if c == '\x1b' {
                    self.state = State::Esc;
                    Ok(Some(AnsiEvent::AnsiControlChar(c)))
                } else {
                    Ok(None)
                }
            }
            State::Esc => {
                if c == '[' {
                    self.state = State::Csi;
                    self.buf.reset_ptr();
                    Ok(Some(AnsiEvent::AnsiControlChar(c)))
                } else {
                    self.state = State::Normal;
                    Ok(None)
                }
            }
            State::Csi => {
                if self.buf.enqueue(c).is_err() {
                    self.buf.reset_ptr();
                    self.buf.enqueue(c)?;
                }

                // end of CSI sequence
                if c.is_ascii_alphabetic() {
                    return Ok(self.parse_csi_seq());
                }

                Ok(Some(AnsiEvent::AnsiControlChar(c)))
            }
        }
    }
}

#[test_case]
fn test_new() {
    let stream = AnsiEscapeStream::new();
    assert_eq!(stream.state, State::Normal);
    assert_eq!(stream.buf.get_read_write_ptr(), (0, 0));
}

#[test_case]
fn test_simple_csi() {
    let mut stream = AnsiEscapeStream::new();
    assert_eq!(
        stream.push('\x1b').unwrap(),
        Some(AnsiEvent::AnsiControlChar('\x1b'))
    );
    assert_eq!(
        stream.push('[').unwrap(),
        Some(AnsiEvent::AnsiControlChar('['))
    );
    assert_eq!(
        stream.push('1').unwrap(),
        Some(AnsiEvent::AnsiControlChar('1'))
    );
    assert_eq!(
        stream.push('A').unwrap(),
        Some(AnsiEvent::CsiSequence(CsiSequence::CursorUp(1)))
    );

    stream.push('\x1b').unwrap();
    stream.push('[').unwrap();
    stream.push('2').unwrap();
    stream.push('4').unwrap();
    stream.push(';').unwrap();
    stream.push('5').unwrap();
    stream.push('1').unwrap();
    assert_eq!(
        stream.push('H').unwrap(),
        Some(AnsiEvent::CsiSequence(CsiSequence::CursorPosition {
            row: 24,
            col: 51
        }))
    )
}
