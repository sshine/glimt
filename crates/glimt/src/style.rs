//! The style sheet, generated from the shared palette.
//!
//! Written out of the same colours the other candidate paints from, rather than copied
//! into a `.css` file, because a colour that differs between the two candidates would
//! read as a rendering difference when comparing screenshots. Taking the palette as an
//! argument is also what lets the sheet be regenerated when the session switches between
//! light and dark without reloading the process.
//!
//! The backdrop goes on a widget this project owns rather than on the window. Putting
//! it on the window leaves the drawing at the mercy of whichever Adwaita variant the
//! session happens to be in.

use glimt_shell::theme::{self, Palette};

pub fn sheet(palette: &Palette) -> String {
    let ink = palette.ink.hex();
    let muted = palette.muted.hex();
    let shade = palette.shade.hex();
    let rule = palette.rule.hex();
    let accent = palette.accent.hex();
    let selection = palette.selection.hex();
    let background = palette.background.hex();
    let past_fill = palette.past_fill.hex();
    let past_ink = palette.past_ink.hex();
    let today_ink = palette.today_ink.hex();
    let future_fill = palette.future_fill.hex();
    let future_border = palette.future_border.hex();
    let bar_fill = palette.bar_fill.hex();
    let faded = palette.faded.hex();

    let font = theme::FONT;
    let heading = theme::WEEK_HEADING_SIZE;
    let initial = theme::SQUARE_INITIAL_SIZE;
    let number = theme::SQUARE_NUMBER_SIZE;
    let month_heading = theme::MONTH_HEADING_SIZE;
    let month_row = theme::MONTH_ROW_SIZE;
    let holiday = theme::HOLIDAY_SIZE;
    let radius = theme::SQUARE_RADIUS;
    let ring = theme::SELECTION_RING;
    let bar_text = theme::BAR_TEXT_SIZE;
    let bar_note = theme::BAR_NOTE_SIZE;
    let bar_radius = theme::BAR_RADIUS;
    let pad = theme::PAD;
    let alpha = f64::from(palette.highlight_alpha) / 255.0;
    let (hr, hg, hb) = (
        palette.highlight.0,
        palette.highlight.1,
        palette.highlight.2,
    );
    let scrim = f64::from(palette.scrim_alpha) / 255.0;

    format!(
        "
window.glimt {{ background-color: {background}; font-family: {font}; }}
.glimt-root  {{ background-color: {background}; color: {ink}; padding: {pad}px; }}

.week-heading {{ font-size: {heading}px; font-weight: 800; color: {ink}; }}

.day          {{ border-radius: {radius}px; }}
.day-initial  {{ font-size: {initial}px; font-weight: 700; letter-spacing: 1px; }}
.day-number   {{ font-size: {number}px;  font-weight: 800; }}

.day.past   {{ background-color: {past_fill}; }}
.day.past label {{ color: {past_ink}; }}

.day.today  {{ background-color: {accent}; }}
.day.today label {{ color: {today_ink}; }}

.day.future {{ background-color: {future_fill}; border: 1px solid {future_border}; }}
.day.future label {{ color: {ink}; }}

/* The day the keyboard is on, which is not always today. */
.day.selected {{ outline: {ring}px solid {selection}; outline-offset: -{ring}px; }}
.day.highlighted {{ background-image: linear-gradient(rgba({hr}, {hg}, {hb}, {alpha}),
                                                   rgba({hr}, {hg}, {hb}, {alpha})); }}

.month          {{ border-left: 1px solid {rule}; }}
.month-heading  {{ font-size: {month_heading}px; font-weight: 700; color: {ink}; }}

.row        {{ font-size: {month_row}px; color: {ink}; }}
.row.shaded {{ background-color: {shade}; }}
.row .week  {{ font-weight: 700; }}
.row .holiday {{ font-size: {holiday}px; color: {muted}; }}

/* A flat gradient rather than a background colour, because background-image paints over
   background-color: a highlighted weekend keeps its grey underneath. An inset box-shadow
   would do the same thing but collides with the one the selected row uses, and the two
   cannot both win. */
.row.highlighted {{ background-image: linear-gradient(rgba({hr}, {hg}, {hb}, {alpha}),
                                                   rgba({hr}, {hg}, {hb}, {alpha})); }}
.row.selected {{ box-shadow: inset {ring}px 0 0 0 {selection}; }}

/* The reference draws one box around the whole run of seven rows, so the sides are on
   every row of it and the ends carry the cap. */
.row.focus        {{ border-left: 2px solid {accent}; border-right: 2px solid {accent}; }}
.row.focus-first  {{ border-top: 2px solid {accent}; }}
.row.focus-last   {{ border-bottom: 2px solid {accent}; }}

.bar        {{ background-color: {bar_fill}; border-radius: {bar_radius}px;
               padding: 0 14px; }}
.bar-input  {{ font-size: {bar_text}px; color: {ink}; }}
.bar-result {{ font-size: {bar_text}px; font-weight: 700; color: {ink}; }}
.bar-result.faded {{ color: {faded}; font-weight: 400; }}
.bar-result.problem {{ font-size: {bar_note}px; font-weight: 400; color: {muted}; }}
.bar-note   {{ font-size: {bar_note}px; color: {muted}; }}

/* The fullscreen shape: a translucent backdrop with the dashboard as a card on it. */
window.glimt.overlay {{ background-color: rgba(0, 0, 0, {scrim}); }}
.card {{ background-color: {background}; border-radius: 14px; }}
"
    )
}
