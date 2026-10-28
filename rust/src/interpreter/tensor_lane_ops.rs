//! Element-wise broadcast for Words whose scalar law can project.
//!
//! The `Fraction`-level broadcasts in [`tensor_ops`] answer each lane with a
//! number: a `Fraction` records presence, not why something is absent. A Word
//! whose scalar law may *project* — answer NIL for a well-formed operand
//! (`LANG.FAILURE.TRICHOTOMY`) — therefore cannot lift through them without
//! flattening every projection into an anonymous absence, or worse, into one
//! failure for the whole operation.
//!
//! This module is the lift for those Words. It reuses `tensor_ops`' shape
//! rules exactly, so the result shape never depends on whether the Word
//! projected, and differs only in what a lane may answer with. The lanes are
//! decided as `Value`s, so each projection keeps its reason, and the result
//! is then promoted back to a dense Tensor wherever its lanes fit one — which
//! a NIL lane does not, so a projection costs the vector around it its
//! columns, and nothing else.
//!
//! [`tensor_ops`]: crate::interpreter::tensor_ops

use crate::error::{AjisaiError, Result};
use crate::interpreter::broadcast_tree::broadcast_tree;
use crate::interpreter::tensor_ops::{
    broadcast_leaf, broadcast_shape, compute_strides, project_broadcast_index, ravel_index,
    rectangular_shape, unravel_index,
};
use crate::types::fraction::Fraction;
use crate::types::{Value, ValueData};

/// Whether a NIL sits anywhere inside `value`.
///
/// A NIL lives only as a `Value`, carrying its `AbsenceMetadata` and
/// therefore its reason; a dense tensor holds numbers alone. This predicate
/// exists to steer a broadcast away from the flat `Fraction` kernels, which
/// cannot carry a reason through a lane.
///
/// Reached only when a broadcast is about to choose a route, so the cost is
/// one linear scan against a value the flat path would have walked anyway.
pub(crate) fn contains_absent_lane(value: &Value) -> bool {
    match &value.data {
        ValueData::Truths(t) => contains_absent_lane(&t.to_vector()),
        ValueData::Nil(_) => true,
        ValueData::Vector(items) => items.iter().any(contains_absent_lane),
        ValueData::Record(record) => record.values().iter().any(contains_absent_lane),
        ValueData::Tensor { .. }
        | ValueData::Scalar(_)
        | ValueData::Boolean(_)
        | ValueData::ExactScalar(_)
        | ValueData::Symbol(_)
        | ValueData::Text(_) => false,
    }
}

/// The per-lane lift of the scalar NIL passthrough law, or `None` when this
/// lane has two present operands and the numeric law decides it.
///
/// `LANG.COLLECTIONS.LIFT` says each lane preserves the scalar law's NIL
/// distinction, and the scalar law is one rule stated once, in
/// `Interpreter::declared_nil_contract`'s `Passthrough` arm: a NIL operand
/// *is* the result, and the leftmost one wins, matching left-to-right
/// evaluation order. This is that rule, per lane — the same
/// `Value::nil_inheriting_absence_from` the scalar helpers use, so a lane
/// cannot answer a NIL the scalar would not.
///
/// It is applied *before* the lane's numeric law, not inside it. A law that
/// takes `&Fraction` operands cannot obey it: `Fraction` records absence as a
/// zero denominator and carries no reason, so every such law could answer was
/// a reasonless NIL — which reads back as `nil:literal`, "a NIL the program
/// wrote rather than computed" (`spec/outcomes.json`). That is how
/// `[ 1 2 ] [ 1 0 ] DIV [ 1 1 ] DIV` reported a written NIL for a computed
/// division by zero.
pub(crate) fn lane_nil_passthrough(a: &Value, b: &Value) -> Option<Value> {
    if a.is_nil() {
        return Some(Value::nil_inheriting_absence_from(a));
    }
    if b.is_nil() {
        return Some(Value::nil_inheriting_absence_from(b));
    }
    None
}

/// The tree-walking half of [`apply_lane_wise_broadcast`], for ragged or
/// nested-mixed operands: [`broadcast_tree`]'s walk with [`apply_lane_law`]
/// at the leaves. Every caller declares `nonNumeric` uniformly, same as
/// `FlatTensor::from_value`.
fn apply_lane_wise_recursive<F>(a: &Value, b: &Value, op: F) -> Result<Value>
where
    F: Fn(&Fraction, &Fraction) -> Result<Value> + Copy,
{
    broadcast_tree(a, b, &|x, y| apply_lane_law(x, y, op))
}

