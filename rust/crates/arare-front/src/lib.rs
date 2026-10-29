//! Lexer and parser (AR-LLR-011, AR-LLR-012, refining AR-HLR-018 and
//! AR-HLR-021).
//!
//! Quality level QL-B (docs/quality/PLAN_OVERVIEW.md). Source text becomes a
//! [`Program`]: integer literals, Words, quotes, and quantized literals that
//! the parser has already folded. Everything the front end can refuse, it
//! refuses before anything runs: an unknown Word, an unbalanced bracket, a
//! literal out of range, and a non-integer literal that is not quantized where
//! it is written (AR-HLR-018).

#![forbid(unsafe_code)]

mod lexer;
mod parser;

pub use lexer::{lex, Token, TokenKind};
pub use parser::{parse, Item, Program, Word};

/// Why the front end refused a source text, and where.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FrontError {
    pub kind: FrontErrorKind,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FrontErrorKind {
    UnknownWord(String),
    UnclosedQuote,
    UnexpectedClose,
    /// An integer literal beyond the widest finite range.
    IntegerOutOfRange(String),
    /// A decimal or fraction whose numerator or denominator, in lowest
    /// terms, does not fit 64 bits.
    LiteralTooPrecise(String),
    /// A decimal, fraction or wheel point not followed, where it is written,
    /// by a width, an exponent and QUANTIZE.
    UnquantizedLiteral(String),
    /// QUANTIZE's width is not 8, 16, 32 or 64, or its exponent is outside ±31.
    BadQuantize(String),
}

impl core::fmt::Display for FrontError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let what = match &self.kind {
            FrontErrorKind::UnknownWord(w) => format!("unknown Word {w}"),
            FrontErrorKind::UnclosedQuote => "a [ is never closed".to_string(),
            FrontErrorKind::UnexpectedClose => "a ] closes nothing".to_string(),
            FrontErrorKind::IntegerOutOfRange(t) => {
                format!("{t} is beyond the widest finite range")
            }
            FrontErrorKind::LiteralTooPrecise(t) => {
                format!("{t} needs more than 64 bits in lowest terms")
            }
            FrontErrorKind::UnquantizedLiteral(t) => {
                format!("{t} is not an integer; write it as `{t} <width> <exponent> QUANTIZE`")
            }
            FrontErrorKind::BadQuantize(t) => format!(
                "QUANTIZE {t}: the width must be 8, 16, 32 or 64 and the exponent within ±31"
            ),
        };
        write!(f, "{}:{}: {what}", self.line, self.column)
    }
}
