//! The dashboard, as arithmetic.
//!
//! Every rectangle and every text box is placed from the shared theme by hand. This is
//! the other half of what the comparison is about: the GTK candidate gets all of this
//! from boxes and a style sheet, and the difference in length between the two files is
//! part of the answer.
//!
//! Drawing is split in two because a keystroke must not cost a full repaint. The month
//! grid goes into a pixmap of its own and only changes when the six months do; a redraw
//! copies that and paints the week strip, the calculator and the marks over it. At the
//! size this window is maximized to, painting the grid costs around a tenth of a second
//! and copying it costs a few milliseconds, which is the difference between a calculator
//! that keeps up with typing and one that does not. GTK needs none of this, because it
//! already knows which widgets changed.

use glimt_cal::{Day, MonthColumn, Overview, Tense};
use glimt_shell::state::Bar;
use glimt_shell::theme::{self, Palette, Rgb};
use tiny_skia::{FillRule, LineCap, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::text::{Area, Style, Text};

/// Width of the text field holding a holiday name, before the week number.
const HOLIDAY_WIDTH: f32 = 92.0;
/// Height the month heading takes above the first row.
const HEADING_BAND: f32 = 26.0;

/// A rectangle of the window, in logical pixels.
///
/// Damage is tracked in these and converted to buffer pixels at the last moment, so the
/// arithmetic reads the same at any scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Region {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn intersects(&self, other: &Region) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }

    pub fn touches(&self, regions: &[Region]) -> bool {
        regions.iter().any(|region| self.intersects(region))
    }

    /// The same rectangle in buffer pixels, clamped to the buffer and rounded outwards
    /// so a half-covered pixel is still repainted.
    pub fn to_pixels(self, scale: f32, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let left = (self.x * scale).floor().max(0.0) as u32;
        let top = (self.y * scale).floor().max(0.0) as u32;
        let right = (((self.x + self.width) * scale).ceil().max(0.0) as u32).min(width);
        let bottom = (((self.y + self.height) * scale).ceil().max(0.0) as u32).min(height);

        (
            left.min(width),
            top.min(height),
            right.saturating_sub(left),
            bottom.saturating_sub(top),
        )
    }
}

/// Where everything goes, for a window of a given size.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub height: f32,
    pub width: f32,
    pub column_width: f32,
    pub bar_top: f32,
    pub grid_top: f32,
    pub rows_top: f32,
    /// Row height, squeezed when the window is too short for a 31-row month at the
    /// size the theme asks for.
    pub row_height: f32,
}

impl Layout {
    pub fn new(width: f32, height: f32, columns: usize) -> Self {
        let bar_top = theme::PAD + theme::SQUARE + theme::SECTION_GAP;
        let grid_top = bar_top + theme::BAR_HEIGHT + theme::SECTION_GAP;

        Self {
            width,
            height,
            column_width: theme::column_width(width, columns),
            bar_top,
            grid_top,
            rows_top: grid_top + HEADING_BAND,
            row_height: row_height(height, grid_top + HEADING_BAND),
        }
    }

    /// The whole window, for the frames that have to be painted from scratch.
    pub fn everything(&self) -> Region {
        Region::new(0.0, 0.0, self.width, self.height)
    }

    /// The seven day boxes and the week number beside them.
    pub fn strip_band(&self) -> Region {
        Region::new(0.0, 0.0, self.width, self.bar_top)
    }

    /// The calculator, which is what a keystroke usually changes and nothing else.
    pub fn bar_band(&self) -> Region {
        Region::new(0.0, self.bar_top, self.width, theme::BAR_HEIGHT)
    }

    /// One row of one month column.
    pub fn row_band(&self, column: usize, row: usize) -> Region {
        Region::new(
            self.column_x(column),
            self.row_y(row),
            self.column_width,
            self.row_height,
        )
    }

    fn column_x(&self, index: usize) -> f32 {
        theme::PAD + index as f32 * self.column_width
    }

    fn row_y(&self, index: usize) -> f32 {
        self.rows_top + index as f32 * self.row_height
    }
}

/// The part that only changes when the six months do.
pub fn base(
    pixmap: &mut Pixmap,
    text: &mut Text,
    palette: &Palette,
    overview: &Overview,
    layout: Layout,
    scale: f32,
) {
    pixmap.fill(solid(palette.background));

    for (index, month) in overview.months.iter().enumerate() {
        column(pixmap, text, palette, month, layout, index, scale);
    }
}

