//! Element-wise lifting (AR-LLR-014, refining AR-HLR-022 and AR-HLR-023).
//!
//! A leaf operation applies to scalars; lifted, it applies lane by lane.
//! A vector paired only with scalars is walked whatever its nesting, each
//! scalar reused against every lane. Two or more vectors must each have a
//! shape (every lane of a vector the same shape), and they align from the
//! innermost axis, as NumPy broadcasts: an axis of length 1 is reused, a
//! vector of lower rank is reused whole against every lane of the higher
//! one, and any other pairing is refused, as is a ragged vector. This is
//! Ajisai's LANG.COLLECTIONS.LIFT; both languages read a lifted Word alike
//! (ADR-0015). For the arithmetic Words and LT, a NIL operand is the
//! result, unread and unchanged, the left one when both are NIL
//! (AR-HLR-023); AND and SELECT read a NIL as a lane of their own.

use crate::value::{ErrorKind, Reason, Value};
use arare_kernel::Truth;

pub type Leaf2<'a> = &'a dyn Fn(&Value, &Value) -> Result<Value, ErrorKind>;
pub type Leaf1<'a> = &'a dyn Fn(&Value) -> Result<Value, ErrorKind>;
type LeafN<'a> = &'a dyn Fn(&[&Value]) -> Result<Value, ErrorKind>;

/// The length at each axis of a vector whose lanes all have one shape,
/// outermost first; empty for a scalar or NIL, `None` for a ragged vector.
pub fn shape(v: &Value) -> Option<Vec<usize>> {
    match v {
        Value::Vector(xs) => {
            let mut lanes = xs.iter().map(shape);
            let inner = match lanes.next() {
                None => Vec::new(),
                Some(s) => s?,
            };
            for s in lanes {
                if s? != inner {
                    return None;
                }
            }
            let mut out = vec![xs.len()];
            out.extend(inner);
            Some(out)
        }
        _ => Some(Vec::new()),
    }
}

/// Two shapes aligned from the innermost axis: equal lengths meet, a length
/// of 1 (or a missing outer axis) is reused, any other pair is refused.
pub fn broadcast(a: &[usize], b: &[usize]) -> Result<Vec<usize>, ErrorKind> {
    let n = a.len().max(b.len());
    let at = |s: &[usize], k: usize| {
        let pad = n - s.len();
        if k < pad {
            1
        } else {
            s[k - pad]
        }
    };
    (0..n)
        .map(|k| match (at(a, k), at(b, k)) {
            (x, y) if x == y => Ok(x),
            (1, y) => Ok(y),
            (x, 1) => Ok(x),
            (x, y) => Err(ErrorKind::Shape { left: x, right: y }),
        })
        .collect()
}

/// `leaf` applied to `ops`, lifted as the module says. `nil_absorbs`: a NIL
/// operand is the result.
fn lift(ops: &[&Value], nil_absorbs: bool, leaf: LeafN) -> Result<Value, ErrorKind> {
    if nil_absorbs {
        if let Some(Value::Nil(r)) = ops.iter().find(|v| matches!(v, Value::Nil(_))) {
            return Ok(Value::Nil(*r));
        }
    }
    let is_vector = |v: &&&Value| matches!(v, Value::Vector(_));
    match ops.iter().filter(is_vector).count() {
        0 => leaf(ops),
        1 => {
            let k = ops.iter().position(|v| is_vector(&v)).expect("one vector");
            let Value::Vector(xs) = ops[k] else {
                unreachable!("a vector")
            };
            xs.iter()
                .map(|x| {
                    let mut o = ops.to_vec();
                    o[k] = x;
                    lift(&o, nil_absorbs, leaf)
                })
                .collect::<Result<_, _>>()
                .map(Value::Vector)
        }
        _ => {
            let shapes = ops
                .iter()
                .map(|v| shape(v).ok_or(ErrorKind::Ragged))
                .collect::<Result<Vec<_>, _>>()?;
            shapes
                .iter()
                .try_fold(Vec::new(), |acc, s| broadcast(&acc, s))?;
            let ranks = shapes.iter().map(Vec::len).collect();
            aligned(ops, ranks, nil_absorbs, leaf)
        }
    }
}

/// Shapes already aligned: the operands of the highest rank are walked lane
/// by lane, a lane of length 1 reused; the others are reused whole.
fn aligned(
    ops: &[&Value],
    ranks: Vec<usize>,
    nil_absorbs: bool,
    leaf: LeafN,
) -> Result<Value, ErrorKind> {
    let top = ranks.iter().copied().max().unwrap_or(0);
    if top == 0 {
        return lift(ops, nil_absorbs, leaf);
    }
    let lanes = |v: &Value| match v {
        Value::Vector(xs) => xs.len(),
        _ => unreachable!("an operand of rank above 0 is a vector"),
    };
    let width = ops
        .iter()
        .zip(&ranks)
        .filter(|(_, r)| **r == top)
        .map(|(v, _)| lanes(v))
        .fold(1, |w, n| if n == 1 { w } else { n });
    let next: Vec<usize> = ranks
        .iter()
        .map(|r| if *r == top { r - 1 } else { *r })
        .collect();
    (0..width)
        .map(|i| {
            let o: Vec<&Value> = ops
                .iter()
                .zip(&ranks)
                .map(|(v, r)| match v {
                    Value::Vector(xs) if *r == top => &xs[if xs.len() == 1 { 0 } else { i }],
                    v => *v,
                })
                .collect();
            aligned(&o, next.clone(), nil_absorbs, leaf)
        })
        .collect::<Result<_, _>>()
        .map(Value::Vector)
}

pub fn lift2(a: &Value, b: &Value, f: Leaf2) -> Result<Value, ErrorKind> {
    lift(&[a, b], true, &|o| f(o[0], o[1]))
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
    lift(&[a, b], false, &|o| {
        let ((ta, ra), (tb, rb)) = (truth(o[0])?, truth(o[1])?);
        Ok(from_truth(ta.and(tb), ra.or(rb)))
    })
}

/// SELECT, lifted over all three operands: THEN only on TRUE; FALSE,
/// UNKNOWN and NIL take ELSE (AR-HLR-015). A NIL branch is a lane.
pub fn select(then: &Value, otherwise: &Value, cond: &Value) -> Result<Value, ErrorKind> {
    lift(&[then, otherwise, cond], false, &|o| match o[2] {
        Value::Bool(true) => Ok(o[0].clone()),
        Value::Bool(false) | Value::Nil(_) => Ok(o[1].clone()),
        _ => Err(ErrorKind::Type("a truth value")),
    })
}

/// NOT, lifted; a NIL stays the NIL it is.
pub fn not(a: &Value) -> Result<Value, ErrorKind> {
    lift1(a, &|x| {
        let (t, r) = truth(x)?;
        Ok(from_truth(t.not(), r))
    })
}
