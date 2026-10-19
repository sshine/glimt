//! Checked against ugenr.dk's own output for the second half of 2026.
//!
//! The fixtures are a real calendar rather than a constructed one, which is what makes
//! them worth having: 2026 is a 53-week year, its Monday week numbers run 28 through 53
//! across the half year, and its December carries the four named days that are not all
//! public holidays. A hand-written case would have had to be designed to cover that.

// `allow-expect-in-tests` reaches the bodies of the `#[test]` functions below, but not
// the helpers above them, which are ordinary items of the test crate.
#![allow(clippy::expect_used)]

use chrono::{Datelike, NaiveDate};
use glimt_cal::{Overview, Tense, View, WeekStrip, holiday, step};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("fixture dates are valid")
}

/// The Monday week numbers of a column, in order.
fn week_numbers(overview: &Overview, month: u32) -> Vec<u32> {
    overview
        .months
        .iter()
        .find(|column| column.month == month)
        .expect("month is in the window")
        .days
        .iter()
        .filter_map(|day| day.week)
        .collect()
}

fn months(overview: &Overview) -> Vec<u32> {
    overview.months.iter().map(|column| column.month).collect()
}

#[test]
fn week_42_of_2026_runs_monday_12_to_sunday_18_october() {
    let strip = WeekStrip::new(View::on(date(2026, 10, 18)));

    assert_eq!(strip.week, 42);
    assert_eq!(strip.heading, "Uge 42");
    assert_eq!(strip.days.len(), 7);
    assert_eq!(strip.days[0].date, date(2026, 10, 12));
    assert_eq!(strip.days[6].date, date(2026, 10, 18));
}

#[test]
fn the_strip_colours_days_by_where_today_falls() {
    let strip = WeekStrip::new(View::on(date(2026, 10, 15)));

    let tenses: Vec<Tense> = strip.days.iter().map(|day| day.tense).collect();
    assert_eq!(
        tenses,
        [
            Tense::Past,
            Tense::Past,
            Tense::Past,
            Tense::Today,
            Tense::Future,
            Tense::Future,
            Tense::Future
        ]
    );
    assert_eq!(strip.today().map(|day| day.date), Some(date(2026, 10, 15)));
}

#[test]
fn the_week_belongs_to_the_iso_year_not_the_calendar_one() {
    let new_year = WeekStrip::new(View::on(date(2025, 12, 31)));

    assert_eq!((new_year.week, new_year.year), (1, 2026));
    assert_eq!(new_year.days[0].date, date(2025, 12, 29));
    assert_eq!(new_year.days[6].date, date(2026, 1, 4));
}

#[test]
fn six_months_from_october_is_the_second_half_of_the_year() {
    let overview = Overview::new(View::on(date(2026, 10, 18)), 6);

    let headings: Vec<&str> = overview
        .months
        .iter()
        .map(|column| column.heading.as_str())
        .collect();
    assert_eq!(
        headings,
        [
            "Juli 2026",
            "August 2026",
            "September 2026",
            "Oktober 2026",
            "November 2026",
            "December 2026"
        ]
    );
}

#[test]
fn the_window_aligns_to_the_block_that_contains_the_selection() {
    let quarter = Overview::new(View::on(date(2026, 10, 18)), 3);
    assert_eq!(months(&quarter), [10, 11, 12]);

    let year = Overview::new(View::on(date(2026, 10, 18)), 12);
    assert_eq!(year.months.first().map(|column| column.month), Some(1));
    assert_eq!(year.months.len(), 12);
}

#[test]
fn monday_week_numbers_match_the_reference_half_year() {
    let overview = Overview::new(View::on(date(2026, 10, 18)), 6);

    assert_eq!(week_numbers(&overview, 7), [28, 29, 30, 31]);
    assert_eq!(week_numbers(&overview, 8), [32, 33, 34, 35, 36]);
    assert_eq!(week_numbers(&overview, 9), [37, 38, 39, 40]);
    assert_eq!(week_numbers(&overview, 10), [41, 42, 43, 44]);
    assert_eq!(week_numbers(&overview, 11), [45, 46, 47, 48, 49]);

    // 2026 is a 53-week year, which is the case a constructed fixture tends to miss.
    assert_eq!(week_numbers(&overview, 12), [50, 51, 52, 53]);
}

#[test]
fn columns_start_on_the_weekday_the_reference_prints() {
    let overview = Overview::new(View::on(date(2026, 10, 18)), 6);

    let initials: Vec<char> = overview
        .months
        .iter()
        .filter_map(|column| column.days.first())
        .map(|day| day.initial())
        .collect();
    assert_eq!(initials, ['O', 'L', 'T', 'T', 'S', 'T']);
}

#[test]
fn the_month_columns_outline_the_week_the_strip_shows() {
    let overview = Overview::new(View::on(date(2026, 10, 18)), 6);

    let outlined: Vec<u32> = overview
        .months
        .iter()
        .flat_map(|column| column.days.iter())
        .filter(|day| day.in_focus_week)
        .map(|day| day.number())
        .collect();
    assert_eq!(outlined, [12, 13, 14, 15, 16, 17, 18]);
}

