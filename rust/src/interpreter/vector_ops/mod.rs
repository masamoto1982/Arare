//! The Vector Words that address positions and counts: `GET`, `LENGTH`,
//! `TAKE`, `DROP`; `structure.rs` holds `CONCAT`, `REVERSE`, `RANGE`,
//! `COLLECT`.

pub mod structure;

#[cfg(kani)]
mod split_proofs;
#[cfg(test)]
mod tests;

pub use structure::{op_collect, op_concat, op_range, op_reverse};

use crate::error::{AjisaiError, NilReason, Result};
use crate::interpreter::value_extraction_helpers::{extract_position_from_value, normalize_index};
use crate::interpreter::Interpreter;
use crate::types::fraction::Fraction;
use crate::types::Value;

/// Materialize the children of an iterable `Value` (Vector / Record / Tensor)
/// into an owned `Vec<Value>`. Non-iterable values produce an empty `Vec`.
///
/// Implemented on top of [`Value::as_vector_view`], which keeps the borrow vs.
/// owned distinction explicit at the helper level.
pub(crate) fn extract_vector_elements(val: &Value) -> Vec<Value> {
    val.as_vector_view()
        .map(|cow| cow.into_owned())
        .unwrap_or_default()
}

pub(crate) fn with_stacktop_vector_target_with_arg<R, F>(
    interp: &mut Interpreter,
    arg_to_restore: &Value,
    action: F,
) -> Result<R>
where
    F: FnOnce(&Value) -> Result<R>,
{
    let target_val = interp.stack.pop().ok_or_else(|| {
        interp.stack.push(arg_to_restore.clone());
        AjisaiError::stack_underflow()
    })?;

    if !target_val.is_vector() {
        let got = target_val.domain_name();
        interp.stack.push(target_val);
        interp.stack.push(arg_to_restore.clone());
        return Err(AjisaiError::declared(
            "nonVector",
            format!("expected a Vector, got {got}"),
        ));
    }

    match action(&target_val) {
        Ok(result) => Ok(result),
        Err(error) => {
            interp.stack.push(target_val);
            interp.stack.push(arg_to_restore.clone());
            Err(error)
        }
    }
}

pub(crate) fn with_stacktop_vector_target_no_arg<R, F>(
    interp: &mut Interpreter,
    action: F,
) -> Result<R>
where
    F: FnOnce(&Value) -> Result<R>,
{
    let target_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    if !target_val.is_vector() {
        let got = target_val.domain_name();
        interp.stack.push(target_val);
        return Err(AjisaiError::declared(
            "nonVector",
            format!("expected a Vector, got {got}"),
        ));
    }

    match action(&target_val) {
        Ok(result) => Ok(result),
        Err(error) => {
            interp.stack.push(target_val);
            Err(error)
        }
    }
}

/// `extract_position_from_value`, with a structurally malformed index operand
/// reclassified as the declared `invalidInteger` — GET's own condition for "not
/// itself a well-formed index (non-integer, wrong shape)", distinct from
/// `indexOutOfBounds` (a well-formed index outside bounds, which is a NIL
/// projection, not this ERROR). The message names an index, which is what
/// this caller of the shared helper knows the integer was for.
fn require_index_operand(value: &Value) -> Result<i64> {
    extract_position_from_value(value).map_err(|e| {
        AjisaiError::declared(
            "invalidInteger",
            format!("expected a well-formed index, got {}", e.got),
        )
    })
}

/// `GET ( [ container ] [ key ] -> [ value ] )`: the element of a Vector at an
/// index, or the value of a Record under a key — one Word for reading a
/// container, as `PUT` is one Word for writing one.
///
/// The key is a `leaf`, so a Vector of indices or keys lifts at dispatch
/// (LANG.COLLECTIONS.LIFT) and this primitive only ever sees one. What names
/// nothing projects: an index outside the Vector is `indexOutOfBounds`, a key
/// the Record does not hold is `notFound`.
pub fn op_get(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 2 {
        return Err(AjisaiError::stack_underflow());
    }
    let key = interp.stack.pop().expect("checked by len()");
    let target = interp.stack.pop().expect("checked by len()");
    let restore = |interp: &mut Interpreter, target: Value, key: Value| {
        interp.stack.push(target);
        interp.stack.push(key);
    };

    if let Some(record) = target.as_record() {
        let answer = match record.get(&key) {
            Some(value) => value.clone(),
            None => Value::nil_with_reason(NilReason::NotFound),
        };
        interp.stack.push(answer);
        return Ok(());
    }

    if !target.is_vector() {
        let got = target.domain_name();
        restore(interp, target, key);
        return Err(AjisaiError::declared(
            "nonContainer",
            format!("expected a Vector or a Record, got {got}"),
        ));
    }

    let index = match require_index_operand(&key) {
        Ok(index) => index,
        Err(e) => {
            restore(interp, target, key);
            return Err(e);
        }
    };

    // One element is selected, so one element is priced: `GET` is the one
    // Word in the family whose cost does not track the operand it is handed.
    if let Err(e) = crate::interpreter::collection_meter::charge_copy_of(interp, &target, 1) {
        restore(interp, target, key);
        return Err(e);
    }

    let len = target.len();
    let answer = if len == 0 {
        None
    } else {
        normalize_index(index, len)
    }
    .and_then(|position| target.child(position))
    .unwrap_or_else(|| Value::nil_with_reason(NilReason::IndexOutOfBounds));
    interp.stack.push(answer);
    Ok(())
}

