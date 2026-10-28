use crate::error::{AjisaiError, Result};
use crate::interpreter::tensor_lane_ops::{apply_lane_wise_broadcast, contains_absent_lane};
use crate::types::fraction::Fraction;
use crate::types::{Value, ValueData};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct FlatTensor {
    pub(crate) data: Vec<Fraction>,
    pub(crate) shape: Vec<usize>,
    pub(crate) strides: Vec<usize>,
}

impl FlatTensor {
    /// Every caller reaches this only through the arithmetic-broadcast
    /// machinery (`apply_lane_wise_broadcast`, `apply_binary_broadcast`)
    /// on behalf of ADD/SUB/MUL/DIV, which all declare
    /// `nonNumeric` uniformly — so a non-numeric operand's error
    /// is remapped directly here, not at each caller.
    pub(crate) fn from_value(value: &Value) -> Result<Self> {
        match &value.data {
            ValueData::Truths(t) => Self::from_value(&t.to_vector()),
            ValueData::Nil(_) => Err(AjisaiError::declared(
                "nonNumeric",
                "expected a Scalar or a Vector, got NIL",
            )),
            ValueData::Text(_) => Err(AjisaiError::declared(
                "nonNumeric",
                "expected a Scalar or a Vector, got String",
            )),
            // A Record never reaches the flat kernels: `record_lift` peels
            // it value by value first, so one here is a route error.
            ValueData::Record(_) => Err(AjisaiError::declared(
                "nonNumeric",
                "expected a number or vector, got a record",
            )),
            ValueData::Scalar(f) => Ok(Self {
                data: vec![f.clone()],
                shape: Vec::new(),
                strides: Vec::new(),
            }),
            ValueData::Vector(_) => {
                let shape: Vec<usize> = value.shape();
                let total_size: usize = value.count_fractions();
                let mut data: Vec<Fraction> = Vec::with_capacity(total_size);
                value.collect_fractions_flat_into(&mut data);
                // An irrational lane has no `Fraction` and is skipped by the
                // collection, so the data would be shorter than the shape.
                if data.len() != total_size {
                    return Err(AjisaiError::declared(
                        "nonNumeric",
                        "expected a number or vector, got an exact irrational value",
                    ));
                }
                let strides: Vec<usize> = compute_strides(&shape);
                Ok(Self {
                    data,
                    shape,
                    strides,
                })
            }
            ValueData::Tensor { data, shape } => {
                let shape_vec: Vec<usize> = (**shape).clone();
                let strides: Vec<usize> = compute_strides(&shape_vec);
                Ok(Self {
                    data: data.to_fractions(),
                    shape: shape_vec,
                    strides,
                })
            }
            ValueData::ExactScalar(_) => Err(AjisaiError::declared(
                "nonNumeric",
                "expected a number or vector, got an exact irrational value",
            )),
            ValueData::Boolean(_) | ValueData::Symbol(_) => Err(AjisaiError::declared(
                "nonNumeric",
                "expected a number or vector, got a boolean or symbol",
            )),
        }
    }

    pub(crate) fn from_shape_and_data(shape: Vec<usize>, data: Vec<Fraction>) -> Result<Self> {
        let expected: usize = if shape.is_empty() {
            1
        } else {
            shape.iter().product()
        };
        // Every caller builds `data` from `shape`, so a mismatch is a
        // broadcast bug, not an operand the program could have written.
        assert_eq!(
            data.len(),
            expected,
            "a flat tensor holds exactly the elements its shape {shape:?} names"
        );
        let strides: Vec<usize> = compute_strides(&shape);
        Ok(Self {
            data,
            shape,
            strides,
        })
    }

    pub(crate) fn to_value(&self) -> Value {
        if self.shape.is_empty() {
            return Value::from_fraction(self.data[0].clone());
        }
        Value::from_tensor(self.data.clone(), self.shape.clone())
    }
}

pub(crate) fn compute_strides(shape: &[usize]) -> Vec<usize> {
    if shape.is_empty() {
        return Vec::new();
    }
    let mut strides = vec![1; shape.len()];
    for i in (0..shape.len() - 1).rev() {
        strides[i] = strides[i + 1] * shape[i + 1];
    }
    strides
}

pub(crate) fn unravel_index(mut linear: usize, shape: &[usize], strides: &[usize]) -> Vec<usize> {
    if shape.is_empty() {
        return Vec::new();
    }
    let mut out = vec![0; shape.len()];
    for i in 0..shape.len() {
        out[i] = linear / strides[i];
        linear %= strides[i];
    }
    out
}

