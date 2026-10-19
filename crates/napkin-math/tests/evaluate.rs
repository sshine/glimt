//! The whole calculator, from a string to what is shown.
//!
//! The rate table is pinned to the compiled-in one and the day is pinned to a Monday in
//! October 2026, so every number here is reproducible and none of it goes near the
//! network.

// `allow-expect-in-tests` reaches the bodies of the `#[test]` functions below, but not
// the helpers above them, which are ordinary items of the test crate.
#![allow(clippy::expect_used)]

use chrono::NaiveDate;
use napkin_math::{Napkin, Outcome, Rates};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("fixture dates are valid")
}

/// One runtime for the whole file.
///
/// Building it costs a second in a debug build, and there is nothing to gain from doing
/// that once per assertion: the calculator holds no state between expressions, which is
/// itself worth knowing.
fn napkin() -> std::sync::MutexGuard<'static, Napkin> {
    static SHARED: std::sync::OnceLock<std::sync::Mutex<Napkin>> = std::sync::OnceLock::new();

    SHARED
        .get_or_init(|| {
            let mut napkin = Napkin::with_rates(Rates::baked()).expect("the prelude compiles");
            napkin.set_today(date(2026, 10, 19));
            std::sync::Mutex::new(napkin)
        })
        .lock()
        .expect("the shared calculator")
}

fn text(input: &str) -> String {
    match napkin().evaluate(input) {
        Outcome::Ok { text, .. } => text,
        other => panic!("{input:?} did not work out: {other:?}"),
    }
}

fn problem(input: &str) -> String {
    match napkin().evaluate(input) {
        Outcome::Problem(message) => message,
        other => panic!("{input:?} should have been a problem: {other:?}"),
    }
}

/// The number out of a formatted amount, for comparing against a computed rate.
fn amount(text: &str) -> f64 {
    text.split_whitespace()
        .next()
        .expect("an amount")
        .replace(',', ".")
        .parse()
        .expect("a number")
}

#[test]
fn arithmetic_comes_out_in_the_usual_order() {
    assert_eq!(text("2+3*4"), "14");
    assert_eq!(text("  2  +  3  *  4  "), "14");
    assert_eq!(text("(2+3)*4"), "20");
    assert_eq!(text("10-3-2"), "5");
    assert_eq!(text("-5 + 2,5"), "-2,5");
    assert_eq!(text("1/4"), "0,25");
}

#[test]
fn a_long_decimal_is_cut_short_rather_than_filling_the_line() {
    assert_eq!(text("1/3"), "0,3333");
    assert_eq!(text("2/2"), "1");
}

#[test]
fn an_explicit_conversion_goes_through_the_euro() {
    let out = text("13 usd in dkk");
    assert!(out.ends_with(" DKK"), "{out}");

    let rates = Rates::baked();
    let usd = rates.per_eur("usd").expect("usd is in the table");
    let dkk = rates.per_eur("dkk").expect("dkk is in the table");
    assert!((amount(&out) - 13.0 / usd * dkk).abs() < 0.01, "{out}");
}

#[test]
fn an_amount_with_no_target_becomes_kroner() {
    assert_eq!(text("13 usd"), text("13 usd in dkk"));
    assert_eq!(text("100 sek"), text("100 sek in dkk"));
}

#[test]
fn kroner_with_no_target_become_euro_since_kroner_say_nothing() {
    let out = text("13 dkk");
    assert!(out.ends_with(" EUR"), "{out}");

    let rates = Rates::baked();
    let dkk = rates.per_eur("dkk").expect("dkk is in the table");
    assert!((amount(&out) - 13.0 / dkk).abs() < 0.01, "{out}");

    // Saying so explicitly is still allowed and means the same thing.
    assert_eq!(out, text("13 dkk in eur"));
}

#[test]
fn an_amount_says_where_its_rate_came_from() {
    match napkin().evaluate("13 usd") {
        Outcome::Ok { note, .. } => {
            let note = note.expect("money carries a note");
            assert!(note.starts_with("ECB "), "{note}");
            assert!(note.contains("2026-10-16"), "{note}");
        }
        other => panic!("{other:?}"),
    }

    // Plain arithmetic owes nobody an explanation.
    match napkin().evaluate("2+2") {
        Outcome::Ok { note, .. } => assert_eq!(note, None),
        other => panic!("{other:?}"),
    }
}

#[test]
fn money_can_be_added_across_currencies() {
    let out = text("10 eur + 10 eur in eur");
    assert_eq!(amount(&out), 20.0);

    // The left-hand currency wins, and the right is converted into it.
    let mixed = text("10 eur + 100 dkk in eur");
    let rates = Rates::baked();
    let dkk = rates.per_eur("dkk").expect("dkk is in the table");
    assert!(
        (amount(&mixed) - (10.0 + 100.0 / dkk)).abs() < 0.01,
        "{mixed}"
    );
}

#[test]
fn money_scales_by_a_number() {
    let single = amount(&text("10 usd in usd"));
    let triple = amount(&text("10 usd * 3 in usd"));
    assert!((triple - single * 3.0).abs() < 0.001);
}

