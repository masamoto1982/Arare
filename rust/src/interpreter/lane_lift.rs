//! Element lifting for the fixed-arity Words whose law is stated lane by lane
//! (LANG.COLLECTIONS.LIFT).
//!
//! The clause is one rule, so it is one function: "A scalar combines with
//! every element of a vector, however that vector is nested … Paired axes
//! combine when their lengths are equal, and when one of them is 1 that
//! operand's single lane is reused across the other's length." Arithmetic
//! reaches that rule through the tensor path; the truth-order connectives and comparisons
//! reach it through here, so the one-lane broadcast means the same thing in
//! all three families instead of being spelled once per family — the
//! comparison Words used to accept `[ 1 2 3 ] [ 2 2 2 ] GT` and refuse
//! `[ 1 2 3 ] [ 2 ] GT`, which is the clause's own example with the numbers
//! changed.
//!
//! The lift is over a fixed number of operands rather than exactly two,
//! because `SELECT` is the same law with three: its truth operand chooses
//! between the other two lane by lane, and nothing about the alignment
//! changes for having one more operand to align.

use std::borrow::Cow;

use crate::error::{AjisaiError, Result};
use crate::types::{RecordData, Value, ValueData};

/// How one operand supplies its lanes at the current axis.
///
/// The Vector arm holds the `Cow` `as_vector_view` hands back rather than a
/// slice: a Tensor materializes its nested view on demand, so the lanes of a
/// Tensor operand are owned here and would dangle behind a borrow.
enum Lanes<'a> {
    /// Not a Vector here: the same value combines with every lane.
    Scalar(&'a Value),
    /// A Vector. One of length 1 imposes no extent — its single lane is
    /// reused across whatever length the other operands set.
    Vector(Cow<'a, [Value]>),
}

impl<'a> Lanes<'a> {
    fn of(value: &'a Value) -> Lanes<'a> {
        match value.as_vector_view() {
            None => Lanes::Scalar(value),
            Some(items) => Lanes::Vector(items),
        }
    }

    /// The extent this operand imposes on the axis, or `None` when it imposes
    /// none (a scalar, or a reused single lane).
    fn extent(&self) -> Option<usize> {
        match self {
            Lanes::Scalar(_) => None,
            Lanes::Vector(items) => vector_extent(items.len()),
        }
    }

    fn is_vector(&self) -> bool {
        matches!(self, Lanes::Vector(_))
    }

    fn lane(&self, index: usize) -> &Value {
        match self {
            Lanes::Scalar(value) => value,
            Lanes::Vector(items) if items.len() == 1 => &items[0],
            Lanes::Vector(items) => &items[index],
        }
    }
}

/// The extent a Vector of `len` lanes sets on the lifted axis: none when it
/// holds one lane, which is reused across whatever length the others set.
pub(crate) fn vector_extent(len: usize) -> Option<usize> {
    (len != 1).then_some(len)
}

/// The extent the operands set together: `None` when none sets one, or the
/// first width set and the first that disagrees with it. `truth_ops` aligns
/// its columns by the same rule.
pub(crate) fn common_extent(
    extents: impl IntoIterator<Item = Option<usize>>,
) -> std::result::Result<Option<usize>, (usize, usize)> {
    let mut width = None;
    for extent in extents.into_iter().flatten() {
        match width {
            None => width = Some(extent),
            Some(w) if w == extent => {}
            Some(w) => return Err((w, extent)),
        }
    }
    Ok(width)
}

/// Apply `scalar` to the aligned lanes of `operands`, every operand lifted.
///
/// `scalar` sees only operands that are not Vectors or Records at this
/// level, so it states the scalar law alone and never repeats the alignment.
/// Where the extents disagree the result is the `shapeMismatch` ERROR the
/// clause requires, naming the first axis on which two operands disagree.
pub(crate) fn lift_lanes<const N: usize>(
    operands: [&Value; N],
    scalar: impl Fn([&Value; N]) -> Result<Value>,
) -> Result<Value> {
    if let Some(flat) = lift_flat(operands, &scalar) {
        return flat;
    }
    let mut adapter = |lanes: &[&Value]| -> Result<Value> {
        let lanes: [&Value; N] = lanes
            .try_into()
            .expect("lift_lanes_dyn hands back as many operands as it was given");
        scalar(lanes)
    };
    lift_lanes_dyn(&operands, &[true; N], &mut adapter)
}

/// [`lift_lanes`] where every operand is a leaf or a Vector of leaves: one
/// monomorphized loop over the lanes, the scalar law inlined. `None` when the
/// operands need the general walk — a Record, a Tensor, a nested lane, or
/// extents that disagree (whose `shapeMismatch` the walk words).
#[inline]
fn lift_flat<const N: usize>(
    operands: [&Value; N],
    scalar: &impl Fn([&Value; N]) -> Result<Value>,
) -> Option<Result<Value>> {
    // The extent the Vectors set: `None` before any, a length-1 Vector
    // setting none of its own. An empty one is left to the general walk.
    let mut seen = false;
    let mut width = 1;
    for value in operands {
        match &value.data {
            ValueData::Vector(items) => match items.len() {
                0 => return None,
                1 => seen = true,
                len if !seen || width == 1 => (seen, width) = (true, len),
                len if len == width => {}
                _ => return None,
            },
            ValueData::Tensor { .. } | ValueData::Record(_) | ValueData::Truths(_) => return None,
            _ => {}
        }
    }
    if !seen {
        return None;
    }
    fn lane(value: &Value, index: usize) -> Option<&Value> {
        let lane = match &value.data {
            ValueData::Vector(items) if items.len() == 1 => &items[0],
            ValueData::Vector(items) => &items[index],
            _ => return Some(value),
        };
        (!is_container(lane)).then_some(lane)
    }
    let mut result = Vec::with_capacity(width);
    for index in 0..width {
        let mut lanes = operands;
        for slot in lanes.iter_mut() {
            *slot = lane(slot, index)?;
        }
        match scalar(lanes) {
            Ok(value) => result.push(value),
            Err(e) => return Some(Err(e)),
        }
    }
    Some(Ok(Value::from_vector(result)))
}

/// The lifting rule over any number of operands, of which only those marked
/// in `lifted` are aligned; the rest are handed to every lane whole.
///
/// A Record lifts first, in the value direction (LANG.RECORDS.STRUCTURE):
/// lifted Records must share their key sequence, pair value by value, and
/// every other operand — a Vector included — combines with each value. Then
/// Vectors align lane by lane, a one-element Vector's lane reused across the
/// others' length. What remains goes to `scalar`.
pub(crate) fn lift_lanes_dyn(
    operands: &[&Value],
    lifted: &[bool],
    scalar: &mut dyn FnMut(&[&Value]) -> Result<Value>,
) -> Result<Value> {
    let records: Vec<(usize, &RecordData)> = operands
        .iter()
        .zip(lifted)
        .enumerate()
        .filter_map(|(i, (v, l))| {
            if *l {
                v.as_record().map(|r| (i, r))
            } else {
                None
            }
        })
        .collect();
    if let Some((_, first)) = records.first() {
        if let Some((_, other)) = records.iter().find(|(_, r)| !r.same_keys(first)) {
            return Err(AjisaiError::declared(
                "shapeMismatch",
                format!(
                    "expected Records with the same keys, got {} keys against {} keys that do not line up",
                    first.len(),
                    other.len()
                ),
            ));
        }
        let mut values = Vec::with_capacity(first.len());
        for k in 0..first.len() {
            let mut next: Vec<&Value> = operands.to_vec();
            for (i, record) in &records {
                next[*i] = &record.values()[k];
            }
            values.push(lift_lanes_dyn(&next, lifted, scalar)?);
        }
        return Ok(Value::from_record(
            RecordData::new(first.keys().to_vec(), values)
                .expect("the keys are one Record's own, so distinct and aligned"),
        ));
    }

    let lanes: Vec<Lanes> = operands
        .iter()
        .zip(lifted)
        .map(|(v, l)| if *l { Lanes::of(v) } else { Lanes::Scalar(v) })
        .collect();

    let extent = common_extent(lanes.iter().map(Lanes::extent))
        .map_err(|(width, len)| AjisaiError::shape_mismatch(&[width], &[len], 0))?;

    let Some(width) = extent else {
        // No operand is a Vector with an extent to pair. A single lane still
        // has to be entered — `[ TRUE ] [ FALSE ] [ TRUE ] SELECT` answers
        // `[ TRUE ]`, because a one-element Vector is a Vector
        // (LANG.VALUES.DISJOINT) and only its contents were reused.
        if lanes.iter().any(Lanes::is_vector) {
            let first: Vec<&Value> = lanes.iter().map(|lane| lane.lane(0)).collect();
            let inner = lift_lanes_dyn(&first, lifted, scalar)?;
            return Ok(Value::from_vector(vec![inner]));
        }
        return scalar(operands);
    };

    // One buffer serves every lane. Where no lifted lane is a container the
    // recursion would only hand the lane straight to `scalar`, so that step is
    // taken here — the common flat case then allocates nothing per lane.
    let mut result = Vec::with_capacity(width);
    let mut next: Vec<&Value> = Vec::with_capacity(lanes.len());
    for index in 0..width {
        next.clear();
        next.extend(lanes.iter().map(|lane| lane.lane(index)));
        let leaf = next
            .iter()
            .zip(lifted)
            .all(|(v, l)| !*l || !is_container(v));
        result.push(if leaf {
            scalar(&next)?
        } else {
            lift_lanes_dyn(&next, lifted, scalar)?
        });
    }
    Ok(Value::from_vector(result))
}

/// Whether the lifting rule would enter `value` rather than hand it to the
/// scalar law: a Vector, a Tensor or a Record.
fn is_container(value: &Value) -> bool {
    matches!(
        value.data,
        ValueData::Vector(_)
            | ValueData::Tensor { .. }
            | ValueData::Record(_)
            | ValueData::Truths(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The flat loop answers exactly what the general walk answers, error
    /// included, wherever it answers at all.
    #[test]
    fn the_flat_loop_agrees_with_the_general_walk() {
        let t = Value::from_bool(true);
        let f = Value::from_bool(false);
        let leaves = [t.clone(), f.clone(), Value::nil(), Value::from_int(7)];
        let vector = |items: &[Value]| Value::from_vector(items.to_vec());
        let mut operands = leaves.to_vec();
        operands.extend([
            vector(&[]),
            vector(std::slice::from_ref(&t)),
            vector(&[t.clone(), f.clone(), Value::nil()]),
            vector(&[Value::nil(), t.clone(), f.clone()]),
            vector(&[t.clone(), f.clone()]),
            vector(&[vector(std::slice::from_ref(&t)), f.clone(), t.clone()]),
            Value::from_int_tensor(vec![1, 2, 3]),
        ]);
        let law = |[a, b]: [&Value; 2]| -> Result<Value> {
            match (&a.data, &b.data) {
                (ValueData::Scalar(_), _) | (_, ValueData::Scalar(_)) => Err(
                    AjisaiError::declared("nonTruthValue", "a number".to_string()),
                ),
                _ => Ok(Value::from_vector(vec![a.clone(), b.clone()])),
            }
        };
        let mut checked = 0;
        for a in &operands {
            for b in &operands {
                let general = lift_lanes_dyn(&[a, b], &[true; 2], &mut |lanes: &[&Value]| {
                    law([lanes[0], lanes[1]])
                });
                if let Some(flat) = lift_flat([a, b], &law) {
                    checked += 1;
                    assert_eq!(
                        format!("{flat:?}"),
                        format!("{general:?}"),
                        "{a:?} and {b:?}"
                    );
                }
            }
        }
        assert!(checked > 20, "the flat loop answered only {checked} pairs");
    }
}

#[cfg(kani)]
#[path = "lane_lift_proofs.rs"]
mod proofs;
