//! Danish holidays, including the movable ones.
//!
//! The reference calendar prints a holiday's name beside its day and shades the public
//! ones like a Sunday, so the model has to distinguish the two: Christmas Eve and New
//! Year's Eve are named but are ordinary working days in law, and so is Constitution
//! Day. Easter decides ten of the entries, which is why this is computed rather than
//! listed.

use chrono::{Datelike, Days, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Holiday {
    pub name: &'static str,
    /// A public holiday in the legal sense, which is what shading follows. The named
    /// working days are false.
    pub public: bool,
}

/// Easter Sunday, by the anonymous Gregorian algorithm.
///
/// `None` only for years whose March and April cannot be represented, which chrono
/// bounds far outside any year this calendar will be asked about.
pub fn easter(year: i32) -> Option<NaiveDate> {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;

    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
}

/// The holiday falling on `date`, if any.
pub fn holiday(date: NaiveDate) -> Option<Holiday> {
    const FIXED: [(u32, u32, &str, bool); 5] = [
        (1, 1, "Nytårsdag", true),
        (6, 5, "Grundlovsdag", false),
        (12, 24, "Juleaften", false),
        (12, 25, "Juledag", true),
        (12, 26, "2. juledag", true),
    ];

    for (month, day, name, public) in FIXED {
        if date.month() == month && date.day() == day {
            return Some(Holiday { name, public });
        }
    }

    // New Year's Eve is last so that a year ending on a named day still matches above.
    if date.month() == 12 && date.day() == 31 {
        return Some(Holiday {
            name: "Nytårsaften",
            public: false,
        });
    }

    movable(date)
}

fn movable(date: NaiveDate) -> Option<Holiday> {
    let easter = easter(date.year())?;
    let offset = date.signed_duration_since(easter).num_days();

    let (name, public) = match offset {
        -7 => ("Palmesøndag", false),
        -3 => ("Skærtorsdag", true),
        -2 => ("Langfredag", true),
        0 => ("Påskedag", true),
        1 => ("2. påskedag", true),
        // Store bededag stopped being a public holiday in 2024. Years before that are
        // still rendered correctly, which is the point of computing it rather than
        // dropping it.
        26 if date.year() < 2024 => ("Store bededag", true),
        39 => ("Kristi himmelfartsdag", true),
        49 => ("Pinsedag", true),
        50 => ("2. pinsedag", true),
        _ => return None,
    };

    Some(Holiday { name, public })
}

/// Easter plus `days`, for callers that want the movable dates directly.
pub fn after_easter(year: i32, days: u64) -> Option<NaiveDate> {
    easter(year)?.checked_add_days(Days::new(days))
}
