use crate::error::{AjisaiError, Result};
use crate::interpreter::record_ops;
use crate::interpreter::value_extraction_helpers::nil_passthrough_unary;
use crate::interpreter::Interpreter;
use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;
use crate::types::{Value, ValueData};

/// Multiply dimension sizes without ever overflowing `usize`. Returns `None`
/// when the running product would wrap, so callers can reject pathological
/// shapes with a structured error instead of panicking (debug) or silently
/// computing a wrong size (release).
pub(super) fn checked_shape_product(shape: &[usize]) -> Option<usize> {
    shape
        .iter()
        .try_fold(1usize, |acc, &dim| acc.checked_mul(dim))
}

/// How many elements building a value of `shape` materializes: the leaf count,
/// or — when an axis is 0 and there are no leaves — the Vectors built above
/// that axis, which the leaf count misses (`[ 9 0 ]` is nine empty Vectors).
/// This is what the materialization ceiling bounds and the meter charges.
/// `None` on overflow, as [`checked_shape_product`].
pub(super) fn checked_materialized_count(shape: &[usize]) -> Option<usize> {
    match shape.iter().position(|&dim| dim == 0) {
        Some(first_zero) => checked_shape_product(&shape[..first_zero]),
        None => checked_shape_product(shape),
    }
}

use super::arithmetic::value_contains_exact_scalar;
use super::tensor_lane_ops::contains_absent_lane;
use super::tensor_ops::{apply_unary_flat, build_nested_value};

fn apply_unary_math<F, G>(interp: &mut Interpreter, op: F, exact_op: G) -> Result<()>
where
    F: Fn(&Fraction) -> Fraction + Copy,
    G: Fn(&ExactReal) -> ExactReal,
{
    if nil_passthrough_unary(interp) {
        return Ok(());
    }

    let val: Value = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    if val.is_nil() {
        let got = val.domain_name();
        interp.stack.push(val);
        return Err(AjisaiError::declared(
            "nonNumeric",
            format!("expected a Scalar or a Vector, got {got}"),
        ));
    }

    if val.is_scalar() {
        if let Some(f) = val.as_scalar() {
            let result: Fraction = op(f);
            interp.stack.push(Value::from_fraction(result));
            return Ok(());
        }
    }

    // ExactScalar path: an algebraic irrational, whose floor and rounding
    // are decidable (LANG.VALUES.EXACT).
    if let ValueData::ExactScalar(er) = &val.data {
        interp.stack.push(Value::from_exact_real(exact_op(er)));
        return Ok(());
    }

    // A NIL lane passes through carrying its reason (LANG.FAILURE.PASSTHROUGH).
    // The flat route below works on bare fractions and would keep the lane
    // absent but drop why, so a vector holding one takes the lane-wise route.
    // An irrational lane has no `Fraction` at all, so it takes the same route:
    // the flat route would drop it and answer a shorter vector than the shape.
    if val.is_vector() && (contains_absent_lane(&val) || value_contains_exact_scalar(&val)) {
        let scalar_op = |lane: &Value| -> Result<Value> {
            if let Some(f) = lane.as_scalar() {
                return Ok(Value::from_fraction(op(f)));
            }
            if let ValueData::ExactScalar(er) = &lane.data {
                return Ok(Value::from_exact_real(exact_op(er)));
            }
            Err(AjisaiError::declared(
                "nonNumeric",
                format!("expected a Scalar or a Vector, got {}", lane.domain_name()),
            ))
        };
        return match crate::interpreter::math_ops::lift_unary_numeric(&val, &scalar_op) {
            Ok(result) => {
                interp.stack.push(result);
                Ok(())
            }
            Err(e) => {
                interp.stack.push(val);
                Err(e)
            }
        };
    }

    if val.is_vector() {
        match apply_unary_flat(&val, op) {
            Ok(result) => {
                interp.stack.push(result);
                return Ok(());
            }
            Err(_) => {
                interp.stack.push(val);
                return Err(AjisaiError::declared(
                    "nonNumeric",
                    "expected a Scalar or a Vector of Scalars",
                ));
            }
        }
    }

    let got = val.domain_name();
    interp.stack.push(val);
    Err(AjisaiError::declared(
        "nonNumeric",
        format!("expected a Scalar or a Vector, got {got}"),
    ))
}

