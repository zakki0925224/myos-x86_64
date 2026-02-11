use crate::display_item::DisplayItem;
use crate::renderer::layout::computed_style::{Color, FontSize, TextDecoration};
use core::convert::Infallible;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::*},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::*,
    text::{Baseline, Text},
};

pub struct Framebuffer {
    fb: *mut u8,
    width: usize,
    height: usize,
}

impl Framebuffer {
    pub fn new(fb: *mut u8, width: usize, height: usize) -> Self {
        Self { fb, width, height }
    }
}

impl Dimensions for Framebuffer {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(
            Point::zero(),
            Size::new(self.width as u32, self.height as u32),
        )
    }
}

impl DrawTarget for Framebuffer {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> core::result::Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            let x = coord.x as usize;
            let y = coord.y as usize;
            if x < self.width && y < self.height {
                let offset = (y * self.width + x) * 4;
                unsafe {
                    let pixel_ptr = self.fb.add(offset);
                    *pixel_ptr = color.b();
                    *pixel_ptr.add(1) = color.g();
                    *pixel_ptr.add(2) = color.r();
                    *pixel_ptr.add(3) = 0xff;
                }
            }
        }

        Ok(())
    }
}

fn color_to_rgb888(color: &Color) -> Rgb888 {
    let code = color.code_u32();
    Rgb888::new(
        ((code >> 16) & 0xff) as u8,
        ((code >> 8) & 0xff) as u8,
        (code & 0xff) as u8,
    )
}

fn font_for_size(font_size: FontSize) -> &'static embedded_graphics::mono_font::MonoFont<'static> {
    match font_size {
        FontSize::Medium => &FONT_8X13,
        FontSize::XLarge => &FONT_9X18,
        FontSize::XXLarge => &FONT_10X20,
    }
}

/// Clear the entire framebuffer with a background color.
pub fn clear(fb: &mut Framebuffer, color: Rgb888) {
    let w = fb.width as u32;
    let h = fb.height as u32;
    let _ = Rectangle::new(Point::zero(), Size::new(w, h))
        .into_styled(PrimitiveStyleBuilder::new().fill_color(color).build())
        .draw(fb);
}

/// Render all display items onto the framebuffer.
pub fn paint_display_items(fb: &mut Framebuffer, items: &[DisplayItem]) {
    clear(fb, Rgb888::WHITE);

    for item in items {
        match item {
            DisplayItem::Rect {
                style,
                layout_point,
                layout_size,
            } => {
                let bg = color_to_rgb888(&style.background_color());
                let _ = Rectangle::new(
                    Point::new(layout_point.x() as i32, layout_point.y() as i32),
                    Size::new(layout_size.width() as u32, layout_size.height() as u32),
                )
                .into_styled(PrimitiveStyleBuilder::new().fill_color(bg).build())
                .draw(fb);
            }
            DisplayItem::Text {
                text,
                style,
                layout_point,
            } => {
                let fg = color_to_rgb888(&style.color());
                let font = font_for_size(style.font_size());
                let text_style = MonoTextStyle::new(font, fg);

                let pos = Point::new(layout_point.x() as i32, layout_point.y() as i32);
                let _ = Text::with_baseline(text.as_str(), pos, text_style, Baseline::Top).draw(fb);

                // underline decoration
                if style.text_decoration() == TextDecoration::Underline {
                    let text_width = text.len() as u32 * font.character_size.width;
                    let underline_y =
                        layout_point.y() as i32 + font.character_size.height as i32 + 1;
                    let _ = Line::new(
                        Point::new(layout_point.x() as i32, underline_y),
                        Point::new(layout_point.x() as i32 + text_width as i32, underline_y),
                    )
                    .into_styled(PrimitiveStyle::with_stroke(fg, 1))
                    .draw(fb);
                }
            }
        }
    }
}
