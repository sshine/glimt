//! The widget tree, and how it is kept up to date.
//!
//! Built once and changed in place. That is the half of this candidate worth measuring:
//! a keystroke sets a few labels and toggles a few style classes, and GTK works out
//! which pixels that touched. The other spike has to decide that for itself, and the
//! difference in how much code each of those takes is part of the answer.
//!
//! The month rows are rebuilt only when the six months on show change, which happens
//! when the selection walks off the end of a half year. Everything else a key can do
//! changes classes, not structure.

use glimt_cal::{Day, MonthColumn, Overview, Tense};
use glimt_shell::{AppState, theme};
use gtk4 as gtk;
use gtk4::prelude::*;

/// The labels inside one of the seven day boxes.
struct Square {
    frame: gtk::Box,
    initial: gtk::Label,
    number: gtk::Label,
}

/// The labels inside one row of a month .
struct Row {
    frame: gtk::Box,
    initial: gtk::Label,
    number: gtk::Label,
    holiday: gtk::Label,
    week: gtk::Label,
}

pub struct Ui {
    pub root: gtk::Box,
    heading: gtk::Label,
    squares: Vec<Square>,
    grid: gtk::Box,
    rows: Vec<Row>,
    /// Where each month.s rows sit in `rows`, so the box around the week on show can be
    /// capped at the top and bottom of each column it crosses.
    spans: Vec<std::ops::Range<usize>>,
    input: gtk::Label,
    result: gtk::Label,
    note: gtk::Label,
    /// The first month on show, which is what decides whether the grid has to be built
    /// again rather than merely restyled.
    window: (i32, u32),
}

impl Ui {
    pub fn build(state: &AppState) -> Self {
        let overview = state.overview();

        let root = gtk::Box::new(gtk::Orientation::Vertical, theme::SECTION_GAP as i32);
        root.add_css_class("glimt-root");

        let (strip, squares, heading) = build_strip(overview);
        let (bar, input, result, note) = build_bar();
        let grid = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        grid.set_vexpand(true);

        root.append(&strip);
        root.append(&bar);
        root.append(&grid);

        let mut ui = Self {
            root,
            heading,
            squares,
            grid,
            rows: Vec::new(),
            spans: Vec::new(),
            input,
            result,
            note,
            window: (0, 0),
        };

        ui.fill_grid(overview);
        ui.update(state);
        ui
    }

    pub fn update(&mut self, state: &AppState) {
        let overview = state.overview();

        self.heading.set_text(&overview.strip.heading);

        for (square, day) in self.squares.iter().zip(&overview.strip.days) {
            square.initial.set_text(&day.weekday_name().to_uppercase());
            square.number.set_text(&day.number().to_string());
            style_square(square, day);
        }

        if self.window != first_month(overview) {
            self.fill_grid(overview);
        }

        for (span, month) in self.spans.iter().zip(&overview.months) {
            let rows = &self.rows[span.clone()];

            for (index, (row, day)) in rows.iter().zip(&month.days).enumerate() {
                let first = day.in_focus_week
                    && index
                        .checked_sub(1)
                        .is_none_or(|prev| !month.days[prev].in_focus_week);
                let last = day.in_focus_week
                    && month
                        .days
                        .get(index + 1)
                        .is_none_or(|next| !next.in_focus_week);

                style_row(row, day, first, last);
            }
        }

        self.update_bar(state);
    }

    fn update_bar(&mut self, state: &AppState) {
        let bar = state.bar();

        self.input.set_text(bar.input);

        match (bar.problem, bar.answer) {
            (Some(problem), _) => {
                self.result.set_text(problem);
                set_class(&self.result, "problem", true);
                set_class(&self.result, "faded", false);
                self.note.set_text("");
            }
            (None, Some(answer)) => {
                self.result
                    .set_text(&format!("{} {}", theme::PROMPT, answer.text));
                set_class(&self.result, "problem", false);
                set_class(&self.result, "faded", bar.faded);
                self.note
                    .set_text(answer.note.as_deref().unwrap_or_default());
            }
            (None, None) => {
                self.result.set_text("");
                self.note.set_text("");
                set_class(&self.result, "problem", false);
                set_class(&self.result, "faded", false);
            }
        }
    }

    /// Build the month columns from scratch, which only the six months changing needs.
    fn fill_grid(&mut self, overview: &Overview) {
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }

        self.rows.clear();
        self.spans.clear();

        for (index, month) in overview.months.iter().enumerate() {
            let (column, rows) = build_column(month, index > 0);
            self.grid.append(&column);

            let start = self.rows.len();
            self.rows.extend(rows);
            self.spans.push(start..self.rows.len());
        }

        self.window = first_month(overview);
    }
}

fn first_month(overview: &Overview) -> (i32, u32) {
    overview
        .months
        .first()
        .map(|column| (column.year, column.month))
        .unwrap_or_default()
}

fn build_strip(overview: &Overview) -> (gtk::Box, Vec<Square>, gtk::Label) {
    let strip = gtk::Box::new(gtk::Orientation::Horizontal, theme::SQUARE_GAP as i32);
    strip.set_size_request(-1, theme::SQUARE as i32);

    let squares: Vec<Square> = overview.strip.days.iter().map(|_| build_square()).collect();
    for square in &squares {
        strip.append(&square.frame);
    }

    // Beside the boxes rather than above them: the same information in no extra height.
    let heading = gtk::Label::new(None);
    heading.add_css_class("week-heading");
    heading.set_margin_start(28);
    heading.set_valign(gtk::Align::Center);

    strip.append(&heading);

    (strip, squares, heading)
}