/// The slice `TAKE` answers with, or `None` when the count names a position
/// past the end.
///
/// Asking for more than there is, is an index question — the same question
/// `GET` answers past the end, and it is answered the same way: with a
/// reasoned absence rather than a raise. A well-formed count over a
/// well-formed Vector is data that did not work out, not a malformed program,
/// and LANG.FAILURE.PROJECT is what the trichotomy reserves for that. `TAKE`
/// raising where `GET` projected was the standing example of the two
/// disagreeing (`spec/words.schema.json`'s note on `errorWhen`); they now
/// agree.
///
/// Compare the magnitude in u64 before narrowing to usize. `(-count) as
/// usize` overflowed and panicked on i64::MIN (reachable via
/// `[ .. ] -9223372036854775808 TAKE`), and a bare `count as usize` would
/// silently truncate a huge count on 32-bit wasm. Working in u64 keeps both
/// the past-the-end test and the eventual narrowing exact.
fn compute_take_bounds(len: usize, count: i64) -> Option<(usize, usize)> {
    let magnitude: u64 = count.unsigned_abs();
    if magnitude > len as u64 {
        return None;
    }
    let take = magnitude as usize;

    if count < 0 {
        Some((len - take, len))
    } else {
        Some((0, take))
    }
}

/// The slice `DROP` answers with: everything `TAKE` would not have, on the
/// same count over the same Vector. So the two agree about the one thing they
/// share — when a count names a position past the end — because they read it
/// from the same place.
fn compute_drop_bounds(len: usize, count: i64) -> Option<(usize, usize)> {
    compute_take_bounds(len, count).map(
        |(start, end)| {
            if count < 0 {
                (0, start)
            } else {
                (end, len)
            }
        },
    )
}

/// Which half of the split a count addresses: `TAKE` answers the addressed
/// prefix (or suffix), `DROP` answers the rest.
#[derive(Clone, Copy)]
enum Split {
    Take,
    Drop,
}

impl Split {
    fn bounds(self, len: usize, count: i64) -> Option<(usize, usize)> {
        match self {
            Split::Take => compute_take_bounds(len, count),
            Split::Drop => compute_drop_bounds(len, count),
        }
    }

    /// How many elements the answer copies out of a Vector of `len`: the
    /// addressed count for `TAKE`, the remainder for `DROP`. Clamped here only
    /// for pricing; `bounds` still decides whether an over-long count projects.
    fn copied(self, len: usize, wanted: usize) -> usize {
        match self {
            Split::Take => wanted.min(len),
            Split::Drop => len.saturating_sub(wanted),
        }
    }
}

pub fn op_length(interp: &mut Interpreter) -> Result<()> {
    // `LENGTH` is `[ vec ] -> [ count ]` and consumes what it reads: the
    // measured vector leaves the stack.
    let target_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    let len = {
        if target_val.is_nil() {
            0
        } else if target_val.is_vector() {
            // `Value::len()` reads the count. This used to call
            // `extract_vector_elements`, which deep-copies the whole element
            // vector out of the `Cow` and then asks the copy for its length —
            // 18 ms and 100,000 element clones to answer a question the header
            // already holds. Measured by
            // `examples/collection_word_calibration`; a Word documented O(1)
            // was the third most expensive linear Word in the family.
            target_val.len()
        } else {
            let got = target_val.domain_name();
            interp.stack.push(target_val);
            return Err(AjisaiError::declared(
                "nonVector",
                format!("expected a Vector, got {got}"),
            ));
        }
    };
    let len_frac = Fraction::from(len as i64);
    interp.stack.push(Value::from_fraction(len_frac));
    Ok(())
}

pub fn op_take(interp: &mut Interpreter) -> Result<()> {
    split_by_count(interp, Split::Take)
}

pub fn op_drop(interp: &mut Interpreter) -> Result<()> {
    split_by_count(interp, Split::Drop)
}

/// `TAKE` and `DROP` are one Word up to which side of the cut they answer
/// with, so they are one executor: same operand reading, same `invalidInteger`
/// on a count that is not an integer, same projection past the end.
fn split_by_count(interp: &mut Interpreter, split: Split) -> Result<()> {
    let count_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    let count = match extract_position_from_value(&count_val) {
        Ok(v) => v,
        // `invalidInteger`: TAKE's own declared condition for a count operand
        // that isn't a well-formed integer.
        Err(e) => {
            interp.stack.push(count_val);
            return Err(AjisaiError::declared(
                "invalidInteger",
                format!("expected an integer count, got {}", e.got),
            ));
        }
    };

    // Priced on what the answer copies, not on the vector it was handed:
    // taking 100 elements out of 100,000 copies 100 of them, and dropping 100
    // copies the other 99,900.
    let wanted = usize::try_from(count.unsigned_abs()).unwrap_or(usize::MAX);
    if let Err(e) = crate::interpreter::collection_meter::charge_stacktop_copy(interp, |len| {
        split.copied(len, wanted)
    }) {
        interp.stack.push(count_val);
        return Err(e);
    }

    let result = with_stacktop_vector_target_with_arg(interp, &count_val, |vector_val| {
        Ok(split
            .bounds(vector_val.len(), count)
            .map(|(start, end)| vector_val.children_range(start, end))
            .unwrap_or_else(|| Value::nil_with_reason(NilReason::IndexOutOfBounds)))
    })?;

    interp.stack.push(result);
    Ok(())
}
