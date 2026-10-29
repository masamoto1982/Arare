//! Values, NIL reasons and errors (AR-LLR-013, AR-LLR-016).

use arare_front::{Item, Word};
use arare_kernel::Num;
use core::fmt;
use std::rc::Rc;

/// Why a NIL arose (AR-HLR-020). A reason never changes a value or a truth
/// value; it is carried, unchanged, wherever the NIL flows.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Reason {
    /// Written as `NIL`.
    Literal,
    /// An order was asked of a wheel point (AR-HLR-011).
    OrderOfPoint,
    /// An aggregate had no element that was not NIL (AR-HLR-016).
    EmptyAggregate,
}

impl Reason {
    pub fn name(self) -> &'static str {
        match self {
            Reason::Literal => "literal",
            Reason::OrderOfPoint => "orderOfPoint",
            Reason::EmptyAggregate => "emptyAggregate",
        }
    }
}

/// A value on the stack. UNKNOWN is not a separate value: it is a NIL read
/// where a truth value is expected (AR-HLR-013).
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Num(Num),
    Bool(bool),
    Nil(Reason),
    Vector(Vec<Value>),
    /// A bracketed sequence not yet consumed: data or code, by what consumes it.
    Quote(Rc<Vec<Item>>),
}

/// Malformed use: the program is wrong, so it stops (AR-HLR-025).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ErrorKind {
    StackUnderflow {
        needed: usize,
        had: usize,
    },
    /// An operand of the wrong kind; names the kind expected.
    Type(&'static str),
    /// Two vectors whose lengths neither match nor include a single element.
    Shape {
        left: usize,
        right: usize,
    },
    /// A result exponent outside ±31 (AR-LLR-004).
    ScaleOverflow,
    /// An operand outside what the Word accepts; says what it accepts.
    BadOperand(&'static str),
    /// A block left other than exactly one value.
    BlockResult(usize),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Error {
    pub word: Word,
    pub kind: ErrorKind,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.word.name())?;
        match &self.kind {
            ErrorKind::StackUnderflow { needed, had } => {
                write!(f, "needs {needed} operand(s), the stack has {had}")
            }
            ErrorKind::Type(expected) => write!(f, "expected {expected}"),
            ErrorKind::Shape { left, right } => {
                write!(f, "vectors of length {left} and {right} do not pair")
            }
            ErrorKind::ScaleOverflow => write!(f, "the result exponent would leave ±31"),
            ErrorKind::BadOperand(accepts) => write!(f, "accepts {accepts}"),
            ErrorKind::BlockResult(n) => write!(f, "a block must leave exactly one value, not {n}"),
        }
    }
}

/// The exact rational of a number, as Ajisai prints one: `n/d`, with `1/0`
/// and `0/0` for the points.
pub fn rational(n: Num) -> String {
    let (p, q) = n.exact();
    format!("{p}/{q}")
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Num(n) => write!(f, "{}", rational(*n)),
            Value::Bool(b) => f.write_str(if *b { "TRUE" } else { "FALSE" }),
            Value::Nil(r) => write!(f, "NIL({})", r.name()),
            Value::Vector(xs) => {
                f.write_str("[")?;
                for x in xs {
                    write!(f, " {x}")?;
                }
                f.write_str(" ]")
            }
            Value::Quote(items) => write!(f, "[ …{} item(s) ]", items.len()),
        }
    }
}
