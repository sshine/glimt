//! Colours, sizes and font sizes, shared so the two spikes draw the same picture.
//!
//! The comparison is only worth something if what differs between the candidates is the
//! toolkit. Layout is left to each of them, because how a toolkit expresses layout is
//! one of the things being compared, but the numbers it lays out from are fixed here. A
//! difference in the drawn result is then a difference in the renderer.
//!
//! Colours come in two palettes rather than as constants: the session's own light or
//! dark preference picks one, and both candidates paint from the same one. Every role
//! that used to be a single constant is a field, so a colour added later cannot be
//! added to one palette and forgotten in the other.
//!
//! Lengths are logical pixels, before the output's scale is applied. The window is
//! maximized, so its width and height come from the compositor and the layout is
//! computed against them; the constants below are minimums and fixed parts.

use crate::appearance::Scheme;

/// Straight 8-bit sRGB, which is what both a GTK CSS colour and a tiny-skia paint want.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// `#rrggbb`, for the GTK style sheet.
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// Every colour either frontend paints with, in one of two variants.
///
/// The light values are the ones the dashboard was drawn with before there was a dark
/// one. The dark values keep the same relationships: ink readable on every surface,
/// today still red and the selection still blue, the highlight a wash rather than a
/// fill, and the past still quieter than the future.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: Rgb,
    pub ink: Rgb,
    pub muted: Rgb,
    /// Weekends and public holidays, as the reference shades them.
    pub shade: Rgb,
    pub rule: Rgb,
    /// Today, and the outline the month columns put around the week on show.
    pub accent: Rgb,
    /// The day the keyboard is on. Blue rather than another red, so that moving the
    /// selection away from today leaves both visible and tells them apart.
    pub selection: Rgb,
    /// The days a duration covers. Laid over what is already drawn, so it has to be
    /// light enough to read text through and distinct from the grey of a weekend.
    pub highlight: Rgb,
    /// How much of the highlight shows through, out of 255.
    pub highlight_alpha: u8,

    pub past_fill: Rgb,
    pub past_ink: Rgb,
    pub today_ink: Rgb,
    pub future_fill: Rgb,
    pub future_border: Rgb,

    /// The calculator, between the week strip and the month grid.
    pub bar_fill: Rgb,
    /// An answer that belongs to an earlier keystroke, which is how an unfinished
    /// expression reports itself: by going quiet rather than by complaining.
    pub faded: Rgb,

    /// How dark the fullscreen backdrop is, out of 255. The backdrop is black in both
    /// palettes; it is heavier in the dark one so the card still separates from it.
    pub scrim_alpha: u8,
}

impl Palette {
    pub const LIGHT: Self = Self {
        background: Rgb(0xff, 0xff, 0xff),
        ink: Rgb(0x11, 0x11, 0x11),
        muted: Rgb(0x8a, 0x8a, 0x8a),
        shade: Rgb(0xd6, 0xd6, 0xd6),
        rule: Rgb(0xdf, 0xdf, 0xdf),
        accent: Rgb(0xc6, 0x28, 0x28),
        selection: Rgb(0x1d, 0x5f, 0xc4),
        highlight: Rgb(0x9a, 0xc0, 0xf0),
        highlight_alpha: 110,

        past_fill: Rgb(0xec, 0xec, 0xec),
        past_ink: Rgb(0x9a, 0x9a, 0x9a),
        today_ink: Rgb(0xff, 0xff, 0xff),
        future_fill: Rgb(0xff, 0xff, 0xff),
        future_border: Rgb(0xc9, 0xc9, 0xc9),

        bar_fill: Rgb(0xf4, 0xf4, 0xf6),
        faded: Rgb(0xa8, 0xa8, 0xa8),

        scrim_alpha: 89,
    };

    pub const DARK: Self = Self {
        background: Rgb(0x1b, 0x1b, 0x1d),
        ink: Rgb(0xec, 0xec, 0xed),
        muted: Rgb(0xa3, 0xa3, 0xa6),
        shade: Rgb(0x2b, 0x2b, 0x2f),
        rule: Rgb(0x3a, 0x3a, 0x3e),
        accent: Rgb(0xd2, 0x3f, 0x3f),
        selection: Rgb(0x7a, 0xa7, 0xff),
        highlight: Rgb(0x4a, 0x7d, 0xbd),
        highlight_alpha: 96,

        past_fill: Rgb(0x26, 0x26, 0x2a),
        past_ink: Rgb(0x7a, 0x7a, 0x80),
        today_ink: Rgb(0xff, 0xff, 0xff),
        future_fill: Rgb(0x1b, 0x1b, 0x1d),
        future_border: Rgb(0x45, 0x45, 0x4a),

        bar_fill: Rgb(0x2a, 0x2a, 0x2e),
        faded: Rgb(0x78, 0x78, 0x7e),

        scrim_alpha: 140,
    };

    pub fn for_scheme(scheme: Scheme) -> Self {
        match scheme {
            Scheme::Light => Self::LIGHT,
            Scheme::Dark => Self::DARK,
        }
    }
}

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
pub const BAR_RADIUS: f32 = 8.0;
pub const BAR_TEXT_SIZE: f32 = 21.0;
pub const BAR_NOTE_SIZE: f32 = 11.0;
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