/// The part that changes on every key press.
// The three bands and the damage list are what a keystroke threads through; bundling
// them into a context struct would only move the same arguments one level down.
#[allow(clippy::too_many_arguments)]
pub fn overlay(
    pixmap: &mut Pixmap,
    text: &mut Text,
    palette: &Palette,
    overview: &Overview,
    bar: &Bar<'_>,
    layout: Layout,
    scale: f32,
    damage: &[Region],
) {
    if layout.strip_band().touches(damage) {
        strip(pixmap, text, palette, overview, layout, scale);
    }

    if layout.bar_band().touches(damage) {
        calculator(pixmap, text, palette, bar, layout, scale);
    }

    for (index, month) in overview.months.iter().enumerate() {
        marks(pixmap, palette, month, layout, index, scale, damage);
    }
}

fn strip(
    pixmap: &mut Pixmap,
    text: &mut Text,
    palette: &Palette,
    overview: &Overview,
    layout: Layout,
    scale: f32,
) {
    let y = theme::PAD;

    for (index, day) in overview.strip.days.iter().enumerate() {
        let x = theme::PAD + index as f32 * (theme::SQUARE + theme::SQUARE_GAP);
        square(pixmap, text, palette, day, x, y, scale);
    }

    // Beside the boxes rather than above them, which is what made room for the
    // calculator without taking any height from the grid.
    let boxes = 7.0 * theme::SQUARE + 6.0 * theme::SQUARE_GAP;
    let heading = Style::new(theme::WEEK_HEADING_SIZE, palette.ink).bold();
    let left = theme::PAD + boxes + 28.0;

    text.draw(
        pixmap,
        &overview.strip.heading,
        Area::new(
            left,
            y,
            (layout.width - left - theme::PAD).max(0.0),
            theme::SQUARE,
        ),
        heading,
        scale,
    );
}

fn square(
    pixmap: &mut Pixmap,
    text: &mut Text,
    palette: &Palette,
    day: &Day,
    x: f32,
    y: f32,
    scale: f32,
) {
    let (fill, ink) = match day.tense {
        Tense::Past => (palette.past_fill, palette.past_ink),
        Tense::Today => (palette.accent, palette.today_ink),
        Tense::Future => (palette.future_fill, palette.ink),
    };

    let Some(path) = rounded_rect(x, y, theme::SQUARE, theme::SQUARE, theme::SQUARE_RADIUS) else {
        return;
    };

    let transform = Transform::from_scale(scale, scale);
    pixmap.fill_path(
        &path,
        &solid_paint(fill),
        FillRule::Winding,
        transform,
        None,
    );

    if day.highlighted {
        pixmap.fill_path(&path, &wash(palette), FillRule::Winding, transform, None);
    }

    // A future day is white on white, so it needs the border the other two do not.
    if day.tense == Tense::Future {
        pixmap.stroke_path(
            &path,
            &solid_paint(palette.future_border),
            &hairline(1.0),
            transform,
            None,
        );
    }

    // The day the keyboard is on, which is not always today.
    if day.selected {
        pixmap.stroke_path(
            &path,
            &solid_paint(palette.selection),
            &hairline(theme::SELECTION_RING),
            transform,
            None,
        );
    }

    let initial = Style::new(theme::SQUARE_INITIAL_SIZE, ink)
        .bold()
        .centered();
    text.draw(
        pixmap,
        &day.weekday_name().to_uppercase(),
        Area::line(x, y + 12.0, theme::SQUARE, initial),
        initial,
        scale,
    );

    let number = Style::new(theme::SQUARE_NUMBER_SIZE, ink).bold().centered();
    text.draw(
        pixmap,
        &day.number().to_string(),
        Area::new(x, y + 26.0, theme::SQUARE, theme::SQUARE - 26.0),
        number,
        scale,
    );
}

