//! The calendar the dashboard draws, as data.
//!
//! Two views, both taken from ugenr.dk: a strip of the seven days of one week, and a run
//! of months laid out one per column with ISO week numbers down the Mondays.
//!
//! Nothing here knows about pixels. Both graphics spikes consume this crate, so a
//! difference between them is a difference between the toolkits rather than a difference
//! in what they were asked to draw.

use chrono::{Datelike, Days, Local, NaiveDate, Weekday};

pub mod holiday;
pub mod names;

pub use holiday::Holiday;

/// Where a day sits relative to today: the three colours of the week strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tense {
    Past,
    Today,
    Future,
}

impl Tense {
    fn of(date: NaiveDate, today: NaiveDate) -> Self {
        match date.cmp(&today) {
            std::cmp::Ordering::Less => Tense::Past,
            std::cmp::Ordering::Equal => Tense::Today,
            std::cmp::Ordering::Greater => Tense::Future,
        }
    }
}

/// What the renderers are looking at.
///
/// Today and the selection are separate because the keyboard moves one and not the
/// other: the three colours of the week strip keep meaning past, now and later, while
/// the strip itself follows wherever the arrow keys have gone. The highlight is a range
/// the calculator is pointing at, which is how a duration shows itself on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct View {
    pub today: NaiveDate,
    pub selection: NaiveDate,
    pub highlight: Option<(NaiveDate, NaiveDate)>,
}

impl View {
    /// Today and the selection on the same day with nothing highlighted, which is what
    /// the dashboard shows before a key is pressed.
    pub fn on(today: NaiveDate) -> Self {
        Self {
            today,
            selection: today,
            highlight: None,
        }
    }

    pub fn now() -> Self {
        Self::on(Local::now().date_naive())
    }

    pub fn with_selection(self, selection: NaiveDate) -> Self {
        Self { selection, ..self }
    }

    /// The ends are sorted, so a span may be given either way round.
    pub fn with_highlight(self, span: Option<(NaiveDate, NaiveDate)>) -> Self {
        Self {
            highlight: span.map(|(from, to)| if from <= to { (from, to) } else { (to, from) }),
            ..self
        }
    }

    fn highlights(&self, date: NaiveDate) -> bool {
        self.highlight
            .is_some_and(|(from, to)| date >= from && date <= to)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Day {
    pub date: NaiveDate,
    pub tense: Tense,
    pub weekend: bool,
    /// ISO week number, carried only on Mondays, which is where the reference prints it.
    pub week: Option<u32>,
    pub holiday: Option<Holiday>,
    /// Inside the week the strip shows, which the month columns outline.
    pub in_focus_week: bool,
    /// The day the keyboard is on.
    pub selected: bool,
    /// Inside the range the calculator is pointing at.
    pub highlighted: bool,
}

impl Day {
    fn new(date: NaiveDate, view: &View) -> Self {
        let weekday = date.weekday();

        Self {
            date,
            tense: Tense::of(date, view.today),
            weekend: matches!(weekday, Weekday::Sat | Weekday::Sun),
            week: (weekday == Weekday::Mon).then(|| date.iso_week().week()),
            holiday: holiday::holiday(date),
            in_focus_week: date.iso_week() == view.selection.iso_week(),
            selected: date == view.selection,
            highlighted: view.highlights(date),
        }
    }

    pub fn initial(&self) -> char {
        names::weekday_initial(self.date.weekday())
    }

    pub fn number(&self) -> u32 {
        self.date.day()
    }

    /// "mandag". Here so a frontend does not need chrono's `Datelike` in scope for the
    /// one thing it wants off the date.
    pub fn weekday_name(&self) -> &'static str {
        names::weekday(self.date.weekday())
    }

    /// Drawn with the Sunday background: weekends, and holidays that are public in the
    /// legal sense. Christmas Eve is named but not shaded, which is what Danish
    /// calendars do and what employment law says.
    pub fn shaded(&self) -> bool {
        self.weekend || self.holiday.is_some_and(|h| h.public)
    }
}

/// One week as seven days, with the heading drawn beside them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeekStrip {
    pub week: u32,
    /// ISO week-numbering year, which is not always the calendar year of the Monday.
    pub year: i32,
    /// "Uge 42".
    pub heading: String,
    /// Monday through Sunday. Seven entries unless the week runs off the end of the
    /// representable range, which chrono puts past the year 262143.
    pub days: Vec<Day>,
}

