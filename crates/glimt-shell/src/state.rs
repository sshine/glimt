//! What the dashboard is showing, and what a key does to it.
//!
//! Both frontends own their own main loop and their own idea of a key event, and
//! neither owns any of this. They translate a key into an [`Action`], hand it over, and
//! do what the [`Response`] says. Everything between those two points is written once
//! and tested without a compositor, which is what makes maintaining two frontends
//! affordable.
//!
//! There are two levels, and they are told apart by whether anything has been typed.
//! Escape goes one level back: it clears the input, and at the top it dismisses the
//! window. Ctrl+Q goes all the way back, which means clearing the input and returning
//! the selection to today before dismissing. Neither ends the process; the daemon stays
//! resident, which is the whole point of it.

use chrono::{Local, NaiveDate};
use glimt_cal::{Overview, View, step};
use napkin_math::{Napkin, Outcome};

/// A week is seven days, and the only reason the distinction exists is that Ctrl makes
/// the arrow keys move by one.
const WEEK: i64 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// A printable character, which goes into the calculator wherever the selection is.
    Type(char),
    Backspace,
    /// One level back.
    Escape,
    /// All the way back, and out.
    Root,
    /// Move the selection by whole days.
    Days(i64),
    /// Move the selection by whole weeks.
    Weeks(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    /// Something changed on screen.
    Redraw,
    /// Unmap the window and keep everything else.
    Hide,
    /// The key meant nothing here.
    Ignored,
}

/// The answer still worth showing while the next one is being typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub text: String,
    pub note: Option<String>,
}

/// What the calculator bar draws.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar<'a> {
    pub input: &'a str,
    /// The last answer there was, which may be older than what is typed.
    pub answer: Option<&'a Answer>,
    /// The answer is not for what is on the line now, so it is drawn faded. This is the
    /// whole of how an unfinished expression is reported: by saying nothing and letting
    /// the previous answer go quiet.
    pub faded: bool,
    /// Whether the caret is showing this instant. It blinks while the window is up and
    /// is held solid while keys are arriving, the way a text field does.
    pub cursor: bool,
    /// An expression that parsed and could not be worked out. One quiet line, and only
    /// when there is genuinely something to say.
    pub problem: Option<&'a str>,
}

pub struct AppState {
    today: NaiveDate,
    selection: NaiveDate,
    months: u32,
    input: String,
    answer: Option<Answer>,
    problem: Option<String>,
    faded: bool,
    cursor: bool,
    /// The range a duration is pointing at, which the grid paints.
    highlight: Option<(NaiveDate, NaiveDate)>,
    overview: Overview,
    /// Absent when the Scheme runtime could not be built, which leaves a calendar that
    /// works and a calculator that says so.
    napkin: Option<Napkin>,
}

impl AppState {
    pub fn new(months: u32, napkin: Option<Napkin>) -> Self {
        Self::at(Local::now().date_naive(), months, napkin)
    }

    /// The same, on a day of the caller.s choosing, so a test does not depend on when
    /// it runs.
    pub fn at(today: NaiveDate, months: u32, napkin: Option<Napkin>) -> Self {
        Self {
            today,
            selection: today,
            months,
            input: String::new(),
            answer: None,
            problem: None,
            faded: false,
            cursor: true,
            highlight: None,
            overview: Overview::new(View::on(today), months),
            napkin,
        }
    }

    pub fn today(&self) -> NaiveDate {
        self.today
    }

    pub fn selection(&self) -> NaiveDate {
        self.selection
    }

    pub fn overview(&self) -> &Overview {
        &self.overview
    }