fn calculator(
    pixmap: &mut Pixmap,
    text: &mut Text,
    palette: &Palette,
    bar: &Bar<'_>,
    layout: Layout,
    scale: f32,
) {
    let width = layout.width - 2.0 * theme::PAD;

    if let Some(path) = rounded_rect(
        theme::PAD,
        layout.bar_top,
        width,
        theme::BAR_HEIGHT,
        theme::BAR_RADIUS,
    ) {
        pixmap.fill_path(
            &path,
            &solid_paint(palette.bar_fill),
            FillRule::Winding,
            Transform::from_scale(scale, scale),
            None,
        );
    }

    let inset = 14.0;
    let body = Style::new(theme::BAR_TEXT_SIZE, palette.ink);

    text.draw(
        pixmap,
        &caret(bar),
        Area::new(
            theme::PAD + inset,
            layout.bar_top,
            width - 2.0 * inset,
            theme::BAR_HEIGHT,
        ),
        body,
        scale,
    );

    // A problem replaces the answer and is written small and grey, because the only
    // ones that reach here are real and none of them are urgent.
    if let Some(problem) = bar.problem {
        let style = Style::new(theme::BAR_NOTE_SIZE, palette.muted).right();
        text.draw(
            pixmap,
            problem,
            Area::new(
                theme::PAD + inset,
                layout.bar_top,
                width - 2.0 * inset,
                theme::BAR_HEIGHT,
            ),
            style,
            scale,
        );
        return;
    }

    let Some(answer) = bar.answer else {
        return;
    };

    // An answer older than the line it sits on goes grey. That is the whole of how an
    // unfinished expression reports itself.
    let colour = if bar.faded {
        palette.faded
    } else {
        palette.ink
    };
    let result = Style::new(theme::BAR_TEXT_SIZE, colour).bold().right();
    let has_note = answer.note.is_some();
    let lift = if has_note { 7.0 } else { 0.0 };

    text.draw(
        pixmap,
        &format!("{} {}", theme::PROMPT, answer.text),
        Area::new(
            theme::PAD + inset,
            layout.bar_top - lift,
            width - 2.0 * inset,
            theme::BAR_HEIGHT,
        ),
        result,
        scale,
    );

    if let Some(note) = answer.note.as_deref() {
        let style = Style::new(theme::BAR_NOTE_SIZE, palette.muted).right();
        text.draw(
            pixmap,
            note,
            Area::new(
                theme::PAD + inset,
                layout.bar_top + theme::BAR_HEIGHT / 2.0 + 2.0,
                width - 2.0 * inset,
                theme::BAR_HEIGHT / 2.0,
            ),
            style,
            scale,
        );
    }
}

fn column(
    pixmap: &mut Pixmap,
    text: &mut Text,
    palette: &Palette,
    month: &MonthColumn,
    layout: Layout,
    index: usize,
    scale: f32,
) {
    let x = layout.column_x(index);
    let height = month.days.len() as f32 * layout.row_height;

    if index > 0 {
        fill(
            pixmap,
            x,
            layout.grid_top,
            1.0,
            HEADING_BAND + height,
            palette.rule,
            scale,
        );
    }

    let heading = Style::new(theme::MONTH_HEADING_SIZE, palette.ink).bold();
    text.draw(
        pixmap,
        &month.heading,
        Area::line(
            x + theme::ROW_INSET + 3.0,
            layout.grid_top,
            layout.column_width - 12.0,
            heading,
        ),
        heading,
        scale,
    );

    for (position, day) in month.days.iter().enumerate() {
        row(
            pixmap,
            text,
            palette,
            day,
            layout.row_band(index, position),
            scale,
        );
    }
}

fn row(
    pixmap: &mut Pixmap,
    text: &mut Text,
    palette: &Palette,
    day: &Day,
    area: Region,
    scale: f32,
) {
    let width = area.width - 2.0 * theme::ROW_INSET;
    let height = area.height;
    let (x, y) = (area.x, area.y);

    if day.shaded() {
        fill(
            pixmap,
            x + theme::ROW_INSET,
            y,
            width,
            height,
            palette.shade,
            scale,
        );
    }

    let body = Style::new(theme::MONTH_ROW_SIZE, palette.ink);

    text.draw(
        pixmap,
        &day.initial().to_string(),
        Area::new(x + 6.0, y, 16.0, height),
        body,
        scale,
    );
    text.draw(
        pixmap,
        &day.number().to_string(),
        Area::new(x + 22.0, y, 22.0, height),
        body.right(),
        scale,
    );

    if let Some(holiday) = day.holiday {
        let label = Style::new(theme::HOLIDAY_SIZE, palette.muted);
        text.draw(
            pixmap,
            holiday.name,
            Area::new(x + 50.0, y, HOLIDAY_WIDTH, height),
            label,
            scale,
        );
    }

    if let Some(week) = day.week {
        text.draw(
            pixmap,
            &week.to_string(),
            Area::new(x + area.width - 30.0, y, 24.0, height),
            body.bold().right(),
            scale,
        );
    }
}

/// Everything drawn over the grid: the week on show, the day the keyboard is on, and
/// the days a duration covers.
fn marks(
    pixmap: &mut Pixmap,
    palette: &Palette,
    month: &MonthColumn,
    layout: Layout,
    index: usize,
    scale: f32,
    damage: &[Region],
) {
    let x = layout.column_x(index);
    let width = layout.column_width - 2.0 * theme::ROW_INSET;
    let transform = Transform::from_scale(scale, scale);

    for (position, day) in month.days.iter().enumerate() {
        if !layout.row_band(index, position).touches(damage) {
            continue;
        }

        let y = layout.row_y(position);

        if day.highlighted {
            if let Some(rect) = Rect::from_xywh(x + theme::ROW_INSET, y, width, layout.row_height) {
                pixmap.fill_rect(rect, &wash(palette), transform, None);
            }
        }

        if day.selected {
            fill(
                pixmap,
                x + theme::ROW_INSET,
                y,
                theme::SELECTION_RING,
                layout.row_height,
                palette.selection,
                scale,
            );
        }
    }

    focus_outline(pixmap, palette, month, layout, index, scale, damage);
}

