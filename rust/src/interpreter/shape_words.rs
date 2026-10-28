//! Shape Words: `SHAPE`, `RESHAPE`, `FLATTEN`, `DEPTH`, `ZIP` and `PUT`
//! (LANG.COLLECTIONS.LIFT).
//!
//! The lifting law already reasons about a Vector's shape — the lengths of its
//! axes, outermost first — and the runtime already carries one for a dense
//! `Tensor`. Until these Words a program could observe none of it: `LENGTH`
//! answers the outermost axis and nothing below it. Two of the four are also
//! the clearest instances of the vocabulary-100 admission test
//! (docs/dev/ajisai-minimal-core-identity.md 付録 B): how deeply a value nests
//! is not known in advance, and a language with no recursion and no unbounded
//! loop cannot walk a structure of unknown depth, so `FLATTEN` and `DEPTH`
//! cannot be written as user definitions at any cost.

use super::ordering_ops::{elements_of, restore, take_operand};
use super::tensor_cmds::checked_materialized_count;
use crate::error::{AjisaiError, NilReason, Result};
use crate::interpreter::collection_meter::charge_materialization;
use crate::interpreter::value_extraction_helpers::{extract_position_from_value, normalize_index};
use crate::interpreter::Interpreter;
use crate::types::fraction::saturating_narrow;
use crate::types::{Value, ValueData};

/// The shape of a rectangular nesting, or `None` for a ragged one.
///
/// Every non-Vector — a Text included — is a leaf here, where the numeric
/// broadcast's `tensor_ops::rectangular_shape` admits numeric leaves only;
/// the walk itself is `rectangular_shape_by`'s.
fn rectangular_shape(value: &Value) -> Option<Vec<usize>> {
    crate::interpreter::tensor_ops::rectangular_shape_by(value, |_| true)
}

/// How deeply a value nests: a leaf is 0, a Vector one more than its deepest
/// element, so an empty Vector is 1.
pub(crate) fn depth_of(value: &Value) -> usize {
    if let ValueData::Tensor { shape, .. } = &value.data {
        return shape.len();
    }
    match value.as_vector_view() {
        None => 0,
        Some(children) => 1 + children.iter().map(depth_of).max().unwrap_or(0),
    }
}

/// Every leaf under `value`, in index order.
fn flatten_into(value: &Value, out: &mut Vec<Value>) {
    match value.as_vector_view() {
        None => out.push(value.clone()),
        Some(children) => {
            for child in children.iter() {
                flatten_into(child, out);
            }
        }
    }
}

/// `leaves`, regrouped under `shape` in row-major order. The caller has already
/// checked that the product of `shape` is `leaves.len()`. The empty shape is
/// rank 0 — the lone leaf itself, as `5 SHAPE` is `[ ]` — and an axis of
/// length 0 answers empty Vectors below it, as `[ [ ] [ ] ] SHAPE` is `[ 2 0 ]`.
pub(super) fn regroup(leaves: &[Value], shape: &[usize]) -> Value {
    match shape.split_first() {
        None => leaves[0].clone(),
        Some((_, [])) => Value::from_vector_promoted(leaves.to_vec()),
        Some((&outer, inner)) => {
            let stride: usize = inner.iter().product();
            Value::from_vector_promoted(
                (0..outer)
                    .map(|i| regroup(&leaves[i * stride..(i + 1) * stride], inner))
                    .collect(),
            )
        }
    }
}

/// A shape operand: a Vector of non-negative integers — exactly what `SHAPE`
/// answers, the empty shape of a leaf and a zero-length axis included — or
/// `None` for anything else (`invalidShape`).
///
/// Each axis is read as a `u64` on every target and narrowed saturating. An
/// axis a 32-bit `usize` cannot hold is still a well-formed axis, too long to
/// build, so it reaches the size check as one: declining it here made
/// `[ 4294967296 ] 0 FILL` an `invalidShape` on wasm32 and a ceiling refusal
/// natively.
pub(super) fn parse_shape(shape_val: &Value) -> Option<Vec<usize>> {
    Some(
        shape_axes(shape_val)?
            .into_iter()
            .map(saturating_narrow)
            .collect(),
    )
}

