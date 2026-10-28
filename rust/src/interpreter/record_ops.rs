//! The Record Words: `RECORD`, `KEYS`, `VALUES`, `WITHOUT`, `HAS?`, `MERGE`
//! (LANG.RECORDS.STRUCTURE). Reading and writing one key is `GET` and `PUT`,
//! the same two Words that read and write a Vector position.
//!
//! A Record is the seventh value domain: a keyed correspondence whose keys
//! keep the order they arrived in. It is the one shape the parallel-vector
//! idiom could only imitate — `INDEX-OF` then `GET` scans every key where
//! `GET` on a Record hashes one — and it is what structured data from a host arrives as.
//! Nothing here converts a Vector to a Record or back on its own: `RECORD`
//! and `KEYS`/`VALUES` are the only bridges, both explicit.
//!
//! Every Word puts its operands back before raising (the ERROR discipline of
//! every Core Word), and every Record it hands back is a new value: a Record
//! on the stack is never mutated in place.

use super::ordering_ops::{elements_of, restore, take_operand};
use crate::error::{AjisaiError, NilReason, Result};
use crate::interpreter::collection_meter::ScanMeter;
use crate::interpreter::value_extraction_helpers::extract_operands;
use crate::interpreter::Interpreter;
use crate::types::{RecordBuildError, RecordData, Value};

fn push_record(interp: &mut Interpreter, record: RecordData) {
    interp.stack.push(Value::from_record(record));
}

/// The declared `nonRecord` condition, naming the position and what was
/// found there.
fn non_record(position: &str, got: &Value) -> AjisaiError {
    AjisaiError::declared(
        "nonRecord",
        format!("expected a Record as {position}, got {}", got.domain_name()),
    )
}

/// The `notFound` absence `WITHOUT` projects for a key the
/// Record does not hold.
fn not_found() -> Value {
    Value::nil_with_reason(NilReason::NotFound)
}

/// Charge for hashing every key of a Record being built or rebuilt: one
/// hash-keyed scan, the same price `UNIQUE` pays for the same work.
fn charge_key_scan(interp: &mut Interpreter, keys: &[Value]) -> Result<()> {
    let meter = ScanMeter::new(keys);
    for completed in 0..keys.len() {
        meter.charge_scan_of(interp, completed)?;
        meter.charge_retained(interp, completed)?;
    }
    Ok(())
}

/// `RECORD ( [ keys ] [ values ] -> [ record ] )`.
pub fn op_record(interp: &mut Interpreter) -> Result<()> {
    let operands = extract_operands(interp, 2)?;
    let outcome = (|| {
        let keys = elements_of(&operands[0], "a Vector of keys as the first operand")?;
        let values = elements_of(&operands[1], "a Vector of values as the second operand")?;
        charge_key_scan(interp, &keys)?;
        RecordData::new(keys, values).map_err(|e| match e {
            RecordBuildError::LengthMismatch { keys, values } => {
                AjisaiError::length_mismatch(keys, values)
            }
            RecordBuildError::DuplicateKey { first, second } => AjisaiError::declared(
                "duplicateKey",
                format!(
                    "the key at position {second} repeats the key at position {first}; \
                     a Record holds each key once"
                ),
            ),
        })
    })();
    match outcome {
        Ok(record) => {
            push_record(interp, record);
            Ok(())
        }
        Err(e) => {
            interp.stack.extend(operands);
            Err(e)
        }
    }
}

/// `KEYS ( [ record ] -> [ keys ] )`.
pub fn op_keys(interp: &mut Interpreter) -> Result<()> {
    let operand = take_operand(interp)?;
    let Some(record) = operand.as_record() else {
        let err = non_record("its operand", &operand);
        restore(interp, operand);
        return Err(err);
    };
    let keys = Value::from_vector(record.keys().to_vec());
    interp.stack.push(keys);
    Ok(())
}

/// `VALUES ( [ record ] -> [ values ] )`.
pub fn op_values(interp: &mut Interpreter) -> Result<()> {
    let operand = take_operand(interp)?;
    let Some(record) = operand.as_record() else {
        let err = non_record("its operand", &operand);
        restore(interp, operand);
        return Err(err);
    };
    let values = Value::from_vector(record.values().to_vec());
    interp.stack.push(values);
    Ok(())
}

/// `WITHOUT ( [ record ] [ key ] -> [ record ] )`: projects `notFound`.
pub fn op_without(interp: &mut Interpreter) -> Result<()> {
    let operands = extract_operands(interp, 2)?;
    let Some(record) = operands[0].as_record() else {
        let err = non_record("the first operand", &operands[0]);
        interp.stack.extend(operands);
        return Err(err);
    };
    match record.without(&operands[1]) {
        Some(next) => {
            push_record(interp, next);
        }
        None => {
            interp.stack.push(not_found());
        }
    }
    Ok(())
}

/// `HAS? ( [ record ] [ key ] -> [ TRUE | FALSE ] )`.
pub fn op_has(interp: &mut Interpreter) -> Result<()> {
    let operands = extract_operands(interp, 2)?;
    let Some(record) = operands[0].as_record() else {
        let err = non_record("the first operand", &operands[0]);
        interp.stack.extend(operands);
        return Err(err);
    };
    let present = record.has(&operands[1]);
    interp.stack.push(Value::from_bool(present));
    Ok(())
}

