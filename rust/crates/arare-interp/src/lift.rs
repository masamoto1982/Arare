//! Element-wise lifting (AR-LLR-014, refining AR-HLR-022 and AR-HLR-023).
//!
//! A leaf operation applies to two numbers; lifted, it applies lane by lane
//! to vectors of equal length, reuses the single lane of a one-element vector
//! or a scalar against every lane of the other operand, and refuses any other
//! pairing. A NIL operand is the result, unread and unchanged; when both are
//! NIL the left one is (AR-HLR-023).

use crate::value::{ErrorKind, Reason, Value};
use arare_kernel::Truth;

pub type Leaf2<'a> = &'a dyn Fn(&Value, &Value) -> Result<Value, ErrorKind>;
pub type Leaf1<'a> = &'a dyn Fn(&Value) -> Result<Value, ErrorKind>;

pub fn lift2(a: &Value, b: &Value, f: Leaf2) -> Result<Value, ErrorKind> {
    match (a, b) {
        (Value::Nil(r), _) | (_, Value::Nil(r)) => Ok(Value::Nil(*r)),
        (Value::Vector(xs), Value::Vector(ys)) => {
            let pairs: Vec<(&Value, &Value)> = match (xs.len(), ys.len()) {
                (n, m) if n == m => xs.iter().zip(ys).collect(),
                (1, _) => ys.iter().map(|y| (&xs[0], y)).collect(),
                (_, 1) => xs.iter().map(|x| (x, &ys[0])).collect(),
                (n, m) => return Err(ErrorKind::Shape { left: n, right: m }),
            };
            pairs
                .into_iter()
                .map(|(x, y)| lift2(x, y, f))
                .collect::<Result<_, _>>()
                .map(Value::Vector)
        }
        (Value::Vector(xs), y) => xs
            .iter()
            .map(|x| lift2(x, y, f))
            .collect::<Result<_, _>>()
            .map(Value::Vector),
        (x, Value::Vector(ys)) => ys
            .iter()
            .map(|y| lift2(x, y, f))
            .collect::<Result<_, _>>()
            .map(Value::Vector),
        (x, y) => f(x, y),
    }
}

pub fn lift1(a: &Value, f: Leaf1) -> Result<Value, ErrorKind> {
    match a {
        Value::Nil(r) => Ok(Value::Nil(*r)),
        Value::Vector(xs) => xs
            .iter()
            .map(|x| lift1(x, f))
            .collect::<Result<_, _>>()
            .map(Value::Vector),
        x => f(x),
    }
}

/// A truth operand: TRUE or FALSE, or UNKNOWN for a NIL (AR-HLR-013).
fn truth(v: &Value) -> Result<(Truth, Option<Reason>), ErrorKind> {
    match v {
        Value::Bool(true) => Ok((Truth::True, None)),
        Value::Bool(false) => Ok((Truth::False, None)),
        Value::Nil(r) => Ok((Truth::Unknown, Some(*r))),
        _ => Err(ErrorKind::Type("a truth value")),
    }
}

/// A truth value back on the stack; UNKNOWN is the NIL it came from.
fn from_truth(t: Truth, reason: Option<Reason>) -> Value {
    match t {
        Truth::True => Value::Bool(true),
        Truth::False => Value::Bool(false),
        Truth::Unknown => Value::Nil(reason.expect("UNKNOWN comes only from a NIL operand")),
    }
}

/// AND, lifted. A NIL is a truth operand here, not passed through: FALSE
/// settles the result by itself (AR-HLR-012).
pub fn and(a: &Value, b: &Value) -> Result<Value, ErrorKind> {
    match (a, b) {
        (Value::Vector(_), _) | (_, Value::Vector(_)) => {
            let (xs, ys) = (as_lanes(a), as_lanes(b));
            let pairs: Vec<(&Value, &Value)> = match (xs.len(), ys.len()) {
                (n, m) if n == m => xs.iter().zip(ys.iter()).collect(),
                (1, _) => ys.iter().map(|y| (&xs[0], y)).collect(),
                (_, 1) => xs.iter().map(|x| (x, &ys[0])).collect(),
                (n, m) => return Err(ErrorKind::Shape { left: n, right: m }),
            };
            pairs
                .into_iter()
                .map(|(x, y)| and(x, y))
                .collect::<Result<_, _>>()
                .map(Value::Vector)
        }
        _ => {
            let ((ta, ra), (tb, rb)) = (truth(a)?, truth(b)?);
            Ok(from_truth(ta.and(tb), ra.or(rb)))
        }
    }
}

/// NOT, lifted; a NIL stays the NIL it is.
pub fn not(a: &Value) -> Result<Value, ErrorKind> {
    lift1(a, &|x| {
        let (t, r) = truth(x)?;
        Ok(from_truth(t.not(), r))
    })
}

fn as_lanes(v: &Value) -> Vec<Value> {
    match v {
        Value::Vector(xs) => xs.clone(),
        other => vec![other.clone()],
    }
}
