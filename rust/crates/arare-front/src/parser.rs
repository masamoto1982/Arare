//! Parsing (AR-LLR-012): brackets into quotes, names into Words, integer
//! literals into integers, and every decimal, fraction or wheel point,
//! together with the `<width> <exponent> QUANTIZE` that must follow it, into
//! one quantized number (AR-HLR-018).

use crate::lexer::{lex, Token, TokenKind};
use crate::{FrontError, FrontErrorKind};
use arare_kernel::{Num, Width, MAX_SCALE};

/// The vocabulary of Phase 2 (WORK_ORDER.md §7).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Word {
    Quantize,
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Rescale,
    Narrow,
    Eq,
    Lt,
    And,
    Not,
    Filter,
    Map,
    Fold,
    Sum,
    Select,
    True,
    False,
    Nil,
}

impl Word {
    pub const ALL: [Word; 20] = [
        Word::Quantize,
        Word::Add,
        Word::Sub,
        Word::Mul,
        Word::Div,
        Word::Neg,
        Word::Rescale,
        Word::Narrow,
        Word::Eq,
        Word::Lt,
        Word::And,
        Word::Not,
        Word::Filter,
        Word::Map,
        Word::Fold,
        Word::Sum,
        Word::Select,
        Word::True,
        Word::False,
        Word::Nil,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Word::Quantize => "QUANTIZE",
            Word::Add => "ADD",
            Word::Sub => "SUB",
            Word::Mul => "MUL",
            Word::Div => "DIV",
            Word::Neg => "NEG",
            Word::Rescale => "RESCALE",
            Word::Narrow => "NARROW",
            Word::Eq => "EQ",
            Word::Lt => "LT",
            Word::And => "AND",
            Word::Not => "NOT",
            Word::Filter => "FILTER",
            Word::Map => "MAP",
            Word::Fold => "FOLD",
            Word::Sum => "SUM",
            Word::Select => "SELECT",
            Word::True => "TRUE",
            Word::False => "FALSE",
            Word::Nil => "NIL",
        }
    }

    pub fn from_name(name: &str) -> Option<Word> {
        Word::ALL.into_iter().find(|w| w.name() == name)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Item {
    /// An integer literal: scale 0, the narrowest width that holds it.
    Int(i64),
    /// A quantized literal, already folded, with the exact rational `p/q`
    /// it was written as (`q ≥ 0`; `q = 0` for a wheel point), which the
    /// error-budget checker measures the quantization against (AR-LLR-020).
    Num {
        num: Num,
        exact: (i64, i64),
    },
    Word(Word),
    /// A bracketed sequence: data or code, by what consumes it.
    Quote(Vec<Item>),
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Program {
    pub items: Vec<Item>,
}

fn error(t: &Token, kind: FrontErrorKind) -> FrontError {
    FrontError {
        kind,
        line: t.line,
        column: t.column,
    }
}

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 {
        a.abs()
    } else {
        gcd(b, a % b)
    }
}

/// `digits` as an `i128`, `None` past its range.
fn natural(digits: &str) -> Option<i128> {
    digits.bytes().try_fold(0i128, |n, b| {
        n.checked_mul(10)?.checked_add((b - b'0') as i128)
    })
}

/// A decimal or fraction as `(p, q)` in lowest terms with `q ≥ 0`, each
/// within 64 bits; `q = 0` for a wheel point.
fn ratio(t: &Token) -> Result<(i64, i64), FrontError> {
    let too_precise = || error(t, FrontErrorKind::LiteralTooPrecise(t.text.clone()));
    let (negative, body) = match t.text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.text.as_str()),
    };
    let (p, q) = match t.kind {
        TokenKind::Decimal => {
            let (whole, frac) = body.split_once('.').expect("a decimal has a point");
            let p = natural(&format!("{whole}{frac}")).ok_or_else(too_precise)?;
            let q = 10i128
                .checked_pow(frac.len() as u32)
                .ok_or_else(too_precise)?;
            (p, q)
        }
        _ => {
            let (n, d) = body.split_once('/').expect("a fraction has a slash");
            (
                natural(n).ok_or_else(too_precise)?,
                natural(d).ok_or_else(too_precise)?,
            )
        }
    };
    let g = gcd(p, q).max(1);
    let (p, q) = if negative {
        (-p / g, q / g)
    } else {
        (p / g, q / g)
    };
    Ok((
        i64::try_from(p).map_err(|_| too_precise())?,
        i64::try_from(q).map_err(|_| too_precise())?,
    ))
}

fn width_of(bits: &str) -> Option<Width> {
    Width::ALL
        .into_iter()
        .find(|w| w.bits().to_string() == bits)
}

/// One bracket level of tokens, from `tokens[*at]` to its closing `]` (or
/// the end, at the top level).
fn sequence(tokens: &[Token], at: &mut usize, top: bool) -> Result<Vec<Item>, FrontError> {
    let mut items = Vec::new();
    while *at < tokens.len() {
        let t = &tokens[*at];
        *at += 1;
        match t.kind {
            TokenKind::Open => items.push(Item::Quote(sequence(tokens, at, false)?)),
            TokenKind::Close if top => return Err(error(t, FrontErrorKind::UnexpectedClose)),
            TokenKind::Close => return Ok(items),
            TokenKind::Integer => {
                let k = t
                    .text
                    .parse::<i64>()
                    .ok()
                    .filter(|&k| Num::int(k).is_some());
                items.push(Item::Int(k.ok_or_else(|| {
                    error(t, FrontErrorKind::IntegerOutOfRange(t.text.clone()))
                })?));
            }
            TokenKind::Decimal | TokenKind::Fraction => items.push(quantized(tokens, at, t)?),
            TokenKind::Name => match Word::from_name(&t.text) {
                Some(w) => items.push(Item::Word(w)),
                None => return Err(error(t, FrontErrorKind::UnknownWord(t.text.clone()))),
            },
        }
    }
    if top {
        Ok(items)
    } else {
        Err(error(
            tokens.last().expect("an open bracket was read"),
            FrontErrorKind::UnclosedQuote,
        ))
    }
}

/// A non-integer literal and the `<width> <exponent> QUANTIZE` after it.
fn quantized(tokens: &[Token], at: &mut usize, t: &Token) -> Result<Item, FrontError> {
    let (p, q) = ratio(t)?;
    let rest = tokens.get(*at..*at + 3);
    let Some([w, e, word]) = rest else {
        return Err(error(t, FrontErrorKind::UnquantizedLiteral(t.text.clone())));
    };
    if w.kind != TokenKind::Integer || e.kind != TokenKind::Integer || word.text != "QUANTIZE" {
        return Err(error(t, FrontErrorKind::UnquantizedLiteral(t.text.clone())));
    }
    let shown = format!("{} {}", w.text, e.text);
    let width =
        width_of(&w.text).ok_or_else(|| error(w, FrontErrorKind::BadQuantize(shown.clone())))?;
    let scale = e.text.parse::<i32>().ok().filter(|s| s.abs() <= MAX_SCALE);
    let scale = scale.ok_or_else(|| error(e, FrontErrorKind::BadQuantize(shown)))?;
    *at += 3;
    Ok(Item::Num {
        num: Num::from_ratio(p, q, width, scale).expect("the exponent was checked"),
        exact: (p, q),
    })
}

/// Parse a source text.
pub fn parse(source: &str) -> Result<Program, FrontError> {
    let tokens = lex(source);
    let mut at = 0;
    Ok(Program {
        items: sequence(&tokens, &mut at, true)?,
    })
}