pub(crate) fn ravel_index(index: &[usize], strides: &[usize]) -> usize {
    index.iter().zip(strides.iter()).map(|(i, s)| i * s).sum()
}

pub(crate) fn project_broadcast_index(
    output_index: &[usize],
    output_shape: &[usize],
    input_shape: &[usize],
) -> Vec<usize> {
    if input_shape.is_empty() {
        return Vec::new();
    }

    let mut projected = vec![0; input_shape.len()];
    let rank_diff = output_shape.len().saturating_sub(input_shape.len());

    for i in 0..input_shape.len() {
        let out_axis = i + rank_diff;
        let out_val = output_index[out_axis];
        projected[i] = if input_shape[i] == 1 { 0 } else { out_val };
    }

    projected
}

/// The length `shape` has on `axis` of a `rank`-axis broadcast: shapes are
/// right-aligned, NumPy-style, and an axis a shorter shape lacks is 1. An
/// axis past the rank reads as 1 too rather than panicking, since
/// `AjisaiError::shape_mismatch` reports through this whatever axis it is
/// handed.
pub(crate) fn aligned_dim(shape: &[usize], rank: usize, axis: usize) -> usize {
    (axis + shape.len())
        .checked_sub(rank)
        .and_then(|own| shape.get(own).copied())
        .unwrap_or(1)
}

pub(crate) fn broadcast_shape(a: &[usize], b: &[usize]) -> Result<Vec<usize>> {
    // Report the axis, not just the two shapes. It is an index into the
    // *aligned* rank (shapes are right-aligned, NumPy-style), which is the
    // axis a reader counts when they look at the value.
    broadcast_dims(a, b).map_err(|axis| AjisaiError::shape_mismatch(a, b, axis))
}

/// The shape two shapes broadcast to, or the aligned axis where they meet
/// with two lengths that differ and neither is 1.
fn broadcast_dims(a: &[usize], b: &[usize]) -> std::result::Result<Vec<usize>, usize> {
    let rank: usize = a.len().max(b.len());
    let mut out: Vec<usize> = vec![1; rank];

    for (i, slot) in out.iter_mut().enumerate() {
        let a_dim = aligned_dim(a, rank, i);
        let b_dim = aligned_dim(b, rank, i);
        if a_dim == b_dim || a_dim == 1 || b_dim == 1 {
            // A length-1 axis takes the *other* axis's length, which is not
            // its maximum when the other one is empty: `max` read
            // `[ ] 1 ADD` (shapes `[0]` and `[]`, the scalar broadcast to
            // `[1]`) as a one-lane result and then indexed lane 0 of a
            // zero-lane tensor — a panic, which is no outcome at all and so
            // outside LANG.FAILURE.TRICHOTOMY entirely. Stretching a
            // one-element axis over an empty one yields an empty axis.
            *slot = if a_dim == 1 { b_dim } else { a_dim };
        } else {
            return Err(i);
        }
    }

    Ok(out)
}

pub(crate) fn build_nested_value(data: &[Fraction], shape: &[usize]) -> Value {
    if shape.is_empty() {
        if data.len() == 1 {
            return Value::new(ValueData::Scalar(data[0].clone()), None);
        }
        let children: Vec<Value> = data
            .iter()
            .map(|f| Value::from_fraction(f.clone()))
            .collect();
        return Value::from_children(children);
    }

    if shape.len() == 1 {
        let children: Vec<Value> = data
            .iter()
            .map(|f| Value::from_fraction(f.clone()))
            .collect();
        return Value::new(ValueData::Vector(Arc::new(children)), None);
    }

    let outer_size: usize = shape[0];
    let inner_shape = &shape[1..];
    let inner_size: usize = inner_shape.iter().product();

    let children: Vec<Value> = (0..outer_size)
        .map(|i| {
            let start = i * inner_size;
            let end = start + inner_size;
            build_nested_value(&data[start..end], inner_shape)
        })
        .collect();

    Value::new(ValueData::Vector(Arc::new(children)), None)
}