fn shape_axes(shape_val: &Value) -> Option<Vec<u64>> {
    shape_val
        .as_vector_view()?
        .iter()
        .map(|dim| dim.as_scalar()?.as_u64())
        .collect()
}

/// The element count a shape names, as a `materializedElements` refusal reports
/// it: the product of the axes as written, in `u64` on every target (`None`
/// past it), so the observed size is one number on wasm32 and natively. The
/// product of `parse_shape`'s saturated axes is not that number on wasm32.
/// Past a zero-length axis it counts the Vectors built above that axis, as
/// [`checked_materialized_count`] does for the ceiling itself.
pub(super) fn shape_observed_size(shape_val: &Value) -> Option<u128> {
    shape_axes(shape_val)?
        .iter()
        .take_while(|&&axis| axis != 0)
        .try_fold(1u64, |acc, &axis| acc.checked_mul(axis))
        .map(u128::from)
}

/// `SHAPE ( [ vec ] -> [ shape ] )`: the axis lengths of a rectangular Vector,
/// outermost first; a ragged Vector has no shape and projects `domainMiss`.
pub fn op_shape(interp: &mut Interpreter) -> Result<()> {
    let value = take_operand(interp)?;
    // A value that is not a Vector has no axes: its shape is the empty one,
    // rank 0, the shape `DEPTH` already reports as depth 0.
    if !value.is_vector() {
        interp.stack.push(Value::from_vector(Vec::new()));
        return Ok(());
    }
    let answer = match rectangular_shape(&value) {
        Some(shape) => Value::from_vector_promoted(
            shape
                .into_iter()
                .map(|axis| Value::from_int(axis as i64))
                .collect(),
        ),
        // A ragged Vector is well-formed data that the question does not fit:
        // the trichotomy's middle case, not a malformed program.
        None => Value::nil_with_reason(NilReason::DomainMiss),
    };
    interp.stack.push(answer);
    Ok(())
}

/// `DEPTH ( [ x ] -> [ n ] )`: how deeply a value nests. Total over every
/// value, NIL included (a leaf, so 0).
pub fn op_depth(interp: &mut Interpreter) -> Result<()> {
    let value = take_operand(interp)?;
    let depth = depth_of(&value);
    interp.stack.push(Value::from_int(depth as i64));
    Ok(())
}

/// `FLATTEN ( [ vec ] -> [ flat ] )`: every leaf in index order, all axes
/// collapsed into one.
pub fn op_flatten(interp: &mut Interpreter) -> Result<()> {
    let value = take_operand(interp)?;
    let children = match elements_of(&value, "a Vector") {
        Ok(children) => children,
        Err(e) => {
            restore(interp, value);
            return Err(e);
        }
    };
    let mut leaves = Vec::new();
    for child in &children {
        flatten_into(child, &mut leaves);
    }
    if let Err(e) = charge_materialization(interp, leaves.len()) {
        restore(interp, value);
        return Err(e);
    }
    interp.stack.push(Value::from_vector_promoted(leaves));
    Ok(())
}