/// One box around the run of days belonging to the week the strip shows.
fn focus_outline(
    pixmap: &mut Pixmap,
    palette: &Palette,
    month: &MonthColumn,
    layout: Layout,
    index: usize,
    scale: f32,
    damage: &[Region],
) {
    let first = month.days.iter().position(|day| day.in_focus_week);
    let last = month.days.iter().rposition(|day| day.in_focus_week);

    let (Some(first), Some(last)) = (first, last) else {
        return;
    };

    let top = layout.row_y(first);
    let height = (last - first + 1) as f32 * layout.row_height;
    let width = layout.column_width - 2.0 * theme::ROW_INSET;

    // The box spans several rows, so it is redrawn whenever any of them is.
    let band = Region::new(layout.column_x(index), top, layout.column_width, height);
    if !band.touches(damage) {
        return;
    }

    let Some(rect) = Rect::from_xywh(
        layout.column_x(index) + theme::ROW_INSET,
        top,
        width,
        height,
    ) else {
        return;
    };

    let Some(path) = PathBuilder::from_rect(rect).stroke(&hairline(2.0), 1.0) else {
        return;
    };

    pixmap.fill_path(
        &path,
        &solid_paint(palette.accent),
        FillRule::Winding,
        Transform::from_scale(scale, scale),
        None,
    );
}

fn fill(pixmap: &mut Pixmap, x: f32, y: f32, width: f32, height: f32, color: Rgb, scale: f32) {
    let Some(rect) = Rect::from_xywh(x, y, width, height) else {
        return;
    };

    pixmap.fill_rect(
        rect,
        &solid_paint(color),
        Transform::from_scale(scale, scale),
        None,
    );
}

fn rounded_rect(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Option<tiny_skia::Path> {
    let mut path = PathBuilder::new();

    path.move_to(x + radius, y);
    path.line_to(x + width - radius, y);
    path.quad_to(x + width, y, x + width, y + radius);
    path.line_to(x + width, y + height - radius);
    path.quad_to(x + width, y + height, x + width - radius, y + height);
    path.line_to(x + radius, y + height);
    path.quad_to(x, y + height, x, y + height - radius);
    path.line_to(x, y + radius);
    path.quad_to(x, y, x + radius, y);
    path.close();

    path.finish()
}

fn hairline(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Butt,
        ..Stroke::default()
    }
}

fn solid(color: Rgb) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.0, color.1, color.2, 255)
}

fn solid_paint(color: Rgb) -> Paint<'static> {
    Paint {
        shader: tiny_skia::Shader::SolidColor(solid(color)),
        anti_alias: true,
        ..Paint::default()
    }
}

/// The highlight, laid over what is already drawn so a highlighted weekend still reads
/// as a weekend and the numbers stay legible through it.
fn wash(palette: &Palette) -> Paint<'static> {
    Paint {
        shader: tiny_skia::Shader::SolidColor(tiny_skia::Color::from_rgba8(
            palette.highlight.0,
            palette.highlight.1,
            palette.highlight.2,
            palette.highlight_alpha,
        )),
        anti_alias: true,
        ..Paint::default()
    }
}

/// What a row can have, given what is left below the heading.
///
/// A month of 31 days at the height the theme asks for is taller than some screens, and
/// a calendar whose last week is off the bottom is the thing this iteration set out to
/// fix. Never taller than the theme says, so a tall window gets white space rather than
/// stretched rows.
fn row_height(window_height: f32, rows_top: f32) -> f32 {
    const LONGEST_MONTH: f32 = 31.0;

    let available = (window_height - rows_top - theme::PAD) / LONGEST_MONTH;

    available.clamp(10.0, theme::MONTH_ROW_HEIGHT)
}

/// What has been typed, with the caret after it when it is showing.
///
/// Appended rather than drawn beside, so the text in front of it does not shift as it
/// blinks, and so both frontends put exactly the same glyph in the same place.
fn caret(bar: &Bar<'_>) -> String {
    match bar.cursor {
        true => format!("{}{}", bar.input, theme::CURSOR),
        false => bar.input.to_owned(),
    }
}
