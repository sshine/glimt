//! Danish weekday, month and holiday names.
//!
//! Nineteen strings and a capitalisation rule, which is less than a locale dependency
//! costs to pull in and far less than it costs to configure. Danish writes month names
//! in lower case inside a sentence and capitalises them as a heading, so both forms are
//! needed and neither can be derived from an `icu` pattern without picking one.

use chrono::Weekday;

const WEEKDAYS: [&str; 7] = [
    "mandag", "tirsdag", "onsdag", "torsdag", "fredag", "lørdag", "søndag",
];

/// The single letters the month columns are keyed by: M T O T F L S.
///
/// Monday and Thursday share M and T with no ambiguity in practice, because the column
/// is read downwards and the sequence is fixed.
const INITIALS: [char; 7] = ['M', 'T', 'O', 'T', 'F', 'L', 'S'];

const MONTHS: [&str; 12] = [
    "januar",
    "februar",
    "marts",
    "april",
    "maj",
    "juni",
    "juli",
    "august",
    "september",
    "oktober",
    "november",
    "december",
];

pub fn weekday(day: Weekday) -> &'static str {
    WEEKDAYS[day.num_days_from_monday() as usize]
}

pub fn weekday_initial(day: Weekday) -> char {
    INITIALS[day.num_days_from_monday() as usize]
}

/// Lower case, as it appears mid-sentence. `None` outside 1..=12.
pub fn month(month: u32) -> Option<&'static str> {
    MONTHS.get(month.checked_sub(1)? as usize).copied()
}

/// Upper-cased first letter, as a heading takes it.
pub fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