#[test]
fn december_carries_the_named_days_and_shades_only_the_public_ones() {
    let overview = Overview::new(View::on(date(2026, 10, 18)), 6);
    let december = overview
        .months
        .iter()
        .find(|column| column.month == 12)
        .expect("december is in the window");

    let named: Vec<(u32, &str, bool)> = december
        .days
        .iter()
        .filter_map(|day| day.holiday.map(|h| (day.number(), h.name, h.public)))
        .collect();
    assert_eq!(
        named,
        [
            (24, "Juleaften", false),
            (25, "Juledag", true),
            (26, "2. juledag", true),
            (31, "Nytårsaften", false),
        ]
    );

    // Christmas Eve is a working day in law, so it is named but not shaded.
    let eve = december
        .days
        .iter()
        .find(|day| day.number() == 24)
        .expect("december has a 24th");
    assert!(!eve.shaded());
}

#[test]
fn easter_drives_the_movable_holidays() {
    assert_eq!(holiday::easter(2026), Some(date(2026, 4, 5)));

    let names = |day: NaiveDate| holiday::holiday(day).map(|h| h.name);
    assert_eq!(names(date(2026, 4, 2)), Some("Skærtorsdag"));
    assert_eq!(names(date(2026, 4, 3)), Some("Langfredag"));
    assert_eq!(names(date(2026, 4, 6)), Some("2. påskedag"));
    assert_eq!(names(date(2026, 5, 14)), Some("Kristi himmelfartsdag"));
    assert_eq!(names(date(2026, 5, 24)), Some("Pinsedag"));
    assert_eq!(names(date(2026, 5, 25)), Some("2. pinsedag"));
}

#[test]
fn store_bededag_stops_in_2024() {
    let name = |day: NaiveDate| holiday::holiday(day).map(|h| h.name);

    assert_eq!(name(date(2023, 5, 5)), Some("Store bededag"));
    assert_eq!(name(date(2024, 4, 26)), None);
    assert_eq!(holiday::after_easter(2023, 26), Some(date(2023, 5, 5)));
}

// The selection, which is what the arrow keys move.

#[test]
fn the_selection_moves_the_strip_but_not_the_colours() {
    let today = date(2026, 10, 18);
    let view = View::on(today).with_selection(date(2026, 11, 3));
    let overview = Overview::new(view, 6);

    // The strip followed the selection.
    assert_eq!(overview.strip.week, 45);
    assert_eq!(
        overview.strip.selected().map(|day| day.date),
        Some(date(2026, 11, 3))
    );

    // Today is still today, in the month column where it lives.
    let october = overview
        .months
        .iter()
        .find(|column| column.month == 10)
        .expect("october is in the window");
    let eighteenth = october
        .days
        .iter()
        .find(|day| day.number() == 18)
        .expect("october has an 18th");
    assert_eq!(eighteenth.tense, Tense::Today);
    assert!(!eighteenth.selected);
}

#[test]
fn stepping_off_the_end_of_a_month_lands_in_the_next_one() {
    assert_eq!(step(date(2026, 10, 31), 1), date(2026, 11, 1));
    assert_eq!(step(date(2026, 11, 1), -1), date(2026, 10, 31));
    assert_eq!(step(date(2026, 2, 28), 1), date(2026, 3, 1));
    assert_eq!(step(date(2024, 2, 28), 1), date(2024, 2, 29));
}

#[test]
fn stepping_a_week_crosses_the_year_and_keeps_the_weekday() {
    let monday = date(2026, 12, 28);
    let next = step(monday, 7);

    assert_eq!(next, date(2027, 1, 4));
    assert_eq!(next.weekday(), monday.weekday());

    // 2026 ends in week 53 and 2027 starts in week 53's successor.
    assert_eq!(WeekStrip::new(View::on(monday)).week, 53);
    assert_eq!(WeekStrip::new(View::on(next)).week, 1);
}

#[test]
fn walking_off_december_redraws_the_next_half_year() {
    let today = date(2026, 10, 18);

    let december = Overview::new(View::on(today).with_selection(date(2026, 12, 31)), 6);
    assert_eq!(months(&december), [7, 8, 9, 10, 11, 12]);

    let january = Overview::new(
        View::on(today).with_selection(step(date(2026, 12, 31), 1)),
        6,
    );
    assert_eq!(months(&january), [1, 2, 3, 4, 5, 6]);
    assert_eq!(january.months[0].year, 2027);

    // Backwards over the same edge returns the half year it came from.
    let back = Overview::new(View::on(today).with_selection(date(2026, 6, 30)), 6);
    assert_eq!(months(&back), [1, 2, 3, 4, 5, 6]);
    assert_eq!(back.months[0].year, 2026);
}

#[test]
fn the_highlight_paints_a_range_across_months() {
    let today = date(2026, 10, 19);
    let view = View::on(today).with_highlight(Some((today, date(2026, 11, 18))));
    let overview = Overview::new(view, 6);

    let painted: Vec<NaiveDate> = overview
        .months
        .iter()
        .flat_map(|column| column.days.iter())
        .filter(|day| day.highlighted)
        .map(|day| day.date)
        .collect();

    assert_eq!(painted.len(), 31);
    assert_eq!(painted.first(), Some(&date(2026, 10, 19)));
    assert_eq!(painted.last(), Some(&date(2026, 11, 18)));
}

#[test]
fn a_highlight_given_backwards_is_still_a_range() {
    let today = date(2026, 10, 19);
    let backwards = View::on(today).with_highlight(Some((date(2026, 10, 25), today)));

    assert_eq!(backwards.highlight, Some((today, date(2026, 10, 25))));

    let strip = WeekStrip::new(backwards);
    assert_eq!(strip.days.iter().filter(|day| day.highlighted).count(), 7);
}

#[test]
fn step_saturates_rather_than_wrapping() {
    let far = NaiveDate::MAX;
    assert_eq!(step(far, 1), far);
    assert_eq!(step(NaiveDate::MIN, -1), NaiveDate::MIN);
}
