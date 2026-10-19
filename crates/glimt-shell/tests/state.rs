//! The two levels, and what each key does at each of them.
//!
//! None of this needs a compositor, which is the point of it being here rather than in
//! either frontend.

// `allow-expect-in-tests` reaches the bodies of the `#[test]` functions below, but not
// the helpers above them, which are ordinary items of the test crate.
#![allow(clippy::expect_used)]

use chrono::NaiveDate;
use glimt_shell::state::{Action, AppState, Response};
use napkin_math::{Napkin, Rates};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("fixture dates are valid")
}

const TODAY: (i32, u32, u32) = (2026, 10, 19);

fn today() -> NaiveDate {
    date(TODAY.0, TODAY.1, TODAY.2)
}

/// A dashboard with no calculator behind it, for the keys that do not need one.
fn bare() -> AppState {
    AppState::at(today(), 6, None)
}

/// A dashboard with the calculator, pinned to the same day and the compiled-in rates.
fn full() -> AppState {
    let mut napkin = Napkin::with_rates(Rates::baked()).expect("the prelude compiles");
    napkin.set_today(today());

    AppState::at(today(), 6, Some(napkin))
}

fn type_in(state: &mut AppState, text: &str) {
    for character in text.chars() {
        state.apply(Action::Type(character));
    }
}

// Escape, which goes one level back.

#[test]
fn escape_with_nothing_typed_dismisses() {
    let mut state = bare();
    assert_eq!(state.apply(Action::Escape), Response::Hide);
}

#[test]
fn escape_with_something_typed_clears_it_first() {
    let mut state = full();
    type_in(&mut state, "2+2");
    assert_eq!(state.bar().input, "2+2");

    assert_eq!(state.apply(Action::Escape), Response::Redraw);
    assert_eq!(state.bar().input, "");

    // And only then does the next one dismiss.
    assert_eq!(state.apply(Action::Escape), Response::Hide);
}

#[test]
fn escape_dismisses_without_disturbing_the_selection() {
    let mut state = bare();
    state.apply(Action::Weeks(4));
    let moved = state.selection();
    assert_ne!(moved, today());

    assert_eq!(state.apply(Action::Escape), Response::Hide);
    assert_eq!(
        state.selection(),
        moved,
        "dismissing should resume where it was"
    );
}

// Ctrl+Q, which goes all the way back.

#[test]
fn the_root_key_clears_everything_and_dismisses() {
    let mut state = full();
    type_in(&mut state, "13 usd");
    state.apply(Action::Weeks(10));

    assert_eq!(state.apply(Action::Root), Response::Hide);
    assert_eq!(state.bar().input, "");
    assert_eq!(state.selection(), today(), "the root is today");
}

#[test]
fn the_root_key_works_from_either_level() {
    let mut state = bare();
    assert_eq!(state.apply(Action::Root), Response::Hide);
    assert_eq!(state.selection(), today());
}

// Moving about.

#[test]
fn the_arrows_move_a_day_and_a_week() {
    let mut state = bare();

    assert_eq!(state.apply(Action::Days(1)), Response::Redraw);
    assert_eq!(state.selection(), date(2026, 10, 20));

    assert_eq!(state.apply(Action::Days(-1)), Response::Redraw);
    assert_eq!(state.selection(), today());

    assert_eq!(state.apply(Action::Weeks(1)), Response::Redraw);
    assert_eq!(state.selection(), date(2026, 10, 26));

    assert_eq!(state.apply(Action::Weeks(-2)), Response::Redraw);
    assert_eq!(state.selection(), date(2026, 10, 12));
}

#[test]
fn moving_takes_the_strip_with_it_and_leaves_today_alone() {
    let mut state = bare();
    state.apply(Action::Weeks(2));

    assert_eq!(state.overview().strip.week, 45);
    assert_eq!(state.today(), today(), "today does not move");
}

#[test]
fn walking_off_the_end_of_the_year_redraws_the_next_half() {
    let mut state = bare();

    // From 19 October to 1 January, which is in the other block.
    for _ in 0..11 {
        state.apply(Action::Weeks(1));
    }

    assert_eq!(state.selection(), date(2027, 1, 4));
    let months: Vec<u32> = state
        .overview()
        .months
        .iter()
        .map(|column| column.month)
        .collect();
    assert_eq!(months, [1, 2, 3, 4, 5, 6]);
    assert_eq!(state.overview().months[0].year, 2027);
}

#[test]
fn a_key_that_changes_nothing_says_so() {
    let mut state = bare();
    assert_eq!(state.apply(Action::Backspace), Response::Ignored);
    assert_eq!(state.apply(Action::Days(0)), Response::Ignored);
}

// The calculator.

#[test]
fn typing_an_expression_answers_it() {
    let mut state = full();
    type_in(&mut state, "2+3*4");

    let bar = state.bar();
    assert_eq!(bar.input, "2+3*4");
    assert_eq!(bar.answer.map(|answer| answer.text.as_str()), Some("14"));
    assert_eq!(bar.problem, None);
    assert!(!bar.faded);
}

#[test]
fn a_half_typed_expression_keeps_the_last_answer_and_says_nothing() {
    let mut state = full();
    type_in(&mut state, "2+2");
    assert_eq!(state.bar().answer.map(|a| a.text.as_str()), Some("4"));

    // Starting the next one leaves the previous answer up, faded, with no complaint.
    type_in(&mut state, " + ");

    let bar = state.bar();
    assert_eq!(bar.answer.map(|answer| answer.text.as_str()), Some("4"));
    assert!(bar.faded, "the answer is older than the line");
    assert_eq!(bar.problem, None, "nothing to say about a half-typed sum");
}

#[test]
fn an_expression_that_cannot_be_worked_out_gets_one_quiet_line() {
    let mut state = full();
    type_in(&mut state, "1/0");

    let bar = state.bar();
    assert_eq!(bar.problem, Some("division med nul"));
}

#[test]
fn backspacing_back_to_nothing_puts_the_answer_away() {
    let mut state = full();
    type_in(&mut state, "2+2");

    for _ in 0..3 {
        state.apply(Action::Backspace);
    }

    let bar = state.bar();
    assert_eq!(bar.input, "");
    assert_eq!(bar.answer, None);
    assert_eq!(bar.problem, None);
}

#[test]
fn a_duration_paints_the_days_it_covers() {
    let mut state = full();
    type_in(&mut state, "today + 7 days");

    let painted: Vec<NaiveDate> = state
        .overview()
        .months
        .iter()
        .flat_map(|column| column.days.iter())
        .filter(|day| day.highlighted)
        .map(|day| day.date)
        .collect();

    assert_eq!(painted.len(), 8, "today and the seven days after it");
    assert_eq!(painted.first(), Some(&today()));
    assert_eq!(painted.last(), Some(&date(2026, 10, 26)));
}

#[test]
fn clearing_the_input_unpaints_the_calendar() {
    let mut state = full();
    type_in(&mut state, "today + 7 days");
    state.apply(Action::Escape);

    let painted = state
        .overview()
        .months
        .iter()
        .flat_map(|column| column.days.iter())
        .filter(|day| day.highlighted)
        .count();
    assert_eq!(painted, 0);
}

#[test]
fn a_calculator_that_did_not_start_leaves_a_working_calendar() {
    let mut state = bare();

    assert_eq!(state.apply(Action::Type('2')), Response::Redraw);
    assert_eq!(state.bar().problem, Some("regnemaskinen startede ikke"));

    // The calendar still answers its keys.
    assert_eq!(state.apply(Action::Days(1)), Response::Redraw);
    assert_eq!(state.selection(), date(2026, 10, 20));
}
