//! Input into tokens.
//!
//! Whitespace separates tokens and is otherwise ignored, so `2+3*4` and
//! `  2  +  3  *  4  ` reach the parser identically. The one place spacing matters is
//! inside a date, because `2026-10-19` is a single token while `2026 - 10 - 19` is three
//! subtractions, and both readings are useful.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    /// A bare word: a currency, a unit, `today`, or `in`.
    Word(String),
    Date(i32, u32, u32),
    Plus,
    Minus,
    Star,
    Slash,
    Open,
    Close,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Number(n) => write!(f, "{n}"),
            Token::Word(w) => write!(f, "{w}"),
            Token::Date(y, m, d) => write!(f, "{y:04}-{m:02}-{d:02}"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Open => write!(f, "("),
            Token::Close => write!(f, ")"),
        }
    }
}

/// A token and where it started, so an error can say where it went wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub at: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    /// A character the language has no use for.
    Unexpected { at: usize, character: char },
    /// The input stopped in the middle of a token: a number whose decimal separator has
    /// no digits after it yet, which is every decimal halfway through being typed.
    Incomplete,
}

pub fn lex(input: &str) -> Result<Vec<Spanned>, LexError> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < chars.len() {
        let at = index;
        let current = chars[index];

        if current.is_whitespace() {
            index += 1;
            continue;
        }

        let token = match current {
            '+' => single(&mut index, Token::Plus),
            '-' => single(&mut index, Token::Minus),
            '*' => single(&mut index, Token::Star),
            '/' => single(&mut index, Token::Slash),
            '(' => single(&mut index, Token::Open),
            ')' => single(&mut index, Token::Close),
            c if c.is_ascii_digit() => number_or_date(&chars, &mut index),
            c if c.is_alphabetic() || c == '_' => word(&chars, &mut index),
            // A decimal separator with nothing after it yet. At the end of the input
            // that is a half-typed number rather than a stray character.
            sep @ ('.' | ',') if index + 1 == chars.len() => {
                let _ = sep;
                return Err(LexError::Incomplete);
            }

            c => return Err(LexError::Unexpected { at, character: c }),
        };

        tokens.push(Spanned { token, at });
    }

    Ok(tokens)
}

fn single(index: &mut usize, token: Token) -> Token {
    *index += 1;
    token
}

/// A date when the digits have the shape `dddd-dd-dd`, otherwise a number.
///
/// Decided by looking ahead rather than by backtracking, so a year followed by a
/// subtraction is never mistaken for half a date.
fn number_or_date(chars: &[char], index: &mut usize) -> Token {
    if let Some(date) = date_at(chars, *index) {
        *index += 10;
        return date;
    }

    let start = *index;
    while *index < chars.len() && chars[*index].is_ascii_digit() {
        *index += 1;
    }

    // A comma is a decimal point here. Danish writes 13,5 and the grammar has nothing
    // else a comma could mean.
    if *index < chars.len()
        && matches!(chars[*index], '.' | ',')
        && chars.get(*index + 1).is_some_and(char::is_ascii_digit)
    {
        *index += 1;
        while *index < chars.len() && chars[*index].is_ascii_digit() {
            *index += 1;
        }
    }

    let text: String = chars[start..*index]
        .iter()
        .map(|c| if *c == ',' { '.' } else { *c })
        .collect();

    Token::Number(text.parse().unwrap_or(f64::NAN))
}

fn date_at(chars: &[char], start: usize) -> Option<Token> {
    let digits = |from: usize, count: usize| -> Option<u32> {
        let slice = chars.get(from..from + count)?;
        if !slice.iter().all(char::is_ascii_digit) {
            return None;
        }
        slice.iter().collect::<String>().parse().ok()
    };

    if chars.get(start + 4) != Some(&'-') || chars.get(start + 7) != Some(&'-') {
        return None;
    }

    // Not a date if more digits follow, which keeps 2026-10-199 an error rather than a
    // date with something stuck to it.
    if chars.get(start + 10).is_some_and(char::is_ascii_digit) {
        return None;
    }

    Some(Token::Date(
        digits(start, 4)? as i32,
        digits(start + 5, 2)?,
        digits(start + 8, 2)?,
    ))
}

fn word(chars: &[char], index: &mut usize) -> Token {
    let start = *index;

    while *index < chars.len() && (chars[*index].is_alphanumeric() || chars[*index] == '_') {
        *index += 1;
    }

    Token::Word(
        chars[start..*index]
            .iter()
            .collect::<String>()
            .to_lowercase(),
    )
}
