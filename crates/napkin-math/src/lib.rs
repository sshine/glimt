//! A napkin calculator.
//!
//! Infix arithmetic, currency conversion and date arithmetic, parsed in Rust and
//! evaluated in Scheme. The split is deliberate: the parser is a pure function of a
//! string and is tested as a table, and the semantics live in `prelude.scm` where adding
//! another kind of napkin calculation means writing a few lines of Scheme rather than
//! another match arm.
//!
//! ```text
//! 2 + 3 * 4            -> 14
//! 13 usd in dkk        -> 86,34 DKK
//! 13 usd               -> 86,34 DKK      the target is implied
//! 13 dkk               ->  1,74 EUR      kroner cannot be converted to kroner
//! today + 7 days       -> a date, and eight days painted on the calendar
//! 2026-10-19 + 30 days -> onsdag 18. november 2026
//! ```
//!
//! Everything is evaluated on every keystroke, so most of what this sees is a prefix of
//! something valid. That is reported as [`Outcome::Incomplete`], which the interface
//! shows by saying nothing at all.

use chrono::{Local, NaiveDate};

pub mod engine;
pub mod format;
pub mod lexer;
pub mod parser;
pub mod rates;
pub mod sexp;
pub mod span;
pub mod value;

pub use engine::{Engine, EngineError};
pub use parser::{Expr, ParseError, parse};
pub use rates::{Rates, Source};
pub use value::Value;

/// What to show for what has been typed.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Nothing typed.
    Empty,
    /// Not an expression yet. Every valid expression passes through a dozen of these on
    /// the way to being typed, so this is not a mistake and is not reported as one: the
    /// interface keeps showing the previous answer and waits.
    Incomplete,
    /// An expression that cannot be worked out, which is worth one quiet line.
    Problem(String),
    Ok {
        /// The answer, formatted.
        text: String,
        /// Where the rates came from, when the answer depended on them.
        note: Option<String>,
        /// The days the calendar should paint.
        span: Option<(NaiveDate, NaiveDate)>,
    },
}

impl Outcome {
    pub fn span(&self) -> Option<(NaiveDate, NaiveDate)> {
        match self {
            Outcome::Ok { span, .. } => *span,
            _ => None,
        }
    }
}

pub struct Napkin {
    engine: Engine,
    rates: Rates,
    today: NaiveDate,
}

impl Napkin {
    /// Build the Scheme runtime and compile the prelude, with whatever rates are on hand
    /// without going to the network. Around 60 ms, so not on a path anyone is waiting on.
    pub fn new() -> Result<Self, EngineError> {
        Ok(Self {
            engine: Engine::new()?,
            rates: Rates::offline(),
            today: Local::now().date_naive(),
        })
    }

    pub fn with_rates(rates: Rates) -> Result<Self, EngineError> {
        Ok(Self {
            engine: Engine::new()?,
            rates,
            today: Local::now().date_naive(),
        })
    }

    /// Swap in a fresher table, which the refresh thread does when the ECB answers.
    pub fn set_rates(&mut self, rates: Rates) {
        self.rates = rates;
    }

    /// Pin the day, so a test does not depend on when it runs.
    pub fn set_today(&mut self, today: NaiveDate) {
        self.today = today;
    }

    pub fn rates(&self) -> &Rates {
        &self.rates
    }

    pub fn evaluate(&self, input: &str) -> Outcome {
        let expr = match parse(input) {
            Ok(expr) => expr,
            Err(ParseError::Empty) => return Outcome::Empty,
            Err(_) => return Outcome::Incomplete,
        };

        let lowered = match sexp::lower(&expr, self.today, &self.rates) {
            Ok(lowered) => lowered,
            Err(error) if self.still_being_typed(&error) => return Outcome::Incomplete,
            Err(error) => return Outcome::Problem(error.message()),
        };

        let value = match self.engine.call(
            lowered.datum,
            sexp::rates_alist(&self.rates),
            lowered.implicit,
        ) {
            engine::Answer::Value(value) => value,
            engine::Answer::Pending => return Outcome::Incomplete,
            engine::Answer::Problem(message) => return Outcome::Problem(message),
        };

        Outcome::Ok {
            text: format::value(&value),
            note: matches!(value, Value::Money { .. }).then(|| self.note()),
            span: span::of(&expr, &value, self.today),
        }
    }

    /// Whether an unknown word could still become a known one.
    ///
    /// `13 u` is three keystrokes from `13 usd`, and a calculator that evaluates while
    /// you type sees that state far more often than it sees a finished expression. A
    /// word that is a prefix of something real is treated as unfinished rather than
    /// wrong, which is the difference between a calculator and a nag.
    fn still_being_typed(&self, error: &sexp::LowerError) -> bool {
        match error {
            sexp::LowerError::UnknownUnit(word) => {
                self.rates.has_prefix(word) || value::is_duration_prefix(word)
            }
            sexp::LowerError::UnknownCurrency(word) => self.rates.has_prefix(word),
        }
    }

    /// Reference rates are not a bank.s rates, and a cached table is not today's, so an
    /// amount says where its number came from.
    fn note(&self) -> String {
        let day = self.rates.published;

        match self.rates.source {
            Source::Feed | Source::Cache => format!("ECB {day}"),
            Source::Baked => format!("ECB {day}, indbygget"),
        }
    }
}
