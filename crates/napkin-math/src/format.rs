//! Results, as they are read.
//!
//! Danish writes the decimal separator as a comma, which is also what the lexer accepts
//! on the way in, so a result can be typed straight back into the next calculation.

use chrono::{Datelike, NaiveDate};
use glimt_cal::names;

use crate::value::Value;

pub fn value(value: &Value) -> String {
    match value {
        Value::Number(n) => number(*n),
        Value::Money { amount, currency } => {
            format!("{} {}", fixed(*amount, 2), currency.to_uppercase())
        }
        Value::Date(date) => day(*date),
        Value::Duration { days } => duration(*days),
    }
}

/// Up to four decimals, with the ones that say nothing removed.
pub fn number(x: f64) -> String {
    if !x.is_finite() {
        return "ikke et tal".to_owned();
    }

    let text = fixed(x, 4);
    let trimmed = match text.split_once(',') {
        Some((whole, fraction)) => {
            let fraction = fraction.trim_end_matches('0');
            if fraction.is_empty() {
                whole.to_owned()
            } else {
                format!("{whole},{fraction}")
            }
        }
        None => text,
    };

    // -0 is an answer nobody wants to read.
    if trimmed == "-0" {
        "0".to_owned()
    } else {
        trimmed
    }
}

fn fixed(x: f64, decimals: usize) -> String {
    format!("{x:.decimals$}").replace('.', ",")
}

/// "mandag 26. oktober 2026".
pub fn day(date: NaiveDate) -> String {
    format!(
        "{} {}. {} {}",
        names::weekday(date.weekday()),
        date.day(),
        names::month(date.month()).unwrap_or_default(),
        date.year()
    )
}

fn duration(days: i64) -> String {
    match days.abs() {
        1 => format!("{days} dag"),
        _ => format!("{days} dage"),
    }
}
