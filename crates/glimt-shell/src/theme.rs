//! Colours, sizes and font sizes, shared so the two spikes draw the same picture.
//!
//! The comparison is only worth something if what differs between the candidates is the
//! toolkit. Layout is left to each of them, because how a toolkit expresses layout is
//! one of the things being compared, but the numbers it lays out from are fixed here. A
//! difference in the drawn result is then a difference in the renderer.
//!
//! Lengths are logical pixels, before the output's scale is applied. The window is
//! maximized, so its width and height come from the compositor and the layout is
//! computed against them; the constants below are minimums and fixed parts.

/// Straight 8-bit sRGB, which is what both a GTK CSS colour and a tiny-skia paint want.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// `#rrggbb`, for the GTK style sheet.
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

pub const BACKGROUND: Rgb = Rgb(0xff, 0xff, 0xff);
pub const INK: Rgb = Rgb(0x11, 0x11, 0x11);
pub const MUTED: Rgb = Rgb(0x8a, 0x8a, 0x8a);
/// Weekends and public holidays, as the reference shades them.
pub const SHADE: Rgb = Rgb(0xd6, 0xd6, 0xd6);
pub const RULE: Rgb = Rgb(0xdf, 0xdf, 0xdf);
/// Today, and the outline the month columns put around the week on show.
pub const ACCENT: Rgb = Rgb(0xc6, 0x28, 0x28);

/// The day the keyboard is on. Blue rather than another red, so that moving the
/// selection away from today leaves both visible and tells them apart.
pub const SELECTION: Rgb = Rgb(0x1d, 0x5f, 0xc4);
/// The days a duration covers. Laid over what is already drawn, so it has to be light
/// enough to read text through and distinct from the grey of a weekend.
pub const HIGHLIGHT: Rgb = Rgb(0x9a, 0xc0, 0xf0);
/// How much of the highlight shows through, out of 255.
pub const HIGHLIGHT_ALPHA: u8 = 110;

pub const PAST_FILL: Rgb = Rgb(0xec, 0xec, 0xec);
pub const PAST_INK: Rgb = Rgb(0x9a, 0x9a, 0x9a);
pub const TODAY_INK: Rgb = Rgb(0xff, 0xff, 0xff);
pub const FUTURE_FILL: Rgb = BACKGROUND;
pub const FUTURE_BORDER: Rgb = Rgb(0xc9, 0xc9, 0xc9);

/// Resolved through fontconfig by both candidates, so Pango and cosmic-text pick the
/// same file rather than each applying its own idea of a default.
pub const FONT: &str = "sans-serif";

/// Only used when the compositor will not maximize the window.
pub const WINDOW_WIDTH: f32 = 1180.0;
pub const WINDOW_HEIGHT: f32 = 960.0;
pub const PAD: f32 = 32.0;

/// Beside the seven boxes rather than above them, which is what made room for the
/// calculator without taking any from the grid.
pub const WEEK_HEADING_SIZE: f32 = 64.0;

pub const SQUARE: f32 = 92.0;
pub const SQUARE_GAP: f32 = 10.0;
pub const SQUARE_RADIUS: f32 = 10.0;
pub const SQUARE_INITIAL_SIZE: f32 = 13.0;
pub const SQUARE_NUMBER_SIZE: f32 = 34.0;
/// Width of the ring drawn around the selected day.
pub const SELECTION_RING: f32 = 3.0;

/// The calculator, between the week strip and the month grid.
pub const BAR_HEIGHT: f32 = 52.0;
pub const BAR_FILL: Rgb = Rgb(0xf4, 0xf4, 0xf6);
pub const BAR_RADIUS: f32 = 8.0;
pub const BAR_TEXT_SIZE: f32 = 21.0;
pub const BAR_NOTE_SIZE: f32 = 11.0;
/// An answer that belongs to an earlier keystroke, which is how an unfinished
/// expression reports itself: by going quiet rather than by complaining.
pub const FADED: Rgb = Rgb(0xa8, 0xa8, 0xa8);
/// The prompt in front of the expression.
pub const PROMPT: &str = "=";
/// The caret, drawn after whatever has been typed.
pub const CURSOR: &str = "\u{2502}";
/// Half a blink. GTK's own default is a 1200 ms cycle; this is a little quicker.
pub const CURSOR_BLINK_MS: u64 = 530;

pub const MONTH_HEADING_SIZE: f32 = 15.0;
pub const MONTH_ROW_SIZE: f32 = 14.0;
pub const MONTH_ROW_HEIGHT: f32 = 20.0;
/// A column is as wide as the window allows, never narrower than this.
pub const MONTH_COLUMN_MIN: f32 = 186.0;
pub const HOLIDAY_SIZE: f32 = 10.0;
/// Inset of a month row from its column, which is what leaves the vertical rule visible
/// beside a shaded weekend.
pub const ROW_INSET: f32 = 3.0;

/// Gap between the three bands.
pub const SECTION_GAP: f32 = 20.0;

/// Width one month column gets, given the whole window.
pub fn column_width(window_width: f32, columns: usize) -> f32 {
    let columns = columns.max(1) as f32;
    let available = (window_width - 2.0 * PAD) / columns;

    available.max(MONTH_COLUMN_MIN)
}
