//! The expression tree, lowered into something Scheme can take apart.
//!
//! The two things the Scheme side cannot know are resolved here: which words are
//! currencies, which is a question for the rate table, and what day it is. After this,
//! `13 usd` is `(money 13 usd)` and `today` is a date, and the prelude is a pure
//! function of its arguments.

use chrono::{Datelike, NaiveDate};
use scheme_rs::{
    lists::{self, Pair},
    symbols::Symbol,
    value::{UnpackedValue, Value},
};

use crate::parser::{Expr, Op};
use crate::rates::Rates;
use crate::value::duration_days;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    /// A word after a number that is neither a currency nor a duration.
    UnknownUnit(String),
    /// A word after `in` that is not a currency.
    UnknownCurrency(String),
}

impl LowerError {
    /// What the interface shows, in the language the rest of it uses.
    pub fn message(&self) -> String {
        match self {
            LowerError::UnknownUnit(unit) => format!("ukendt enhed: {unit}"),
            LowerError::UnknownCurrency(code) => format!("ukendt valuta: {code}"),
        }
    }
}

pub struct Lowered {
    pub datum: Value,
    /// True when the expression did not say what to convert to, which is the only time
    /// the calculator picks a target itself.
    pub implicit: bool,
}

pub fn symbol(name: &str) -> Value {
    UnpackedValue::Symbol(Symbol::intern(name)).into_value()
}

fn pair(car: Value, cdr: Value) -> Value {
    UnpackedValue::Pair(Pair::immutable(car, cdr)).into_value()
}

/// The rate table as an alist Scheme can look through: `((usd . 1.1253) ...)`.
pub fn rates_alist(rates: &Rates) -> Value {
    let entries: Vec<Value> = rates
        .codes()
        .map(|(code, rate)| pair(symbol(code), Value::from(rate)))
        .collect();

    lists::slice_to_list(&entries)
}

pub fn lower(expr: &Expr, today: NaiveDate, rates: &Rates) -> Result<Lowered, LowerError> {
    Ok(Lowered {
        datum: lower_expr(expr, today, rates)?,
        implicit: !matches!(expr, Expr::Convert { .. }),
    })
}

fn lower_expr(expr: &Expr, today: NaiveDate, rates: &Rates) -> Result<Value, LowerError> {
    match expr {
        Expr::Number(n) => Ok(Value::from(*n)),

        Expr::Date(date) => Ok(date_datum(*date)),
        Expr::Today => Ok(date_datum(today)),

        Expr::Unit { value, unit } => {
            let inner = lower_expr(value, today, rates)?;

            if rates.is_currency(unit) {
                Ok(lists::slice_to_list(&[
                    symbol("money"),
                    inner,
                    symbol(&rates.code(unit)),
                ]))
            } else if let Some(days) = duration_days(unit) {
                Ok(lists::slice_to_list(&[
                    symbol("duration"),
                    inner,
                    Value::from(days),
                ]))
            } else {
                Err(LowerError::UnknownUnit(unit.clone()))
            }
        }

        Expr::Neg(inner) => Ok(lists::slice_to_list(&[
            symbol("neg"),
            lower_expr(inner, today, rates)?,
        ])),

        Expr::Binary { op, lhs, rhs } => Ok(lists::slice_to_list(&[
            symbol(operator(op)),
            lower_expr(lhs, today, rates)?,
            lower_expr(rhs, today, rates)?,
        ])),

        Expr::Convert { value, to } => {
            if !rates.is_currency(to) {
                return Err(LowerError::UnknownCurrency(to.clone()));
            }

            Ok(lists::slice_to_list(&[
                symbol("convert"),
                lower_expr(value, today, rates)?,
                symbol(&rates.code(to)),
            ]))
        }
    }
}

/// Dates travel as days since the start of the common era, which is one integer and
/// needs no calendar on the Scheme side.
fn date_datum(date: NaiveDate) -> Value {
    lists::slice_to_list(&[
        symbol("date"),
        Value::from(i64::from(date.num_days_from_ce())),
    ])
}

fn operator(op: &Op) -> &'static str {
    match op {
        Op::Add => "+",
        Op::Sub => "-",
        Op::Mul => "*",
        Op::Div => "/",
    }
}