/// `RESHAPE ( [ vec ] [ shape ] -> [ reshaped ] )`: the Vector's leaves, in
/// order, regrouped under `shape`. The product of the shape must equal the
/// leaf count — nothing is padded or repeated — and a well-formed shape too
/// large to materialize is refused as `resourceLimitExceeded`, as `FILL` and
/// `RANGE` refuse one.
pub fn op_reshape(interp: &mut Interpreter) -> Result<()> {
    let shape_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    let target = match interp.stack.pop() {
        Some(target) => target,
        None => {
            interp.stack.push(shape_val);
            return Err(AjisaiError::stack_underflow());
        }
    };
    let put_back = |interp: &mut Interpreter, target: Value, shape_val: Value| {
        interp.stack.push(target);
        interp.stack.push(shape_val);
    };

    let children = match elements_of(&target, "a Vector") {
        Ok(children) => children,
        Err(e) => {
            put_back(interp, target, shape_val);
            return Err(e);
        }
    };

    let Some(shape) = parse_shape(&shape_val) else {
        put_back(interp, target, shape_val);
        return Err(AjisaiError::declared(
            "invalidShape",
            "expected a shape: a Vector of non-negative integers",
        ));
    };
    // A shape's rank is the nesting of the value it builds, so a rank past the
    // nesting ceiling is refused before the value is built — building it is
    // itself a walk one native frame per axis (`regroup`) — the way a count
    // past the materialization ceiling is.
    let max_nesting = interp.runtime_limits.max_nesting_depth;
    if shape.len() > max_nesting {
        let rank = shape.len();
        put_back(interp, target, shape_val);
        return Err(crate::interpreter::ceiling_refusal::nesting_refused(
            max_nesting,
            rank,
        ));
    }

    let max_materialized = interp.runtime_limits.max_materialized_elements;
    let materialized = match checked_materialized_count(&shape) {
        Some(count) if count <= max_materialized => count,
        _ => {
            // The same refusal FILL makes for the same reason: a well-formed
            // request past a host ceiling (LANG.COLLECTIONS.BUDGET).
            let observed = shape_observed_size(&shape_val);
            put_back(interp, target, shape_val);
            return Err(
                crate::interpreter::ceiling_refusal::materialization_refused(
                    max_materialized,
                    observed,
                ),
            );
        }
    };

    // Within the ceiling just checked, so the leaf product cannot overflow.
    let total: usize = shape.iter().product();

    let mut leaves = Vec::new();
    for child in &children {
        flatten_into(child, &mut leaves);
    }
    if leaves.len() != total {
        put_back(interp, target, shape_val);
        return Err(AjisaiError::declared(
            "invalidShape",
            format!(
                "the shape holds {} element(s) but the Vector has {} leaf value(s)",
                total,
                leaves.len()
            ),
        ));
    }
    if let Err(e) = charge_materialization(interp, materialized) {
        put_back(interp, target, shape_val);
        return Err(e);
    }

    let result = regroup(&leaves, &shape);
    interp.stack.push(result);
    Ok(())
}

// Shape Words: `ZIP` and `PUT`.
//
// The companion module to `ordering_ops`, on the same reasoning: each of these
// is expressible in the existing vocabulary, and each one written that way
// costs asymptotically more.
/// `extract_position_from_value`, with a non-integer operand raised as the
/// declared `invalidInteger` that every integer-taking Word shares.
fn require_integer_operand(value: &Value) -> Result<i64> {
    extract_position_from_value(value).map_err(|e| {
        AjisaiError::declared(
            "invalidInteger",
            format!("expected an integer, got {}", e.got),
        )
    })
}

/// `ZIP ( [ [ vec... ] ] -> [ [ tuple... ] ] )`: bundle equal-length vectors
/// position by position.
///
/// Three jobs in one Word. Walking features beside labels is
/// `XS YS 2 COLLECT ZIP { .. } MAP`; transposing a matrix is `M ZIP`; and
/// walking a vector with its own indices is
/// `0 N RANGE XS 2 COLLECT ZIP`. All three previously went through
/// `1 COLLECT 'IV' BIND XS IV GET`, the index-plumbing phrase that made up
/// roughly half the tokens of any program that had to look two things up at
/// the same position.
pub fn op_zip(interp: &mut Interpreter) -> Result<()> {
    let value = take_operand(interp)?;
    if value.is_nil() {
        interp.stack.push(value);
        return Ok(());
    }

    let rows = match elements_of(&value, "vector of vectors") {
        Ok(rows) => rows,
        Err(e) => {
            restore(interp, value);
            return Err(e);
        }
    };

    if rows.is_empty() {
        interp.stack.push(Value::from_vector(Vec::new()));
        return Ok(());
    }

    let mut columns: Vec<Vec<Value>> = Vec::new();
    for row in &rows {
        match row.as_vector_view() {
            Some(view) => columns.push(view.into_owned()),
            None => {
                let got = row.domain_name();
                restore(interp, value);
                return Err(AjisaiError::declared(
                    "nonVector",
                    format!("expected every row to be a Vector, got {got}"),
                ));
            }
        }
    }

    let width = columns[0].len();
    if let Some(other) = columns.iter().find(|column| column.len() != width) {
        restore(interp, value);
        return Err(AjisaiError::length_mismatch(width, other.len()));
    }

    // A transpose copies every cell exactly once, so the price is the whole
    // grid: rows x width, measured on the rows rather than on the outer vector
    // (whose "elements" are the rows themselves).
    let cell_units = columns
        .iter()
        .map(|column| {
            crate::interpreter::collection_meter::element_cost_of_slice(column).copies(column.len())
        })
        .fold(0u64, u64::saturating_add);
    if let Err(e) = crate::interpreter::collection_meter::charge(interp, cell_units) {
        restore(interp, value);
        return Err(e);
    }

    let out: Vec<Value> = (0..width)
        .map(|position| {
            Value::from_vector(
                columns
                    .iter()
                    .map(|column| column[position].clone())
                    .collect(),
            )
        })
        .collect();
    interp.stack.push(Value::from_vector(out));
    Ok(())
}

