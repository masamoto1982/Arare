//! The shape half of a recursive broadcast over ragged or nested-mixed
//! operands, shared by every lift that walks a value tree rather than a
//! flattened tensor.

use crate::error::{AjisaiError, Result};
use crate::interpreter::tensor_ops::{broadcast_children, broadcast_shape, rectangular_shape_by};
use crate::types::Value;

/// The shape half of a recursive broadcast, for ragged or nested-mixed
/// operands that cannot be flattened to a tensor. A leaf paired with a Vector
/// is broadcast across every element, however that Vector nests. Two Vectors
/// pair by shape as LANG.COLLECTIONS.LIFT pairs them: aligned at the innermost
/// axis, an axis of length 1 reused across the other's length, and any other
/// difference — or either Vector being ragged — a `shapeMismatch`. The leaf
/// law is the caller's, so the walk is the same whether a lane answers a
/// number, a whole `Value`, or a NIL with its reason.
///
/// One walk for the lane-wise lift (`tensor_lane_ops`), the exact-real lift
/// (`arithmetic`) and the numeric lift of `MIN`/`MAX` (`math_ops`); each
/// used to carry its own copy of this `match`, kept in step by hand. The
/// copies paired two Vectors at their *outermost* axis and zipped any two of
/// equal length, so `[ [ 1 2 ] [ 3 4 ] ] [ 10 20 ] ADD` meant one thing with
/// rational lanes (the flat route, innermost) and another with an irrational
/// one, a one-element Vector did not stretch on the exact route, and a ragged
/// Vector paired with any Vector of its length.
pub(crate) fn broadcast_tree(
    a: &Value,
    b: &Value,
    leaf: &dyn Fn(&Value, &Value) -> Result<Value>,
) -> Result<Value> {
    match (broadcast_children(a), broadcast_children(b)) {
        (None, None) => leaf(a, b),
        (Some(children), None) => {
            children_of(children.iter().map(|child| broadcast_tree(child, b, leaf)))
        }
        (None, Some(children)) => {
            children_of(children.iter().map(|child| broadcast_tree(a, child, leaf)))
        }
        (Some(_), Some(_)) => {
            // Every leaf counts here, numeric or not: whether a leaf is a
            // number is the leaf law's question, and its `nonNumeric` answer
            // is the one the Word declares.
            let shape_of = |value: &Value| rectangular_shape_by(value, |_| true);
            let (Some(shape_a), Some(shape_b)) = (shape_of(a), shape_of(b)) else {
                return Err(AjisaiError::declared(
                    "shapeMismatch",
                    "expected two Vectors with a shape, got a ragged one",
                ));
            };
            broadcast_shape(&shape_a, &shape_b)?;
            broadcast_aligned(a, b, shape_a.len(), shape_b.len(), leaf)
        }
    }
}

/// Two rectangular operands of ranks `rank_a` and `rank_b`, whose shapes
/// `broadcast_shape` has already accepted. The operand with more axes is
/// walked alone until the ranks meet, which is what aligning at the innermost
/// axis means; then the axes pair, a length-1 one reused across the other.
fn broadcast_aligned(
    a: &Value,
    b: &Value,
    rank_a: usize,
    rank_b: usize,
    leaf: &dyn Fn(&Value, &Value) -> Result<Value>,
) -> Result<Value> {
    if rank_a == 0 && rank_b == 0 {
        return leaf(a, b);
    }
    let walk = |x: &Value, y: &Value, rx: usize, ry: usize| broadcast_aligned(x, y, rx, ry, leaf);
    let rows = |value: &Value| broadcast_children(value).unwrap_or_default();
    if rank_a > rank_b {
        return children_of(rows(a).iter().map(|x| walk(x, b, rank_a - 1, rank_b)));
    }
    if rank_b > rank_a {
        return children_of(rows(b).iter().map(|y| walk(a, y, rank_a, rank_b - 1)));
    }
    let (left, right) = (rows(a), rows(b));
    let rank = rank_a - 1;
    if left.len() == right.len() {
        children_of(
            left.iter()
                .zip(right.iter())
                .map(|(x, y)| walk(x, y, rank, rank)),
        )
    } else if left.len() == 1 {
        children_of(right.iter().map(|y| walk(&left[0], y, rank, rank)))
    } else {
        children_of(left.iter().map(|x| walk(x, &right[0], rank, rank)))
    }
}

fn children_of(values: impl Iterator<Item = Result<Value>>) -> Result<Value> {
    Ok(Value::from_children(
        values.collect::<Result<Vec<Value>>>()?,
    ))
}
