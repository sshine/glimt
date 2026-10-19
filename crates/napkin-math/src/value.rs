//! What the calculator can arrive at.

use chrono::NaiveDate;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Money {
        amount: f64,
        currency: String,
    },
    Date(NaiveDate),
    /// Whole days. Weeks are seven of them; months and years are not a fixed number of
    /// days, so they are not units here.
    Duration {
        days: i64,
    },
}

/// Days a unit word is worth, or `None` if it is not a duration at all.
///
/// Danish and English both, because the interface is Danish and the keyboard is not.
pub fn duration_days(unit: &str) -> Option<i64> {
    match unit {
        "day" | "days" | "dag" | "dage" => Some(1),
        "week" | "weeks" | "uge" | "uger" => Some(7),
        _ => None,
    }
}

/// Whether some duration word starts with this.
///
/// Typing `7 d` is on the way to `7 dage`, and saying so would be rude.
pub fn is_duration_prefix(partial: &str) -> bool {
    const WORDS: [&str; 8] = ["day", "days", "dag", "dage", "week", "weeks", "uge", "uger"];

    WORDS.iter().any(|word| word.starts_with(partial))
}