impl WeekStrip {
    /// The week containing the selection.
    pub fn new(view: View) -> Self {
        let iso = view.selection.iso_week();
        let back = Days::new(view.selection.weekday().num_days_from_monday() as u64);
        let monday = view
            .selection
            .checked_sub_days(back)
            .unwrap_or(view.selection);

        let days: Vec<Day> = (0..7)
            .filter_map(|offset| monday.checked_add_days(Days::new(offset)))
            .map(|date| Day::new(date, &view))
            .collect();

        Self {
            week: iso.week(),
            year: iso.year(),
            heading: format!("Uge {}", iso.week()),
            days,
        }
    }

    /// The day that is today, when the strip happens to contain it.
    pub fn today(&self) -> Option<&Day> {
        self.days.iter().find(|day| day.tense == Tense::Today)
    }

    pub fn selected(&self) -> Option<&Day> {
        self.days.iter().find(|day| day.selected)
    }
}

/// One month, as the reference lays it out: every day in a single column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthColumn {
    pub year: i32,
    pub month: u32,
    /// "Oktober 2026".
    pub heading: String,
    pub days: Vec<Day>,
}

impl MonthColumn {
    pub fn new(year: i32, month: u32, view: &View) -> Option<Self> {
        let first = NaiveDate::from_ymd_opt(year, month, 1)?;

        let mut days = Vec::with_capacity(31);
        let mut date = first;
        while date.month() == month && date.year() == year {
            days.push(Day::new(date, view));
            match date.succ_opt() {
                Some(next) => date = next,
                None => break,
            }
        }

        let name = names::month(month).unwrap_or_default();

        Some(Self {
            year,
            month,
            heading: format!("{} {year}", names::capitalize(name)),
            days,
        })
    }
}

/// Both views together, which is what a frontend is handed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overview {
    pub view: View,
    pub strip: WeekStrip,
    pub months: Vec<MonthColumn>,
}

impl Overview {
    /// `count` months, in the aligned block that contains the selection.
    ///
    /// Six months around a selection in October gives July through December, which is
    /// the half year the reference prints. The alignment generalises: three months give
    /// the quarter, twelve give the year. A count that does not divide the year has no
    /// block to align to, so the window simply starts at the selected month.
    ///
    /// Following the selection rather than today is what lets the arrow keys walk off
    /// the end of December: the grid redraws as the next half year rather than scrolling
    /// a month, so the columns always sit on a block boundary however they were reached.
    pub fn new(view: View, count: u32) -> Self {
        let anchor = view.selection;

        let start = if count > 0 && 12 % count == 0 {
            ((anchor.month() - 1) / count) * count + 1
        } else {
            anchor.month()
        };

        let months = (0..count)
            .filter_map(|offset| {
                let absolute = start + offset - 1;
                let year = anchor.year() + (absolute / 12) as i32;
                MonthColumn::new(year, absolute % 12 + 1, &view)
            })
            .collect();

        Self {
            view,
            strip: WeekStrip::new(view),
            months,
        }
    }

    /// The same, for the day the machine thinks it is.
    pub fn now(count: u32) -> Self {
        Self::new(View::now(), count)
    }
}

/// Move a date by whole days.
///
/// Saturating rather than wrapping: the arrow keys are held down, and running off the
/// end of what chrono can represent should stop rather than jump to the other end of
/// history.
pub fn step(date: NaiveDate, days: i64) -> NaiveDate {
    let magnitude = days.unsigned_abs();

    let moved = if days >= 0 {
        date.checked_add_days(Days::new(magnitude))
    } else {
        date.checked_sub_days(Days::new(magnitude))
    };

    moved.unwrap_or(date)
}