/// `dense_kernels::rounded` on the operand, when it answers.
fn push_rounded_dense(
    interp: &mut Interpreter,
    rounding: crate::interpreter::dense_kernels::Rounding,
) -> bool {
    if !interp.dense_kernels_enabled {
        return false;
    }
    let Some(result) = interp
        .stack
        .last()
        .and_then(|top| crate::interpreter::dense_kernels::rounded(rounding, top))
    else {
        return false;
    };
    interp.stack.pop();
    interp.stack.push(result);
    true
}

pub fn op_floor(interp: &mut Interpreter) -> Result<()> {
    if record_ops::lift_unary(interp, &op_floor)? {
        return Ok(());
    }
    if push_rounded_dense(interp, crate::interpreter::dense_kernels::Rounding::Floor) {
        return Ok(());
    }
    apply_unary_math(interp, |f| f.floor(), |er| er.floor())
}

pub fn op_round(interp: &mut Interpreter) -> Result<()> {
    if record_ops::lift_unary(interp, &op_round)? {
        return Ok(());
    }
    if push_rounded_dense(interp, crate::interpreter::dense_kernels::Rounding::Round) {
        return Ok(());
    }
    apply_unary_math(interp, |f| f.round(), |er| er.round())
}

/// `[ shape ] value FILL` — a Vector of the given shape, every leaf `value`:
/// `[ 2 3 ] 0 FILL` is two rows of three zeros. The shape comes first and the
/// value second, the order `RESHAPE` takes its data and its shape in; the
/// value is a `leaf`, so a Vector of values lifts to one filled Vector each.
pub fn op_fill(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 2 {
        return Err(AjisaiError::stack_underflow());
    }
    let value_val = interp.stack.pop().expect("length checked");
    let shape_val = interp.stack.pop().expect("length checked");
    let restore = |interp: &mut Interpreter, shape_val: Value, value_val: Value| {
        interp.stack.push(shape_val);
        interp.stack.push(value_val);
    };

    let Some(shape) = super::shape_words::parse_shape(&shape_val) else {
        restore(interp, shape_val, value_val);
        return Err(AjisaiError::declared(
            "invalidShape",
            "expected a shape: a Vector of non-negative integers",
        ));
    };
    // The shape's rank is the nesting of the value FILL builds: refused past
    // the nesting ceiling before building, as RESHAPE does.
    let max_nesting = interp.runtime_limits.max_nesting_depth;
    if shape.len() > max_nesting {
        let rank = shape.len();
        restore(interp, shape_val, value_val);
        return Err(crate::interpreter::ceiling_refusal::nesting_refused(
            max_nesting,
            rank,
        ));
    }

    // Compute the element count with overflow protection and reject anything
    // beyond the materialization cap before allocating. `shape.iter().product()`
    // would otherwise panic on a usize overflow (e.g. three ~1e8 dimensions) or
    // drive an OOM abort for a merely large product — neither is recoverable in
    // the WASM playground.
    // CS5: cap sourced from the injectable per-interpreter `RuntimeLimits`
    // (folded), so tests can fire this guard with a tiny limit; same behavior
    // and message as before.
    let max_materialized = interp.runtime_limits.max_materialized_elements;
    let materialized = match checked_materialized_count(&shape) {
        Some(count) if count <= max_materialized => count,
        _ => {
            // A well-formed shape whose element product exceeds the ceiling (or
            // overflows `usize`) is refused by name before anything is
            // allocated (`resourceLimitExceeded`, `materializedElements`;
            // LANG.MACHINE.LIMITS), its operands put back.
            let observed = super::shape_words::shape_observed_size(&shape_val);
            restore(interp, shape_val, value_val);
            return Err(
                crate::interpreter::ceiling_refusal::materialization_refused(
                    max_materialized,
                    observed,
                ),
            );
        }
    };
    if let Err(e) =
        crate::interpreter::collection_meter::charge_materialization(interp, materialized)
    {
        restore(interp, shape_val, value_val);
        return Err(e);
    }

    // Within the ceiling just checked, so the leaf product cannot overflow.
    let total_size: usize = shape.iter().product();

    // Any leaf fills: a number, a text, a truth or a Symbol (a Vector or a
    // Record has already lifted, and a NIL has already passed through). A
    // rational keeps the dense construction it always had.
    let result = match value_val.as_scalar() {
        Some(fill_value) => {
            let data: Vec<Fraction> = (0..total_size).map(|_| fill_value.clone()).collect();
            build_nested_value(&data, &shape)
        }
        None => super::shape_words::regroup(&vec![value_val; total_size], &shape),
    };

    interp.stack.push(result);
    Ok(())
}
