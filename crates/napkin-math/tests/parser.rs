//! The parser, which is a pure function of a string.
//!
//! Most of what this is asked to parse is a prefix of something valid, because the
//! calculator runs on every keystroke. The property at the bottom of this file is the
//! one that matters most: no prefix of a valid expression is ever reported as a mistake.

// `allow-expect-in-tests` reaches the bodies of the `#[test]` functions below, but not
// the helpers above them, which are ordinary items of the test crate.
#![allow(clippy::expect_used)]

use chrono::NaiveDate;
use napkin_math::parser::{Expr, Op, ParseError, parse};

fn num(n: f64) -> Expr {
    Expr::Number(n)
}

fn bin(op: Op, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

fn unit(value: Expr, unit: &str) -> Expr {
    Expr::Unit {
        value: Box::new(value),
        unit: unit.to_owned(),
    }
}

fn ok(input: &str) -> Expr {
    parse(input).unwrap_or_else(|error| panic!("{input:?} should parse, got {error:?}"))
}

fn error(input: &str) -> ParseError {
    parse(input).expect_err(input).clone()
}

// Arithmetic.

#[test]
fn whitespace_anywhere_makes_no_difference() {
    let expected = bin(Op::Add, num(2.0), bin(Op::Mul, num(3.0), num(4.0)));

    for input in [
        "2+3*4",
        "2 + 3 * 4",
        "  2+3*4  ",
        "\t2\t+\t3\t*\t4\t",
        "2 +3* 4",
        "   2   +   3   *   4   ",
        "\n2\n+\n3\n*\n4\n",
    ] {
        assert_eq!(ok(input), expected, "for {input:?}");
    }
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    assert_eq!(
        ok("2+3*4"),
        bin(Op::Add, num(2.0), bin(Op::Mul, num(3.0), num(4.0)))
    );
    assert_eq!(
        ok("2*3+4"),
        bin(Op::Add, bin(Op::Mul, num(2.0), num(3.0)), num(4.0))
    );
    assert_eq!(
        ok("8/4/2"),
        bin(Op::Div, bin(Op::Div, num(8.0), num(4.0)), num(2.0))
    );
}

#[test]
fn addition_and_subtraction_group_to_the_left() {
    assert_eq!(
        ok("10-3-2"),
        bin(Op::Sub, bin(Op::Sub, num(10.0), num(3.0)), num(2.0))
    );
}

#[test]
fn parentheses_override_precedence() {
    assert_eq!(
        ok("(2+3)*4"),
        bin(Op::Mul, bin(Op::Add, num(2.0), num(3.0)), num(4.0))
    );
    assert_eq!(ok("((((7))))"), num(7.0));
    assert_eq!(ok(" ( 2 + 3 ) "), bin(Op::Add, num(2.0), num(3.0)));
}

#[test]
fn a_leading_sign_negates_and_a_leading_plus_does_nothing() {
    assert_eq!(ok("-5"), Expr::Neg(Box::new(num(5.0))));
    assert_eq!(ok("+5"), num(5.0));
    assert_eq!(
        ok("2 * -3"),
        bin(Op::Mul, num(2.0), Expr::Neg(Box::new(num(3.0))))
    );
    assert_eq!(
        ok("-2 + 3"),
        bin(Op::Add, Expr::Neg(Box::new(num(2.0))), num(3.0))
    );
}

#[test]
fn decimals_are_written_with_either_separator() {
    assert_eq!(ok("13.5"), num(13.5));
    assert_eq!(ok("13,5"), num(13.5));
    assert_eq!(ok("0,25 + 0.25"), bin(Op::Add, num(0.25), num(0.25)));
}

// Units and currencies.

#[test]
fn a_word_after_a_number_is_its_unit() {
    assert_eq!(ok("13 usd"), unit(num(13.0), "usd"));
    assert_eq!(ok("13usd"), unit(num(13.0), "usd"));
    assert_eq!(ok("7 days"), unit(num(7.0), "days"));
    assert_eq!(ok("7 dage"), unit(num(7.0), "dage"));
}

#[test]
fn a_unit_is_not_confused_with_the_word_in() {
    assert_eq!(
        ok("13 usd in dkk"),
        Expr::Convert {
            value: Box::new(unit(num(13.0), "usd")),
            to: "dkk".to_owned(),
        }
    );
    assert_eq!(
        ok("13 in dkk"),
        Expr::Convert {
            value: Box::new(num(13.0)),
            to: "dkk".to_owned(),
        }
    );
}

#[test]
fn case_does_not_matter_for_words() {
    assert_eq!(ok("13 USD In DKK"), ok("13 usd in dkk"));
    assert_eq!(ok("TODAY"), Expr::Today);
}

#[test]
fn today_is_a_value_in_both_languages() {
    assert_eq!(ok("today"), Expr::Today);
    assert_eq!(ok("idag"), Expr::Today);
    assert_eq!(
        ok("today + 7 days"),
        bin(Op::Add, Expr::Today, unit(num(7.0), "days"))
    );
}

// Dates.

#[test]
fn a_date_is_one_token_and_a_spaced_out_one_is_subtraction() {
    let date = NaiveDate::from_ymd_opt(2026, 10, 19).expect("a real date");
    assert_eq!(ok("2026-10-19"), Expr::Date(date));

    assert_eq!(
        ok("2026 - 10 - 19"),
        bin(Op::Sub, bin(Op::Sub, num(2026.0), num(10.0)), num(19.0))
    );
}

#[test]
fn dates_take_part_in_arithmetic() {
    let date = NaiveDate::from_ymd_opt(2026, 10, 19).expect("a real date");
    assert_eq!(
        ok("2026-10-19 + 30 days"),
        bin(Op::Add, Expr::Date(date), unit(num(30.0), "days"))
    );
}

#[test]
fn digits_that_are_not_a_day_are_rejected() {
    assert_eq!(error("2026-02-30"), ParseError::BadDate { at: 0 });
    assert_eq!(error("2026-13-01"), ParseError::BadDate { at: 0 });
    assert_eq!(error("2026-00-10"), ParseError::BadDate { at: 0 });

    // A leap day in a leap year is a day.
    assert!(parse("2024-02-29").is_ok());
}

// What is still being typed, and what is wrong.

#[test]
fn nothing_typed_is_its_own_answer() {
    assert_eq!(error(""), ParseError::Empty);
    assert_eq!(error("   "), ParseError::Empty);
    assert_eq!(error("\t\n "), ParseError::Empty);
}

#[test]
fn running_out_of_input_is_not_a_mistake() {
    for input in [
        "2 +",
        "2 + 3 *",
        "-",
        "(",
        "(2+3",
        "( 2 + ",
        "13 usd in",
        "2 + (",
    ] {
        assert_eq!(error(input), ParseError::Incomplete, "for {input:?}");
    }
}

#[test]
fn a_word_where_a_value_belongs_is_a_mistake() {
    assert!(matches!(
        error("usd 13"),
        ParseError::Unexpected { at: 0, .. }
    ));
    assert!(matches!(error(")"), ParseError::Unexpected { at: 0, .. }));
    assert!(matches!(error("2 3"), ParseError::Unexpected { .. }));
    assert!(matches!(
        error("13 usd in 5"),
        ParseError::Unexpected { .. }
    ));
}

#[test]
fn a_character_the_language_has_no_use_for_is_reported_with_its_place() {
    assert_eq!(
        error("2 # 3"),
        ParseError::Unlexable {
            at: 2,
            character: '#'
        }
    );
    assert_eq!(
        error("13 usd ? dkk"),
        ParseError::Unlexable {
            at: 7,
            character: '?'
        }
    );
}

/// The one that matters: typing a valid expression one character at a time never passes
/// through a state the interface would complain about.
#[test]
fn no_prefix_of_a_valid_expression_is_ever_a_mistake() {
    const VALID: [&str; 9] = [
        "2+3*4",
        "  2  +  3  *  4  ",
        "13 usd in dkk",
        "13 dkk",
        "today + 7 days",
        "2026-10-19 + 30 days",
        "(2+3)*4",
        "-5 + 2,5",
        "13 usd in dkk",
    ];

    for input in VALID {
        for end in 0..=input.chars().count() {
            let prefix: String = input.chars().take(end).collect();

            match parse(&prefix) {
                Ok(_) | Err(ParseError::Empty) | Err(ParseError::Incomplete) => {}
                Err(other) => panic!("prefix {prefix:?} of {input:?} was rejected: {other:?}"),
            }
        }
    }
}
