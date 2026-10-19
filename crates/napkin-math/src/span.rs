//! The stretch of calendar a result is talking about.
//!
//! A duration on its own means nothing on a grid until it is anchored, so the anchor is
//! taken from the expression: whichever date was written in it, or today when none was.
//! `today + 7 days` paints the eight days from today; `2026-10-19 + 30 days` paints from
//! the nineteenth to the eighteenth of November; subtracting two dates paints between
//! them.

use chrono::NaiveDate;
use glimt_cal::step;

use crate::parser::Expr;
use crate::value::Value;

pub fn of(expr: &Expr, value: &Value, today: NaiveDate) -> Option<(NaiveDate, NaiveDate)> {
    let mut written = Vec::new();
    collect(expr, today, &mut written);

    match value {
        Value::Date(end) => {
            let start = written.first().copied().unwrap_or(*end);
            Some(order(start, *end))
        }

        Value::Duration { days } => match written.as_slice() {
            // Nothing was written, so the duration is counted from today.
            [] => Some(order(today, step(today, *days))),
            // One date was written, so it is the end the duration hangs from.
            [anchor] => Some(order(*anchor, step(*anchor, *days))),
            // Two or more, so the span is between the outermost of them, which is what
            // subtracting one date from another is asking about.
            dates => {
                let first = dates.iter().min().copied().unwrap_or(today);
                let last = dates.iter().max().copied().unwrap_or(today);
                Some(order(first, last))
            }
        },

        _ => None,
    }
}

fn order(a: NaiveDate, b: NaiveDate) -> (NaiveDate, NaiveDate) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Every date the expression names, in the order written.
fn collect(expr: &Expr, today: NaiveDate, out: &mut Vec<NaiveDate>) {
    match expr {
        Expr::Today => out.push(today),
        Expr::Date(date) => out.push(*date),
        Expr::Number(_) => {}
        Expr::Unit { value, .. } | Expr::Neg(value) | Expr::Convert { value, .. } => {
            collect(value, today, out);
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect(lhs, today, out);
            collect(rhs, today, out);
        }
    }
}
