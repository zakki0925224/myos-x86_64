use crate::error::Result;
use common::geometry::Size;

//PSF font v2
const FONT_BIN: &[u8] = include_bytes!("../../../third-party/font.psf");
const FONT_MAGIC_NUM: u32 = 0x864ab572;
const UNICODE_TABLE_SEPARATOR: u8 = 0xff;

pub static FONT: PsfFont = PsfFont::new();

pub struct PsfFont {
    binary_len: usize,
    wh: Size,
    glyphs_len: usize,
    glyph_size: usize,
    has_unicode_table: bool,
    header_size: usize,
    unicode_table_offset: usize,
}

impl PsfFont {
    const fn new() -> Self {
        const fn get_magic_num() -> u32 {
            (FONT_BIN[3] as u32) << 24
                | (FONT_BIN[2] as u32) << 16
                | (FONT_BIN[1] as u32) << 8
                | FONT_BIN[0] as u32
        }

        const fn get_pixel_height() -> u32 {
            (FONT_BIN[27] as u32) << 24
                | (FONT_BIN[26] as u32) << 16
                | (FONT_BIN[25] as u32) << 8
                | FONT_BIN[24] as u32
        }

        const fn get_pixel_width() -> u32 {
            (FONT_BIN[31] as u32) << 24
                | (FONT_BIN[30] as u32) << 16
                | (FONT_BIN[29] as u32) << 8
                | FONT_BIN[28] as u32
        }

        const fn get_glyphs_len() -> u32 {
            (FONT_BIN[19] as u32) << 24
                | (FONT_BIN[18] as u32) << 16
                | (FONT_BIN[17] as u32) << 8
                | FONT_BIN[16] as u32
        }

        const fn get_glyph_size() -> u32 {
            (FONT_BIN[23] as u32) << 24
                | (FONT_BIN[22] as u32) << 16
                | (FONT_BIN[21] as u32) << 8
                | FONT_BIN[20] as u32
        }

        const fn has_unicode_table() -> bool {
            let flags = (FONT_BIN[15] as u32) << 24
                | (FONT_BIN[14] as u32) << 16
                | (FONT_BIN[13] as u32) << 8
                | FONT_BIN[12] as u32;

            flags == 1
        }

        const fn get_header_size() -> u32 {
            (FONT_BIN[11] as u32) << 24
                | (FONT_BIN[10] as u32) << 16
                | (FONT_BIN[9] as u32) << 8
                | FONT_BIN[8] as u32
        }

        if get_magic_num() != FONT_MAGIC_NUM {
            panic!("Invalid font binary");
        }

        let binary_len = FONT_BIN.len();
        let height = get_pixel_height() as usize;
        let width = get_pixel_width() as usize;
        let glyphs_len = get_glyphs_len() as usize;
        let glyph_size = get_glyph_size() as usize;
        let has_unicode_table = has_unicode_table();
        let header_size = get_header_size() as usize;
        let unicode_table_offset = header_size + glyph_size * glyphs_len;

        if height > 16 || width > 8 {
            panic!("Unsupported font size");
        }

        Self {
            binary_len,
            wh: Size::new(width, height),
            glyphs_len,
            glyph_size,
            has_unicode_table,
            header_size,
            unicode_table_offset,
        }
    }

    pub fn get_wh(&self) -> (usize, usize) {
        self.wh.wh()
    }

    // ascii char only
    fn unicode_char_to_glyph_index(&self, c: char) -> usize {
        if !self.has_unicode_table {
            return c as usize;
        }

        let code_point = c as u8;
        let mut index = 0;

        for i in self.unicode_table_offset..self.binary_len {
            if code_point == FONT_BIN[i] {
                break;
            }

            if FONT_BIN[i] == UNICODE_TABLE_SEPARATOR {
                index += 1;
            }
        }

        index
    }

    pub fn get_glyph(&self, c: char) -> Result<&'static [u8]> {
        let index = self.unicode_char_to_glyph_index(c);

        if index > self.glyphs_len {
            return Err("Invalid glyph index".into());
        }

        let offset = self.header_size + self.glyph_size * index;
        Ok(&FONT_BIN[offset..offset + self.glyph_size])
    }
}