/// The rectangular tensor shape of `value`, or `None` when the value cannot
/// be faithfully represented as a flat tensor.
///
/// A value is rectangular when every leaf is a numeric scalar (or NIL lane)
/// and all sibling sub-vectors share an identical shape. Ragged structures —
/// mixed scalar/vector siblings (e.g. `[ 10 [ 1 2 3 ] 10 ]`) or sub-vectors
/// of differing shape — return `None`. Such values must be broadcast
/// structurally (see [`apply_recursive_broadcast`]) rather than flattened,
/// because `shape()` collapses them to a top-level count that disagrees with
/// the recursively flattened element count.
pub(crate) fn rectangular_shape(value: &Value) -> Option<Vec<usize>> {
    // The logical Unknown (U — `Nil` carrying the `TruthValue` hint) has no
    // dedicated variant, so it is a rectangular nil lane here too, same as an
    // operational NIL.
    rectangular_shape_by(value, |leaf| {
        matches!(
            leaf.data,
            ValueData::Scalar(_) | ValueData::ExactScalar(_) | ValueData::Nil(_)
        )
    })
}

/// The shape of a rectangular nesting whose leaves `is_leaf` admits, or
/// `None` for a ragged one or a leaf it does not.
///
/// A leaf has the empty shape; an empty Vector has the shape `[0]`; a Vector
/// whose children all share one shape prefixes its own length to theirs; a
/// dense `Tensor` is rectangular by construction and already knows its
/// shape. The numeric broadcast admits numeric leaves only
/// ([`rectangular_shape`]); `SHAPE` admits every leaf
/// (`shape_words::rectangular_shape`). The walk is the same, and used to be
/// written twice.
pub(crate) fn rectangular_shape_by(
    value: &Value,
    is_leaf: impl Fn(&Value) -> bool + Copy,
) -> Option<Vec<usize>> {
    match &value.data {
        ValueData::Tensor { shape, .. } => Some((**shape).clone()),
        ValueData::Vector(items) => {
            if items.is_empty() {
                return Some(vec![0]);
            }
            let first: Vec<usize> = rectangular_shape_by(&items[0], is_leaf)?;
            for item in items.iter().skip(1) {
                if rectangular_shape_by(item, is_leaf)? != first {
                    return None;
                }
            }
            let mut shape = Vec::with_capacity(first.len() + 1);
            shape.push(items.len());
            shape.extend(first);
            Some(shape)
        }
        _ if is_leaf(value) => Some(Vec::new()),
        _ => None,
    }
}

/// One level of children for a value, or `None` for a leaf (scalar/NIL) or a
/// non-broadcastable value. Dense tensors are decomposed into their outermost
/// rows so that recursive broadcasting treats them like nested vectors.
pub(crate) fn broadcast_children(value: &Value) -> Option<Vec<Value>> {
    match &value.data {
        ValueData::Vector(items) => Some(items.as_ref().clone()),
        // One row per outermost index, through `Value::child` — the
        // reason-preserving materialization. Rebuilding the rows from
        // `to_fractions()` instead dropped every absent lane's reason here,
        // one level above the lane laws that were taught to keep it.
        ValueData::Tensor { shape, .. } => {
            let outer = *shape.first()?;
            Some((0..outer).filter_map(|row| value.child(row)).collect())
        }
        _ => None,
    }
}

/// The numeric leaf fraction of a value, or `None` when the value is not a
/// numeric leaf. A NIL is not one: the lane-wise lift settles absence while
/// the lane is still a `Value`.
pub(crate) fn broadcast_leaf(value: &Value) -> Option<Fraction> {
    match &value.data {
        ValueData::Scalar(f) => Some(f.clone()),
        _ => None,
    }
}

/// Structural element-wise broadcast for ragged or nested values.
///
/// Mirrors NumPy-style scalar broadcasting but follows the actual value tree
/// instead of a flattened tensor, so it stays correct when scalars and
/// vectors are mixed as siblings or sub-vectors have differing shapes. A
/// scalar paired with a vector is broadcast across every element; two vectors
/// pair by shape, and a ragged one pairs with no vector
/// (`broadcast_tree::broadcast_tree`). The leaf operation is the same `op` used by the
/// flat path, so NIL-lane handling is identical.
fn apply_recursive_broadcast<F>(a: &Value, b: &Value, op: F) -> Result<Value>
where
    F: Fn(&Fraction, &Fraction) -> Result<Fraction> + Copy,
{
    apply_lane_wise_broadcast(a, b, |x, y| op(x, y).map(Value::from_fraction))
}