    pub fn bar(&self) -> Bar<'_> {
        Bar {
            input: &self.input,
            answer: self.answer.as_ref(),
            faded: self.faded,
            cursor: self.cursor,
            problem: self.problem.as_deref(),
        }
    }

    pub fn set_napkin(&mut self, napkin: Napkin) {
        self.napkin = Some(napkin);
        self.recompute();
    }

    /// Swap in a fresher rate table without disturbing what is on screen.
    pub fn with_napkin_mut(&mut self, change: impl FnOnce(&mut Napkin)) {
        if let Some(napkin) = self.napkin.as_mut() {
            change(napkin);
        }
        self.recompute();
    }

    /// The daemon outlives the day it started on. Called before each show.
    ///
    /// Returns whether anything moved, so a show that happens to cross midnight redraws
    /// and every other one does not.
    pub fn refresh_today(&mut self) -> bool {
        let now = Local::now().date_naive();

        if now == self.today {
            return false;
        }

        // The selection follows only when it was sitting on the old today, so a window
        // left open on next March is still on next March in the morning.
        if self.selection == self.today {
            self.selection = now;
        }

        self.today = now;
        self.rebuild();
        true
    }

    /// Flip the caret. Called on a timer while the window is up, and not while it is
    /// down: a hidden dashboard that wakes twice a second to blink at nobody would show
    /// up in the idle measurement as costing something.
    pub fn blink(&mut self) -> bool {
        self.cursor = !self.cursor;
        true
    }

    /// Hold the caret on, which is what a text field does while keys are arriving: a
    /// caret that blinks out mid-word reads as a dropped keystroke.
    fn hold_cursor(&mut self) {
        self.cursor = true;
    }

    pub fn apply(&mut self, action: Action) -> Response {
        match action {
            Action::Type(character) => {
                self.hold_cursor();
                self.input.push(character);
                self.recompute();
                Response::Redraw
            }

            Action::Backspace => {
                self.hold_cursor();

                if self.input.pop().is_none() {
                    return Response::Ignored;
                }
                self.recompute();
                Response::Redraw
            }

            // One level back: the input first, the window second.
            Action::Escape => {
                if self.input.is_empty() {
                    return Response::Hide;
                }
                self.clear();
                Response::Redraw
            }

            // All the way back. The selection returning to today is the only thing that
            // tells this apart from pressing Escape twice.
            Action::Root => {
                self.clear();
                self.selection = self.today;
                self.rebuild();
                Response::Hide
            }

            Action::Days(count) => self.move_selection(count),
            Action::Weeks(count) => self.move_selection(count * WEEK),
        }
    }

    fn move_selection(&mut self, days: i64) -> Response {
        let moved = step(self.selection, days);

        if moved == self.selection {
            return Response::Ignored;
        }

        self.selection = moved;
        self.rebuild();
        Response::Redraw
    }

    fn clear(&mut self) {
        self.input.clear();
        self.answer = None;
        self.problem = None;
        self.faded = false;
        self.highlight = None;
        self.rebuild();
    }

    /// Run what is typed, and decide what the bar says about it.
    fn recompute(&mut self) {
        let Some(napkin) = self.napkin.as_ref() else {
            self.problem = Some("regnemaskinen startede ikke".to_owned());
            return;
        };

        match napkin.evaluate(&self.input) {
            Outcome::Empty => {
                self.answer = None;
                self.problem = None;
                self.faded = false;
                self.highlight = None;
            }

            // The normal state of a half-typed expression. The previous answer stays and
            // goes quiet; nothing is said about the input.
            Outcome::Incomplete => {
                self.problem = None;
                self.faded = self.answer.is_some();
            }

            Outcome::Problem(message) => {
                self.problem = Some(message);
                self.faded = self.answer.is_some();
            }

            Outcome::Ok { text, note, span } => {
                self.answer = Some(Answer { text, note });
                self.problem = None;
                self.faded = false;
                self.highlight = span;
            }
        }

        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.overview = Overview::new(
            View::on(self.today)
                .with_selection(self.selection)
                .with_highlight(self.highlight),
            self.months,
        );
    }
}

/// Go to the ECB once now and once a day after, and hand each answer to the loop.
///
/// Runs on its own thread because it blocks on a socket, and because a dashboard that
/// cannot appear until a bank has answered is not a dashboard.
pub fn refresh_rates_forever(waker: std::sync::Arc<dyn crate::Waker>) {
    use std::time::Duration;

    // Reference rates change once per working day, so asking more often than this buys
    // nothing and asking less often means a Monday morning on Friday's numbers.
    const INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

    loop {
        let rates = napkin_math::Rates::refresh();
        tracing::info!("rates: {:?}, published {}", rates.source, rates.published);
        waker.wake(crate::Command::Rates(Box::new(rates)));

        std::thread::sleep(INTERVAL);
    }
}
