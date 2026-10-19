//! The Scheme runtime, and the one procedure it is kept alive for.
//!
//! Building the runtime and importing the standard library costs around 60 ms, and
//! compiling the prelude costs a millisecond more. Both happen once, when the daemon
//! starts. Calling the compiled procedure afterwards costs about two microseconds, which
//! is why the expression is handed over as data rather than as source: evaluating a
//! string would JIT compile something new on every keystroke.

use chrono::NaiveDate;
use scheme_rs::{
    env::TopLevelEnvironment,
    lists,
    proc::{ContBarrier, Procedure},
    runtime::Runtime,
    value::{UnpackedValue, Value as SchemeValue},
};

use crate::value::Value;

const PRELUDE: &str = include_str!("prelude.scm");

#[derive(Debug)]
pub enum EngineError {
    Library(String),
    Prelude(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::Library(detail) => write!(f, "could not import (rnrs): {detail}"),
            EngineError::Prelude(detail) => write!(f, "could not compile the prelude: {detail}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// What the prelude can answer with.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Value(Value),
    /// A combination more typing can still make sense of, such as a number beside a
    /// date. Shown the same way an unfinished expression is, which is not at all.
    Pending,
    Problem(String),
}

pub struct Engine {
    /// Held because it owns the memory the compiled procedure lives in.
    _runtime: Runtime,
    evaluate: Procedure,
}

impl Engine {
    pub fn new() -> Result<Self, EngineError> {
        let runtime = Runtime::new();
        let env = TopLevelEnvironment::new_repl(&runtime);

        let library = "(library (rnrs))"
            .parse()
            .map_err(|error| EngineError::Library(format!("{error:?}")))?;
        env.import(library)
            .map_err(|error| EngineError::Library(format!("{error:?}")))?;

        let values = env
            .eval(false, PRELUDE)
            .map_err(|error| EngineError::Prelude(format!("{error:?}")))?;

        let evaluate = values
            .last()
            .and_then(|value| value.clone().cast_to_scheme_type::<Procedure>())
            .ok_or_else(|| EngineError::Prelude("the prelude did not end in a procedure".into()))?;

        Ok(Self {
            _runtime: runtime,
            evaluate,
        })
    }

    /// Run one expression.
    pub fn call(&self, datum: SchemeValue, rates: SchemeValue, implicit: bool) -> Answer {
        let returned = match self.evaluate.call(
            &[datum, rates, SchemeValue::from(implicit)],
            &mut ContBarrier::new(),
        ) {
            Ok(returned) => returned,
            Err(error) => return Answer::Problem(format!("{error:?}")),
        };

        match returned.first() {
            Some(result) => read_result(result),
            None => Answer::Problem("intet svar".to_owned()),
        }
    }
}

/// `(ok value)`, `(pending)` or `(err "besked")`.
fn read_result(value: &SchemeValue) -> Answer {
    let Some(parts) = as_list(value) else {
        return Answer::Problem("uforståeligt svar".to_owned());
    };

    match parts.as_slice() {
        [tag] if symbol_is(tag, "pending") => Answer::Pending,
        [tag, payload] if symbol_is(tag, "ok") => match read_value(payload) {
            Some(value) => Answer::Value(value),
            None => Answer::Problem("uforståelig værdi".to_owned()),
        },
        [tag, message] if symbol_is(tag, "err") => {
            Answer::Problem(as_string(message).unwrap_or_else(|| "regnefejl".to_owned()))
        }
        _ => Answer::Problem("uforståeligt svar".to_owned()),
    }
}

fn read_value(value: &SchemeValue) -> Option<Value> {
    if let Some(number) = as_number(value) {
        return Some(Value::Number(number));
    }

    let parts = as_list(value)?;
    let tag = parts.first()?;

    if symbol_is(tag, "money") {
        return Some(Value::Money {
            amount: as_number(parts.get(1)?)?,
            currency: as_symbol(parts.get(2)?)?,
        });
    }

    if symbol_is(tag, "date") {
        let days = as_number(parts.get(1)?)?.round();
        // Outside what chrono can hold, which arithmetic on a far enough date reaches.
        return NaiveDate::from_num_days_from_ce_opt(days as i32).map(Value::Date);
    }

    if symbol_is(tag, "duration") {
        return Some(Value::Duration {
            days: as_number(parts.get(1)?)?.round() as i64,
        });
    }

    None
}

fn as_list(value: &SchemeValue) -> Option<Vec<SchemeValue>> {
    if !matches!(value.clone().unpack(), UnpackedValue::Pair(_)) {
        return None;
    }

    let mut out = Vec::new();
    lists::list_to_vec(value, &mut out);
    Some(out)
}

fn as_number(value: &SchemeValue) -> Option<f64> {
    matches!(value.clone().unpack(), UnpackedValue::Number(_))
        .then(|| value.clone().try_into().ok())
        .flatten()
}

fn as_symbol(value: &SchemeValue) -> Option<String> {
    match value.clone().unpack() {
        UnpackedValue::Symbol(symbol) => Some(symbol.to_str().to_string()),
        _ => None,
    }
}

fn as_string(value: &SchemeValue) -> Option<String> {
    matches!(value.clone().unpack(), UnpackedValue::String(_))
        .then(|| value.clone().try_into().ok())
        .flatten()
}

fn symbol_is(value: &SchemeValue, name: &str) -> bool {
    as_symbol(value).is_some_and(|symbol| symbol == name)
}