/// Element-wise binary operation over two values with tensor broadcasting.
pub(crate) fn apply_binary_broadcast<F>(a: &Value, b: &Value, op: F) -> Result<Value>
where
    F: Fn(&Fraction, &Fraction) -> Result<Fraction> + Copy + Sync,
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

    // Ragged or nested-mixed structures (e.g. `[ 10 [ 1 2 3 ] 10 ]`) cannot be
    // flattened to a single tensor whose shape matches its element count, so
    // they are broadcast structurally by following the value tree. An operand
    // carrying an absent lane takes that route too: this kernel answers each
    // lane with a `Fraction`, and a `Fraction` records absence as a zero
    // denominator with no reason attached, so every NIL it produced read back
    // as `nil:literal` however it had really arisen. The structural route
    // keeps each lane a `Value` and lifts the scalar passthrough law over it
    // (`tensor_lane_ops::lane_nil_passthrough`).
    if rectangular_shape(a).is_none()
        || rectangular_shape(b).is_none()
        || contains_absent_lane(a)
        || contains_absent_lane(b)
    {
        return apply_recursive_broadcast(a, b, op);
    }

    let tensor_a = FlatTensor::from_value(a)?;
    let tensor_b = FlatTensor::from_value(b)?;

    let out_shape = broadcast_shape(&tensor_a.shape, &tensor_b.shape)?;
    let out_size: usize = if out_shape.is_empty() {
        1
    } else {
        out_shape.iter().product()
    };

    if tensor_a.shape == tensor_b.shape {
        // Compute-bound same-shape element-wise op. The per-lane exact-rational
        // arithmetic (num/den cross-multiply + gcd) is the robust parallel
        // scaling target (手4); fan-out is gated by the compute-bound floor and
        // is transparent — the result `Vec<Fraction>` is assembled through the
        // identical `FlatTensor` constructor the sequential lane uses, so the
        // output is structurally identical regardless of worker count.
        let data_a = &tensor_a.data;
        let data_b = &tensor_b.data;
        let out_data: Vec<_> = (0..out_size)
            .map(|i| op(&data_a[i], &data_b[i]))
            .collect::<Result<Vec<_>>>()?;
        let out_tensor = FlatTensor::from_shape_and_data(out_shape, out_data)?;
        return Ok(out_tensor.to_value());
    }

    let out_strides = compute_strides(&out_shape);
    let mut out_data = Vec::with_capacity(out_size);

    for linear in 0..out_size {
        let out_index = unravel_index(linear, &out_shape, &out_strides);

        let a_index = project_broadcast_index(&out_index, &out_shape, &tensor_a.shape);
        let b_index = project_broadcast_index(&out_index, &out_shape, &tensor_b.shape);

        let a_offset = ravel_index(&a_index, &tensor_a.strides);
        let b_offset = ravel_index(&b_index, &tensor_b.strides);

        out_data.push(op(&tensor_a.data[a_offset], &tensor_b.data[b_offset])?);
    }

    let out_tensor = FlatTensor::from_shape_and_data(out_shape, out_data)?;
    Ok(out_tensor.to_value())
}

/// Structural element-wise unary map for ragged or nested values, mirroring
/// [`apply_recursive_broadcast`]. Follows the value tree instead of a
/// flattened tensor so it stays correct when scalars and vectors are mixed as
/// siblings.
fn apply_recursive_unary<F>(val: &Value, op: F) -> Result<Value>
where
    F: Fn(&Fraction) -> Fraction + Copy,
{
    match broadcast_children(val) {
        Some(children) => {
            let out: Vec<Value> = children
                .iter()
                .map(|child| apply_recursive_unary(child, op))
                .collect::<Result<Vec<Value>>>()?;
            Ok(Value::from_children(out))
        }
        None => {
            let Some(f) = broadcast_leaf(val) else {
                return Err(AjisaiError::declared(
                    "nonNumeric",
                    format!("expected a Scalar or a Vector, got {}", val.domain_name()),
                ));
            };
            Ok(Value::from_fraction(op(&f)))
        }
    }
}

/// Element-wise unary operation over a rectangular value, flattened to one
/// tensor pass.
pub(crate) fn apply_unary_flat<F>(val: &Value, op: F) -> Result<Value>
where
    F: Fn(&Fraction) -> Fraction + Copy,
{
    // Ragged or nested-mixed structures cannot be flattened to a tensor whose
    // shape matches its element count, so map over the value tree directly.
    if rectangular_shape(val).is_none() {
        return apply_recursive_unary(val, op);
    }

    let tensor = FlatTensor::from_value(val)?;
    let result_data: Vec<Fraction> = tensor.data.into_iter().map(|f| op(&f)).collect();
    let result_tensor = FlatTensor::from_shape_and_data(tensor.shape, result_data)?;
    Ok(result_tensor.to_value())
}

#[cfg(kani)]
#[path = "tensor_ops_proofs.rs"]
mod proofs;