fn build_square() -> Square {
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.set_size_request(theme::SQUARE as i32, theme::SQUARE as i32);
    // A size request is a minimum, and a box child fills its row, so without this the
    // squares stretch to whatever height the strip ends up with and stop being square.
    frame.set_valign(gtk::Align::Center);
    frame.set_vexpand(false);
    frame.add_css_class("day");

    let initial = gtk::Label::new(None);
    initial.add_css_class("day-initial");
    initial.set_margin_top(12);

    let number = gtk::Label::new(None);
    number.add_css_class("day-number");
    number.set_vexpand(true);

    frame.append(&initial);
    frame.append(&number);

    Square {
        frame,
        initial,
        number,
    }
}

fn build_bar() -> (gtk::Box, gtk::Label, gtk::Label, gtk::Label) {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    bar.add_css_class("bar");
    bar.set_size_request(-1, theme::BAR_HEIGHT as i32);

    let input = gtk::Label::new(None);
    input.add_css_class("bar-input");
    input.set_hexpand(true);
    input.set_xalign(0.0);
    input.set_ellipsize(gtk::pango::EllipsizeMode::Start);

    let right = gtk::Box::new(gtk::Orientation::Vertical, 0);
    right.set_valign(gtk::Align::Center);

    let result = gtk::Label::new(None);
    result.add_css_class("bar-result");
    result.set_xalign(1.0);

    let note = gtk::Label::new(None);
    note.add_css_class("bar-note");
    note.set_xalign(1.0);

    right.append(&result);
    right.append(&note);

    bar.append(&input);
    bar.append(&right);

    (bar, input, result, note)
}

fn build_column(month: &MonthColumn, rule: bool) -> (gtk::Box, Vec<Row>) {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_size_request(theme::MONTH_COLUMN_MIN as i32, -1);
    column.set_hexpand(true);

    if rule {
        column.add_css_class("month");
    }

    let heading = gtk::Label::new(Some(&month.heading));
    heading.add_css_class("month-heading");
    heading.set_margin_bottom(6);
    heading.set_margin_start(theme::ROW_INSET as i32 + 3);
    heading.set_xalign(0.0);

    column.append(&heading);

    let rows: Vec<Row> = month
        .days
        .iter()
        .map(|day| {
            let row = build_row(day);
            column.append(&row.frame);
            row
        })
        .collect();

    (column, rows)
}

fn build_row(day: &Day) -> Row {
    let frame = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    frame.set_size_request(-1, theme::MONTH_ROW_HEIGHT as i32);
    frame.add_css_class("row");

    // The shaded background and the week number both stop short of the column edge, so
    // a week number and the next column's weekday letter do not end up touching.
    frame.set_margin_start(theme::ROW_INSET as i32);
    frame.set_margin_end(theme::ROW_INSET as i32);

    let initial = gtk::Label::new(Some(&day.initial().to_string()));
    initial.set_size_request(16, -1);
    initial.set_xalign(0.0);

    let number = gtk::Label::new(Some(&day.number().to_string()));
    number.set_size_request(22, -1);
    number.set_xalign(1.0);

    let holiday = gtk::Label::new(day.holiday.map(|h| h.name));
    holiday.add_css_class("holiday");
    holiday.set_hexpand(true);
    holiday.set_xalign(0.0);
    holiday.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let week = gtk::Label::new(day.week.map(|w| w.to_string()).as_deref());
    week.add_css_class("week");
    week.set_size_request(22, -1);
    week.set_xalign(1.0);

    frame.append(&initial);
    frame.append(&number);
    frame.append(&holiday);
    frame.append(&week);

    Row {
        frame,
        initial,
        number,
        holiday,
        week,
    }
}

fn style_square(square: &Square, day: &Day) {
    set_class(&square.frame, "past", day.tense == Tense::Past);
    set_class(&square.frame, "today", day.tense == Tense::Today);
    set_class(&square.frame, "future", day.tense == Tense::Future);
    set_class(&square.frame, "selected", day.selected);
    set_class(&square.frame, "highlighted", day.highlighted);
}

fn style_row(row: &Row, day: &Day, focus_first: bool, focus_last: bool) {
    // The dates themselves only change when the grid is rebuilt, but doing it here as
    // well costs nothing and keeps the two paths from disagreeing.
    row.initial.set_text(&day.initial().to_string());
    row.number.set_text(&day.number().to_string());
    row.holiday
        .set_text(day.holiday.map(|h| h.name).unwrap_or_default());
    row.week
        .set_text(&day.week.map(|w| w.to_string()).unwrap_or_default());

    set_class(&row.frame, "shaded", day.shaded());
    set_class(&row.frame, "focus", day.in_focus_week);
    set_class(&row.frame, "focus-first", focus_first);
    set_class(&row.frame, "focus-last", focus_last);
    set_class(&row.frame, "selected", day.selected);
    set_class(&row.frame, "highlighted", day.highlighted);
}

fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, wanted: bool) {
    if wanted {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}