/// Element-wise broadcast whose leaf law answers with a whole `Value`.
///
/// The `Fraction`-level broadcasts can only hand a lane a number, so a Word
/// whose scalar law may *project* — answer NIL for a well-formed operand —
/// cannot use them without flattening every projection into an anonymous
/// absence: a `Fraction` records presence, not a reason. `SQRT` already lifts
/// this way through `lift_unary_numeric`, which is why a negative lane comes
/// back as `NIL(domainMiss)` beside its neighbours. This is the binary
/// counterpart.
///
/// Shape handling is the flat path's, lane for lane — the same
/// [`broadcast_shape`] and the same index projection — so a Word cannot mean
/// one thing when it projects and another when it does not.
///
/// The leaf law never sees an absent operand: [`apply_lane_law`] settles those
/// first, from the `Value`, so each lane's reason survives the lift. It is not
/// a special case for values that happen to carry one — every lane of every
/// operand takes that route, which is why no caller has to ask whether a
/// reason is at stake before choosing this lift.
pub(crate) fn apply_lane_wise_broadcast<F>(a: &Value, b: &Value, op: F) -> Result<Value>
where
    F: Fn(&Fraction, &Fraction) -> Result<Value> + Copy,
{
    if a.is_nil() || b.is_nil() {
        // Defensive: the dispatcher passes a NIL operand through before a
        // numeric Word runs (LANG.FAILURE.PASSTHROUGH). Every caller declares
        // `nonNumeric`, and a NIL is not a Scalar.
        return Err(AjisaiError::declared(
            "nonNumeric",
            "expected a Scalar or a Vector, got NIL",
        ));
    }

    // Ragged operands cannot be flattened to a tensor whose shape matches
    // their element count, so they follow the value tree instead.
    let (Some(shape_a), Some(shape_b)) = (rectangular_shape(a), rectangular_shape(b)) else {
        return apply_lane_wise_recursive(a, b, op);
    };

    let lanes_a = flat_leaf_values(a);
    let lanes_b = flat_leaf_values(b);
    let out_shape = broadcast_shape(&shape_a, &shape_b)?;
    let out_size: usize = if out_shape.is_empty() {
        1
    } else {
        out_shape.iter().product()
    };
    let out_strides = compute_strides(&out_shape);
    let strides_a = compute_strides(&shape_a);
    let strides_b = compute_strides(&shape_b);
    let same_shape = shape_a == shape_b;

    let mut out_values: Vec<Value> = Vec::with_capacity(out_size);
    for linear in 0..out_size {
        let (a_offset, b_offset) = if same_shape {
            (linear, linear)
        } else {
            let out_index = unravel_index(linear, &out_shape, &out_strides);
            let a_index = project_broadcast_index(&out_index, &out_shape, &shape_a);
            let b_index = project_broadcast_index(&out_index, &out_shape, &shape_b);
            (
                ravel_index(&a_index, &strides_a),
                ravel_index(&b_index, &strides_b),
            )
        };
        out_values.push(apply_lane_law(&lanes_a[a_offset], &lanes_b[b_offset], op)?);
    }

    Ok(nest_lane_values(out_values, &out_shape))
}

/// One lane: the passthrough law first, then the numeric law.
///
/// The two flat operands and the tree walk share this so a lane cannot be
/// decided one way in a rectangular value and another way in a ragged one.
fn apply_lane_law<F>(a: &Value, b: &Value, op: F) -> Result<Value>
where
    F: Fn(&Fraction, &Fraction) -> Result<Value> + Copy,
{
    // Absence first, and from the `Value` rather than the `Fraction`: this is
    // the one point in the lift where the lane's reason is still readable.
    if let Some(nil) = lane_nil_passthrough(a, b) {
        return Ok(nil);
    }
    let (Some(fa), Some(fb)) = (broadcast_leaf(a), broadcast_leaf(b)) else {
        return Err(AjisaiError::declared(
            "nonNumeric",
            format!(
                "expected a Scalar or a Vector, got {}",
                if broadcast_leaf(a).is_none() {
                    a.domain_name()
                } else {
                    b.domain_name()
                }
            ),
        ));
    };
    op(&fa, &fb)
}

/// The leaves of a rectangular value, in the order `FlatTensor` flattens it.
///
/// This is `tensor_ops::FlatTensor::from_value` with the lane type widened
/// from `Fraction` to `Value` — the whole point of this module: a `Value`
/// lane carries its `AbsenceMetadata` intact.
fn flat_leaf_values(value: &Value) -> Vec<Value> {
    fn collect(value: &Value, out: &mut Vec<Value>) {
        match &value.data {
            ValueData::Vector(items) => {
                for item in items.iter() {
                    collect(item, out);
                }
            }
            ValueData::Tensor { data, .. } => {
                for lane in 0..data.len() {
                    out.push(Value::from_dense_lane(data, lane));
                }
            }
            _ => out.push(value.clone()),
        }
    }
    let mut out = Vec::new();
    collect(value, &mut out);
    out
}

/// Fold flat lane values back into `shape`: the dense Tensor the flat path
/// would have built when every lane fits one, else nested Vectors. An empty
/// shape is the scalar case — one lane, and it *is* the result.
fn nest_lane_values(mut values: Vec<Value>, shape: &[usize]) -> Value {
    if shape.is_empty() {
        return values.pop().unwrap_or_else(Value::nil);
    }
    Value::from_vector_promoted(nest_children(values, shape))
}

/// The children of the Vector `shape` arranges `values` into, each inner
/// axis already nested.
fn nest_children(mut values: Vec<Value>, shape: &[usize]) -> Vec<Value> {
    if shape.len() == 1 {
        return values;
    }
    let inner_shape = &shape[1..];
    let inner_size: usize = inner_shape.iter().product();
    if inner_size == 0 {
        return Vec::new();
    }
    let mut outer: Vec<Value> = Vec::with_capacity(shape[0]);
    for _ in 0..shape[0] {
        let tail = values.split_off(inner_size.min(values.len()));
        outer.push(Value::from_children(nest_children(values, inner_shape)));
        values = tail;
    }
    outer
}
