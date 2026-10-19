//! Text, through cosmic-text.
//!
//! This is the part of the candidate that has no equivalent in the other one: GTK gets
//! shaping, font fallback and alignment from Pango without being asked. Here a font
//! system is held open for the life of the process, because building it means reading
//! the fontconfig database, and doing that on the way to the screen would put tens of
//! milliseconds into every show.

use cosmic_text::{Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use glimt_shell::theme::Rgb;
use tiny_skia::{Pixmap, PremultipliedColorU8};

/// The box a line of text is placed in. Vertical centring inside it is what lets a
/// caller put text in a table row without computing a baseline.
#[derive(Clone, Copy)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Area {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// A box exactly one line tall, which places the text at `y`.
    pub fn line(x: f32, y: f32, width: f32, style: Style) -> Self {
        Self::new(x, y, width, style.line_height())
    }
}

/// Multiple of the font size used as the line box. Both candidates end up near this,
/// and fixing it keeps rows the same height in the two screenshots.
const LINE_HEIGHT: f32 = 1.3;

pub struct Text {
    fonts: FontSystem,
    cache: SwashCache,
}

#[derive(Clone, Copy)]
pub struct Style {
    pub size: f32,
    pub weight: cosmic_text::Weight,
    pub color: Rgb,
    pub align: Align,
}

impl Style {
    pub fn new(size: f32, color: Rgb) -> Self {
        Self {
            size,
            weight: cosmic_text::Weight::NORMAL,
            color,
            align: Align::Left,
        }
    }

    pub fn bold(mut self) -> Self {
        self.weight = cosmic_text::Weight::BOLD;
        self
    }

    pub fn right(mut self) -> Self {
        self.align = Align::Right;
        self
    }

    pub fn centered(mut self) -> Self {
        self.align = Align::Center;
        self
    }

    pub fn line_height(&self) -> f32 {
        self.size * LINE_HEIGHT
    }
}

impl Text {
    pub fn new() -> Self {
        Self {
            fonts: FontSystem::new(),
            cache: SwashCache::new(),
        }
    }

    /// Draw one line, aligned horizontally in `area` and centred vertically in it.
    ///
    /// The scale is applied to the sizes rather than to the result, so glyphs are
    /// hinted and rasterised at the size they are shown at instead of being drawn once
    /// and stretched.
    pub fn draw(&mut self, pixmap: &mut Pixmap, text: &str, area: Area, style: Style, scale: f32) {
        if text.is_empty() {
            return;
        }

        let size = style.size * scale;
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size * LINE_HEIGHT));
        buffer.set_size(Some(area.width * scale), Some(size * LINE_HEIGHT));

        let attrs = Attrs {
            family: Family::SansSerif,
            weight: style.weight,
            ..Attrs::new()
        };
        buffer.set_text(text, &attrs, Shaping::Advanced, Some(style.align));

        let color = Color::rgb(style.color.0, style.color.1, style.color.2);
        let top = area.y + (area.height - style.line_height()) / 2.0;
        let (origin_x, origin_y) = ((area.x * scale) as i32, (top * scale) as i32);

        buffer.draw(
            &mut self.fonts,
            &mut self.cache,
            color,
            |glyph_x, glyph_y, width, height, color| {
                blend(
                    pixmap,
                    origin_x + glyph_x,
                    origin_y + glyph_y,
                    width,
                    height,
                    color,
                );
            },
        );
    }
}

/// Source-over a coverage span onto the premultiplied pixmap.
///
/// cosmic-text hands back one span per run of equal coverage, so this is called a few
/// thousand times per full redraw and is the inner loop of the whole candidate.
fn blend(pixmap: &mut Pixmap, x: i32, y: i32, width: u32, height: u32, color: Color) {
    let alpha = color.a() as u32;
    if alpha == 0 {
        return;
    }

    let (pixmap_width, pixmap_height) = (pixmap.width() as i32, pixmap.height() as i32);
    let data = pixmap.pixels_mut();

    let inverse = 255 - alpha;
    let src = [
        color.r() as u32 * alpha / 255,
        color.g() as u32 * alpha / 255,
        color.b() as u32 * alpha / 255,
    ];

    for row in 0..height as i32 {
        let py = y + row;
        if py < 0 || py >= pixmap_height {
            continue;
        }

        for column in 0..width as i32 {
            let px = x + column;
            if px < 0 || px >= pixmap_width {
                continue;
            }

            let index = (py * pixmap_width + px) as usize;
            let dst = data[index];

            let mixed = PremultipliedColorU8::from_rgba(
                (src[0] + dst.red() as u32 * inverse / 255) as u8,
                (src[1] + dst.green() as u32 * inverse / 255) as u8,
                (src[2] + dst.blue() as u32 * inverse / 255) as u8,
                (alpha + dst.alpha() as u32 * inverse / 255) as u8,
            );

            data[index] = mixed.unwrap_or(dst);
        }
    }
}