/// `PUT ( [ container ] [ key ] [ value ] -> [ container' ] )`: a copy of a
/// Vector with the element at an index replaced, or of a Record with a key
/// set — one Word for writing a container, as `GET` is one Word for reading
/// one. A negative index counts from the end, as everywhere else. A Record key
/// already present is replaced in place and an absent one is appended: a
/// Record's keys are its own to extend, where a Vector's positions are fixed
/// by its length, so an index past the end projects `indexOutOfBounds`.
///
/// The alternative was rebuilding the whole vector from an index mask and a
/// `SELECT`, or the `TAKE`/`CONCAT` surgery that spells the same thing in
/// three phrases. Updating one position is what a parameter step, a centroid
/// move, a confusion-matrix increment and a histogram bin all do.
pub fn op_put(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 3 {
        return Err(AjisaiError::stack_underflow());
    }
    let replacement = interp.stack.pop().expect("checked by len()");
    let index_value = interp.stack.pop().expect("checked by len()");
    let target = interp.stack.pop().expect("checked by len()");

    // Either container is answered as a copy with one entry changed, so the
    // whole of it is priced however small the edit is.
    if let Some(record) = target.as_record() {
        let entries = record.len();
        if let Err(e) =
            crate::interpreter::collection_meter::charge_copy_of(interp, &target, entries)
        {
            interp.stack.push(target);
            interp.stack.push(index_value);
            interp.stack.push(replacement);
            return Err(e);
        }
        let next = record.with(index_value, replacement);
        interp.stack.push(Value::from_record(next));
        return Ok(());
    }

    // `PUT` answers with a copy of the vector, one element replaced, so it
    // copies the whole thing however small the edit is.
    if let Err(e) =
        crate::interpreter::collection_meter::charge_copy_of(interp, &target, target.len())
    {
        interp.stack.push(target);
        interp.stack.push(index_value);
        interp.stack.push(replacement);
        return Err(e);
    }

    let mut items = match target.as_vector_view() {
        Some(view) => view.into_owned(),
        None => {
            let got = target.domain_name();
            interp.stack.push(target);
            interp.stack.push(index_value);
            interp.stack.push(replacement);
            return Err(AjisaiError::declared(
                "nonContainer",
                format!("expected a Vector or a Record, got {got}"),
            ));
        }
    };

    let raw_index = match require_integer_operand(&index_value) {
        Ok(index) => index,
        Err(e) => {
            interp.stack.push(target);
            interp.stack.push(index_value);
            interp.stack.push(replacement);
            return Err(e);
        }
    };

    // `normalize_index` checks the bounds in `i64` before narrowing: an
    // `as usize` first truncated `2^32 + 1` to slot 1 on 32-bit wasm.
    let Some(position) = normalize_index(raw_index, items.len()) else {
        // A well-formed index over a well-formed Vector that names no slot is
        // the question `GET` already projects for, and `TAKE` now projects for
        // too: data that did not work out, not a malformed program
        // (LANG.FAILURE.PROJECT). `PUT` used to raise here on the grounds that
        // it answers with the whole Vector and so has no single slot to empty
        // — but the absence is of the *answer*, not of a slot, and that is
        // what a reasoned NIL says.
        interp
            .stack
            .push(Value::nil_with_reason(NilReason::IndexOutOfBounds));
        return Ok(());
    };

    items[position] = replacement;
    interp.stack.push(Value::from_vector(items));
    Ok(())
}