/// `MERGE ( [ record ] [ record ] -> [ record ] )`: right-biased union.
pub fn op_merge(interp: &mut Interpreter) -> Result<()> {
    let operands = extract_operands(interp, 2)?;
    let (Some(left), Some(right)) = (operands[0].as_record(), operands[1].as_record()) else {
        let (position, got) = if operands[0].as_record().is_none() {
            ("the first operand", &operands[0])
        } else {
            ("the second operand", &operands[1])
        };
        let err = non_record(position, got);
        interp.stack.extend(operands);
        return Err(err);
    };
    if let Err(e) = charge_key_scan(interp, right.keys()) {
        interp.stack.extend(operands);
        return Err(e);
    }
    let merged = left.merge(right);
    push_record(interp, merged);
    Ok(())
}

// Lifting the arithmetic Words over Records
// (LANG.COLLECTIONS.LIFT, LANG.RECORDS.STRUCTURE).
//
// A Record lifts in the value direction only: the keys are untouched and the
// result is a Record over the same key sequence. Two Records combine when
// their key sequences are equal, pairing values position by position; a
// Record combines with anything else by applying the Word to each value and
// that other operand. The comparison and logic Words lift over a Record
// through `lane_lift`, and every other Word through the dispatcher
// (`declared_lift`); this is the arithmetic family's entry to the same rule,
// ahead of its tensor path.
//
// The lift is generic over the Word rather than written once per Word: the
// Word's own entry point is run on each value pair on a scratch region of
// the stack, so every scalar and Vector law, every NIL rule, every cost
// charge and every declared ERROR is exactly the one the Word already has.
// Nothing here restates a numeric law.
type WordOp<'a> = &'a dyn Fn(&mut Interpreter) -> Result<()>;

/// Lift a unary Word over a Record operand at the top of the stack. Answers
/// `Ok(false)` when the operand is not a Record, having touched nothing.
pub(crate) fn lift_unary(interp: &mut Interpreter, op: WordOp) -> Result<bool> {
    let is_record = interp
        .stack
        .last()
        .is_some_and(|top| top.as_record().is_some());
    if !is_record {
        return Ok(false);
    }
    let operands = extract_operands(interp, 1)?;
    run_lift(interp, operands, &|interp, operands| {
        let record = operands[0].as_record().expect("checked above");
        record
            .map_values(|value| leaf(interp, vec![value.clone()], op))
            .map(Value::from_record)
    })
}

/// Lift a binary Word over one or two Record operands at the top of the
/// stack. Answers `Ok(false)` when neither operand is a Record, having
/// touched nothing.
pub(crate) fn lift_binary(interp: &mut Interpreter, op: WordOp) -> Result<bool> {
    let len = interp.stack.len();
    if len < 2 {
        return Ok(false);
    }
    let slots = interp.stack.as_slice();
    if slots[len - 2].as_record().is_none() && slots[len - 1].as_record().is_none() {
        return Ok(false);
    }
    let operands = extract_operands(interp, 2)?;
    run_lift(interp, operands, &|interp, operands| {
        combine(interp, &operands[0], &operands[1], op)
    })
}

/// Run `lift` over consumed operands, then push the result or put the
/// operands back on an ERROR.
fn run_lift(
    interp: &mut Interpreter,
    operands: Vec<Value>,
    lift: &dyn Fn(&mut Interpreter, &[Value]) -> Result<Value>,
) -> Result<bool> {
    let result = lift(interp, &operands);
    match result {
        Ok(value) => {
            interp.stack.push(value);
            Ok(true)
        }
        Err(e) => {
            interp.stack.extend(operands);
            Err(e)
        }
    }
}

fn combine(interp: &mut Interpreter, a: &Value, b: &Value, op: WordOp) -> Result<Value> {
    match (a.as_record(), b.as_record()) {
        (Some(left), Some(right)) => {
            if !left.same_keys(right) {
                return Err(AjisaiError::declared(
                    "shapeMismatch",
                    format!(
                        "two Records combine only when their key sequences are equal; \
                         got {} keys against {} keys that do not line up",
                        left.len(),
                        right.len()
                    ),
                ));
            }
            let values = left
                .values()
                .iter()
                .zip(right.values())
                .map(|(x, y)| combine(interp, x, y, op))
                .collect::<Result<Vec<_>>>()?;
            Ok(Value::from_record(
                RecordData::new(left.keys().to_vec(), values)
                    .expect("the keys are the left Record's own, so distinct and aligned"),
            ))
        }
        (Some(left), None) => left
            .map_values(|x| combine(interp, x, b, op))
            .map(Value::from_record),
        (None, Some(right)) => right
            .map_values(|y| combine(interp, a, y, op))
            .map(Value::from_record),
        (None, None) => leaf(interp, vec![a.clone(), b.clone()], op),
    }
}

/// Run the Word on `operands` in a scratch region above the current stack
/// and take its single result back off.
fn leaf(interp: &mut Interpreter, operands: Vec<Value>, op: WordOp) -> Result<Value> {
    let base = interp.stack.len();
    for operand in operands {
        interp.stack.push(operand);
    }
    let outcome = op(interp).map(|()| {
        // Every Word lifted over a Record's values has one declared output,
        // so this is the arity the registry states, not a check on input.
        assert_eq!(
            interp.stack.len(),
            base + 1,
            "a Word lifted over a Record's values leaves exactly one result"
        );
        interp
            .stack
            .pop()
            .expect("the result just asserted to be there")
    });
    interp.stack.truncate(base);
    outcome
}