#[test]
fn dates_move_by_durations() {
    assert_eq!(text("today + 7 days"), "mandag 26. oktober 2026");
    assert_eq!(text("2026-10-19 + 30 days"), "onsdag 18. november 2026");
    assert_eq!(text("today - 1 day"), "søndag 18. oktober 2026");
    assert_eq!(text("today + 2 uger"), "mandag 2. november 2026");
    assert_eq!(text("idag + 7 dage"), "mandag 26. oktober 2026");
}

#[test]
fn subtracting_two_dates_gives_the_days_between_them() {
    assert_eq!(text("2026-11-18 - 2026-10-19"), "30 dage");
    assert_eq!(text("today - today"), "0 dage");
    assert_eq!(text("2026-10-20 - 2026-10-19"), "1 dag");
}

#[test]
fn a_duration_paints_the_days_it_covers() {
    let span = napkin().evaluate("today + 7 days").span();
    assert_eq!(span, Some((date(2026, 10, 19), date(2026, 10, 26))));

    let span = napkin().evaluate("2026-10-19 + 30 days").span();
    assert_eq!(span, Some((date(2026, 10, 19), date(2026, 11, 18))));

    // Backwards is still a range, drawn the same way round.
    let span = napkin().evaluate("today - 7 days").span();
    assert_eq!(span, Some((date(2026, 10, 12), date(2026, 10, 19))));

    // Two dates written out paint between them.
    let span = napkin().evaluate("2026-11-18 - 2026-10-19").span();
    assert_eq!(span, Some((date(2026, 10, 19), date(2026, 11, 18))));

    // A bare date is one day.
    let span = napkin().evaluate("2026-12-24").span();
    assert_eq!(span, Some((date(2026, 12, 24), date(2026, 12, 24))));
}

#[test]
fn arithmetic_paints_nothing() {
    assert_eq!(napkin().evaluate("2+2").span(), None);
    assert_eq!(napkin().evaluate("13 usd").span(), None);
}

#[test]
fn nothing_typed_says_nothing() {
    assert_eq!(napkin().evaluate(""), Outcome::Empty);
    assert_eq!(napkin().evaluate("   "), Outcome::Empty);
}

#[test]
fn a_half_typed_expression_is_never_complained_about() {
    for input in [
        "2 +",
        "13 u",
        "13 us",
        "13 usd i",
        "13 usd in",
        "13 usd in d",
        "13 usd in dk",
        "t",
        "tod",
        "today +",
        "today + 7 d",
        "today + 7 da",
        "2,",
        "(2+3",
        "7 w",
    ] {
        assert_eq!(
            napkin().evaluate(input),
            Outcome::Incomplete,
            "for {input:?}"
        );
    }
}

#[test]
fn typing_a_whole_expression_never_passes_through_a_complaint() {
    const VALID: [&str; 6] = [
        "13 usd in dkk",
        "today + 7 days",
        "2026-10-19 + 30 days",
        "2+3*4",
        "13 dkk",
        "idag + 2 uger",
    ];

    let napkin = napkin();

    for input in VALID {
        for end in 0..=input.chars().count() {
            let prefix: String = input.chars().take(end).collect();

            match napkin.evaluate(&prefix) {
                Outcome::Problem(message) => {
                    panic!("prefix {prefix:?} of {input:?} complained: {message}")
                }
                _ => continue,
            }
        }
    }
}

#[test]
fn a_word_that_cannot_become_anything_is_reported() {
    assert_eq!(problem("13 xyzzy"), "ukendt enhed: xyzzy");
    assert_eq!(problem("13 usd in xyzzy"), "ukendt valuta: xyzzy");
}

#[test]
fn arithmetic_that_cannot_be_done_says_why() {
    assert_eq!(problem("1/0"), "division med nul");
    assert_eq!(problem("today + today"), "to datoer kan ikke lægges sammen");
    assert_eq!(
        problem("2026-10-19 + 30 days in dkk"),
        "der kan kun omregnes beløb"
    );
    assert_eq!(problem("today * 2"), "der kan kun ganges med et tal");
}

#[test]
fn the_rate_table_covers_what_a_dane_would_reach_for() {
    let rates = Rates::baked();

    for code in [
        "usd", "eur", "dkk", "sek", "nok", "gbp", "chf", "jpy", "pln", "zar",
    ] {
        assert!(rates.is_currency(code), "{code} should be in the table");
        assert!(rates.per_eur(code).is_some_and(|rate| rate > 0.0), "{code}");
    }

    assert!(!rates.is_currency("xyzzy"));
    assert_eq!(rates.per_eur("eur"), Some(1.0));
}

#[test]
fn the_names_people_type_resolve_to_the_codes_the_ecb_publishes() {
    // Renminbi is the currency's name and yuan its unit; CNY is the code, and the code
    // is the only one of the three the feed carries.
    assert_eq!(text("13 rmb in dkk"), text("13 cny in dkk"));
    assert_eq!(text("13 yuan in dkk"), text("13 cny in dkk"));
    assert_eq!(text("13 euro in dkk"), text("13 eur in dkk"));

    // The answer is given in the code, whichever name was used to ask.
    assert!(text("100 dkk in rmb").ends_with(" CNY"));

    // And half of one is still just half-typed.
    assert_eq!(napkin().evaluate("13 rm"), Outcome::Incomplete);
    assert_eq!(napkin().evaluate("13 yu"), Outcome::Incomplete);
}
