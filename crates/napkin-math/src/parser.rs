//! Tokens into an expression tree.
//!
//! The parser knows the shape of the language and nothing about what the words mean: it
//! will happily parse `13 bogus`, and deciding that `bogus` is neither a currency nor a
//! duration happens later, where the rate table is. That keeps this a pure function of
//! its input, which is what makes the test suite a table.
//!
//! The distinction the interface depends on is between an expression that has run out of
//! input and one that is wrong. Everything a person types passes through a dozen
//! incomplete states on its way to being valid, and those must not be reported as
//! mistakes.

use chrono::NaiveDate;

use crate::lexer::{self, LexError, Spanned, Token};

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Date(NaiveDate),
    Today,
    /// A number with a word after it: `13 usd`, `7 days`.
    Unit {
        value: Box<Expr>,
        unit: String,
    },
    Neg(Box<Expr>),
    Binary {
        op: Op,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// The `in dkk` of `13 usd in dkk`.
    Convert {
        value: Box<Expr>,
        to: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Nothing to parse at all.
    Empty,
    /// The input stopped partway through something that could still become valid. This
    /// is the state of every expression while it is being typed, so it is not an error
    /// anyone should be told about.
    Incomplete,
    /// Something that no amount of further typing will fix.
    Unexpected {
        at: usize,
        found: String,
    },
    /// Well-formed digits that are not a day on the calendar, like 2026-02-30.
    BadDate {
        at: usize,
    },
    Unlexable {
        at: usize,
        character: char,
    },
}

impl From<LexError> for ParseError {
    fn from(error: LexError) -> Self {
        match error {
            LexError::Unexpected { at, character } => ParseError::Unlexable { at, character },
            LexError::Incomplete => ParseError::Incomplete,
        }
    }
}

/// The word that turns an expression into a conversion.
const IN: &str = "in";
const TODAY: &str = "today";
/// Danish for today, since the rest of the interface is in Danish.
const TODAY_DA: &str = "idag";

pub fn parse(input: &str) -> Result<Expr, ParseError> {
    let tokens = lexer::lex(input)?;

    if tokens.is_empty() {
        return Err(ParseError::Empty);
    }

    let mut parser = Parser {
        tokens: &tokens,
        index: 0,
        end: input.chars().count(),
    };

    let value = parser.expression()?;

    let expr = if parser.take_word(IN) {
        let to = parser.unit_word()?;
        Expr::Convert {
            value: Box::new(value),
            to,
        }
    } else {
        value
    };

    match parser.peek() {
        None => Ok(expr),

        // A word left over after a complete expression is usually a mistake, but not
        // while it is still on its way to being `in`: `13 usd i` is one keystroke from
        // `13 usd in`, and the value already had its unit taken.
        Some(extra) => match &extra.token {
            Token::Word(word) if IN.starts_with(word.as_str()) => Err(ParseError::Incomplete),
            other => Err(ParseError::Unexpected {
                at: extra.at,
                found: other.to_string(),
            }),
        },
    }
}

struct Parser<'a> {
    tokens: &'a [Spanned],
    index: usize,
    end: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Spanned> {
        self.tokens.get(self.index)
    }

    fn advance(&mut self) -> Option<&Spanned> {
        let next = self.tokens.get(self.index);
        if next.is_some() {
            self.index += 1;
        }
        next
    }

    /// Consume the given bare word, if that is what comes next.
    fn take_word(&mut self, word: &str) -> bool {
        let matched = matches!(
            self.peek().map(|spanned| &spanned.token),
            Some(Token::Word(found)) if found == word
        );

        if matched {
            self.index += 1;
        }

        matched
    }

    fn take_operator(&mut self, wanted: &[(Token, Op)]) -> Option<Op> {
        let found = self.peek()?;
        let op = wanted
            .iter()
            .find(|(token, _)| *token == found.token)
            .map(|(_, op)| op.clone())?;

        self.index += 1;
        Some(op)
    }

    fn expression(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.term()?;

        while let Some(op) = self.take_operator(&[(Token::Plus, Op::Add), (Token::Minus, Op::Sub)])
        {
            let rhs = self.term()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn term(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.factor()?;

        while let Some(op) = self.take_operator(&[(Token::Star, Op::Mul), (Token::Slash, Op::Div)])
        {
            let rhs = self.factor()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn factor(&mut self) -> Result<Expr, ParseError> {
        let negated = match self.peek().map(|spanned| &spanned.token) {
            Some(Token::Minus) => {
                self.index += 1;
                true
            }
            Some(Token::Plus) => {
                self.index += 1;
                false
            }
            _ => false,
        };

        let atom = self.atom()?;

        // A bare word after a value is its unit, unless it is the `in` that introduces a
        // conversion.
        let value = match self.peek().map(|spanned| &spanned.token) {
            Some(Token::Word(word)) if word != IN => {
                let unit = word.clone();
                self.index += 1;
                Expr::Unit {
                    value: Box::new(atom),
                    unit,
                }
            }
            _ => atom,
        };

        Ok(if negated {
            Expr::Neg(Box::new(value))
        } else {
            value
        })
    }

    fn atom(&mut self) -> Result<Expr, ParseError> {
        let Some(spanned) = self.advance() else {
            return Err(ParseError::Incomplete);
        };

        let at = spanned.at;

        match &spanned.token {
            Token::Number(n) => Ok(Expr::Number(*n)),

            Token::Date(year, month, day) => NaiveDate::from_ymd_opt(*year, *month, *day)
                .map(Expr::Date)
                .ok_or(ParseError::BadDate { at }),

            Token::Word(word) if word == TODAY || word == TODAY_DA => Ok(Expr::Today),

            // A word on its way to being `today`.
            Token::Word(word)
                if TODAY.starts_with(word.as_str()) || TODAY_DA.starts_with(word.as_str()) =>
            {
                Err(ParseError::Incomplete)
            }

            // Any other word is one used where a value belongs, which no amount of
            // further typing turns into an expression.
            Token::Word(word) => Err(ParseError::Unexpected {
                at,
                found: word.clone(),
            }),

            Token::Open => {
                let inner = self.expression()?;
                if self.peek().is_none() {
                    return Err(ParseError::Incomplete);
                }
                match self.advance().map(|spanned| &spanned.token) {
                    Some(Token::Close) => Ok(inner),
                    _ => Err(ParseError::Unexpected {
                        at,
                        found: ")".to_owned(),
                    }),
                }
            }

            other => Err(ParseError::Unexpected {
                at,
                found: other.to_string(),
            }),
        }
    }

    /// The word after `in`, which has to be there.
    fn unit_word(&mut self) -> Result<String, ParseError> {
        let end = self.end;

        match self.advance() {
            None => Err(ParseError::Incomplete),
            Some(spanned) => match &spanned.token {
                Token::Word(word) => Ok(word.clone()),
                other => Err(ParseError::Unexpected {
                    at: spanned.at.min(end),
                    found: other.to_string(),
                }),
            },
        }
    }
}
