//! The numeric Words beyond the four arithmetic operators: `MIN`, `MAX`,
//! `SQRT`, and the three that close the number concept, `POW`, `GCD`,
//! `RATIO` (LANG.VALUES.EXACT).

use num_bigint::BigInt;
use num_traits::Signed;

use crate::error::{AjisaiError, NilReason, ResourceLimit, Result};
use crate::interpreter::arithmetic_meter::measure_operand;
use crate::interpreter::exact_work::{charge_comparison, power_work};
use crate::interpreter::record_ops;
use crate::interpreter::runtime_limits::{broadcast_numeric_work, RuntimeLimits};
use crate::interpreter::value_extraction_helpers::{
    exact_real_of, extract_operands, nil_passthrough_binary,
};
use crate::interpreter::Interpreter;
use crate::types::exact::{ExactReal, PowOutcome, PowPlan};
use crate::types::fraction::Fraction;
use crate::types::Value;

/// `three_way_compare` for MIN/MAX, raising `nonNumeric` like every other
/// Word that asks for an order (LANG.VALUES.ORDER); `None` for an operand
/// with no order (a point over zero), which the Word projects.
fn compare_for_numeric(a: &Value, b: &Value) -> Result<Option<std::cmp::Ordering>> {
    crate::interpreter::comparison::three_way_compare(a, b).map_err(|e| {
        AjisaiError::declared(
            "nonNumeric",
            format!("expected two Scalars or two Strings, got {}", e.got),
        )
    })
}

/// Apply a unary numeric Word across the shapes LANG.COLLECTIONS.LIFT allows.
///
/// The clause makes element-wise application the rule for an arithmetic Word
/// given a vector. The unary Words once took a scalar only, so `[ 4 9 ] SQRT`
/// was an ERROR while `[ 4 9 ] 1 MUL` lifted happily — the same clause read
/// two ways depending on arity. A NIL lane passes through, as it does for the
/// scalar law.
pub(crate) fn lift_unary_numeric(
    value: &Value,
    scalar_op: &dyn Fn(&Value) -> Result<Value>,
) -> Result<Value> {
    match value.as_vector_view() {
        Some(items) => {
            let lanes = items
                .iter()
                .map(|item| lift_unary_numeric(item, scalar_op))
                .collect::<Result<Vec<_>>>()?;
            // Promoted back to a dense Tensor wherever the lanes fit one.
            Ok(Value::from_vector_promoted(lanes))
        }
        None if value.is_nil() => Ok(value.clone()),
        None => scalar_op(value),
    }
}

/// Apply a binary numeric Word across the shapes LANG.COLLECTIONS.LIFT allows.
///
/// The shape rules are the ones the arithmetic broadcast already uses: a
/// scalar pairs with every element of a vector, and two vectors pair by
/// shape, aligned at the innermost axis with a length-1 axis reused. `MIN` and `MAX`
/// used to take scalars only, so `[ -1 2 -3 ] 0 MAX` — a rectifier, and the
/// most ordinary thing anyone writes with `MAX` — was an ERROR while
/// `[ -1 2 -3 ] 0 ADD` lifted happily. Same clause, same family, two answers.
/// A NIL lane passes through, as it does for the scalar law.
pub(crate) fn lift_binary_numeric(
    a: &Value,
    b: &Value,
    leaf_op: &dyn Fn(&Value, &Value) -> Result<Value>,
) -> Result<Value> {
    use crate::interpreter::broadcast_tree::broadcast_tree;

    broadcast_tree(a, b, &|x, y| {
        if x.is_nil() {
            return Ok(x.clone());
        }
        if y.is_nil() {
            return Ok(y.clone());
        }
        leaf_op(x, y)
    })
}

/// `MIN` / `MAX` select one of two operands by the order relation
/// (LANG.VALUES.ORDER). They accept the full numeric domain, algebraic
/// operands included, and two Strings, and decide the order through the same
/// comparison as the relations; an operand with no order (`0/0`) projects `domainMiss`,
/// as `LT` does. The selected operand is returned unchanged (preserving its
/// exact representation). NIL-passthrough. Element-wise over vectors, by
/// [`lift_binary_numeric`].
fn apply_selecting<F>(interp: &mut Interpreter, pick_left: F) -> Result<()>
where
    // Given the order of `a` (left) vs `b` (right), return true to keep `a`.
    F: Fn(std::cmp::Ordering) -> bool,
{
    if nil_passthrough_binary(interp) {
        return Ok(());
    }
    let operands = extract_operands(interp, 2)?;
    if let Err(e) = charge_comparison(interp, &operands[0], &operands[1]) {
        interp.stack.extend(operands);
        return Err(e);
    }
    let select = |a: &Value, b: &Value| -> Result<Value> {
        let Some(ord) = compare_for_numeric(a, b)? else {
            return Ok(crate::interpreter::comparison::unordered_projection());
        };
        Ok(if pick_left(ord) { a.clone() } else { b.clone() })
    };
    match lift_binary_numeric(&operands[0], &operands[1], &select) {
        Ok(result) => {
            interp.stack.push(result);
            Ok(())
        }
        Err(e) => {
            interp.stack.extend(operands);
            Err(e)
        }
    }
}

pub(crate) fn op_min(interp: &mut Interpreter) -> Result<()> {
    if record_ops::lift_binary(interp, &op_min)? {
        return Ok(());
    }
    // Keep the left operand when it is less-or-equal to the right.
    apply_selecting(interp, |ord| ord != std::cmp::Ordering::Greater)
}

pub(crate) fn op_max(interp: &mut Interpreter) -> Result<()> {
    if record_ops::lift_binary(interp, &op_max)? {
        return Ok(());
    }
    // Keep the left operand when it is greater-or-equal to the right.
    apply_selecting(interp, |ord| ord != std::cmp::Ordering::Less)
}

/// `SQRT`: the exact square root of a non-negative rational, and the only Word
/// that leaves the rationals (LANG.VALUES.EXACT). The result is carried in the
/// multiquadratic normal form, so it compares and decides with no rounding.
/// `1/0` and `0/0` are their own roots.
///
/// A negative radicand is a well-formed domain miss: the
/// multiquadratic field is not closed under it, so the operation projects to
/// NIL rather than raising (LANG.FAILURE.PROJECT). It is recoverable — a
/// different input resolves it.
///
/// Element-wise over a vector, by [`lift_unary_numeric`]: a per-element
/// standard deviation is `variances SQRT`, not a `MAP` around a block.
pub(crate) fn op_sqrt(interp: &mut Interpreter) -> Result<()> {
    if record_ops::lift_unary(interp, &op_sqrt)? {
        return Ok(());
    }
    let value = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    let budget = crate::interpreter::arithmetic_meter::RadicandBudget::of(interp);

    let lifted = lift_unary_numeric(&value, &|lane| sqrt_scalar(lane, &budget));
    match budget.settle(interp).and(lifted) {
        Ok(result) => {
            interp.stack.push(result);
            Ok(())
        }
        Err(e) => {
            interp.stack.push(value);
            Err(e)
        }
    }
}

/// The scalar law of `SQRT`, lifted by [`lift_unary_numeric`].
fn sqrt_scalar(
    value: &Value,
    budget: &crate::interpreter::arithmetic_meter::RadicandBudget,
) -> Result<Value> {
    let Some(f) = value.as_scalar() else {
        // `nonNumeric` is the same declared condition `DIV` uses for
        // an operand outside the numeric domain; SQRT did not declare it
        // before this fix even though the failure is the identical shape.
        return Err(AjisaiError::declared(
            "nonNumeric",
            format!(
                "expected a rational Scalar, got {}",
                crate::types::display::describe_operand(value)
            ),
        ));
    };
    // `from_exact_real` collapses a rational result back to Scalar.
    Ok(match budget.sqrt(f.clone())? {
        Some(er) => Value::from_exact_real(er),
        None => Value::nil_with_reason(NilReason::DomainMiss),
    })
}

// `POW`, `GCD`, `RATIO` — the Words that close the number concept
// (LANG.VALUES.EXACT).
//
// `POW` is the kernel's `ExactReal::pow` lifted like every binary
// arithmetic Word. `GCD` exposes the reduction the machine already performs
// on every rational, and `RATIO` reads a rational's two parts back as a
// Vector, so that arithmetic lifts over the answer. Both refuse what is not
// a rational integer or rational: an irrational projects `domainMiss`.
fn non_numeric(operands: &[&Value]) -> AjisaiError {
    let got = operands
        .iter()
        .find(|operand| exact_real_of(operand).is_none())
        .map_or("NIL", |operand| operand.domain_name());
    AjisaiError::declared("nonNumeric", format!("expected a Scalar, got {got}"))
}

fn nil(reason: NilReason) -> Value {
    Value::nil_with_reason(reason)
}

/// The scalar law of `POW`, lifted by [`lift_binary_numeric`].
///
/// A power is sized and priced before it is taken. One past `bigintBits` or
/// `algebraicTerms` is refused by that ceiling's name before it is computed
/// (`resourceLimitExceeded`, LANG.MACHINE.LIMITS), exactly as the same
/// width reached by repeated `MUL` is refused after; and the products that
/// take one are charged to `budget` as `MUL` would charge them.
fn pow_scalar(
    x: &Value,
    y: &Value,
    budget: &crate::interpreter::arithmetic_meter::RadicandBudget,
    limits: &RuntimeLimits,
) -> Result<Value> {
    let (Some(base), Some(exponent)) = (exact_real_of(x), exact_real_of(y)) else {
        return Err(non_numeric(&[x, y]));
    };
    let mut left = budget.take();
    let plan = base.plan_pow_within(&exponent, &mut left);
    budget.spent(
        left,
        matches!(plan, PowPlan::Answered(PowOutcome::WorkExhausted)),
    );
    let outcome = match plan {
        PowPlan::Answered(outcome) => outcome,
        PowPlan::Integer { base, exponent } => {
            let size = base.power_size(&exponent);
            limits.check_algebraic_size(
                usize::try_from(size.terms).unwrap_or(usize::MAX),
                size.bits,
            )?;
            if !budget.charge(power_work(&base, &exponent)) {
                PowOutcome::WorkExhausted
            } else {
                base.power_by_integer(&exponent)
            }
        }
    };
    Ok(match outcome {
        PowOutcome::WorkExhausted => return Err(budget.exhausted_error()),
        PowOutcome::Value(er) => Value::from_exact_real(er),
        PowOutcome::DomainMiss => nil(NilReason::DomainMiss),
        // Sized against `bigintBits` above, which is never wider than the
        // bound `power_by_integer` keeps for itself.
        PowOutcome::TooWide => {
            return Err(AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::BigintBits,
                limit: limits.max_bigint_bits,
                observed: None,
                progress: None,
            })
        }
    })
}

/// The integer a rational integer scalar holds; the projection otherwise — a
/// rational that is not an integer, an irrational, and each of the three
/// points over zero, none of which is an integer.
fn integer_of(value: &Value) -> std::result::Result<BigInt, Value> {
    match exact_real_of(value) {
        Some(ExactReal::Rational(q)) if q.is_integer() => Ok(q.numerator()),
        Some(_) => Err(nil(NilReason::DomainMiss)),
        None => Err(non_numeric_value()),
    }
}

/// A marker for "not a number at all", told apart from a projection by
/// the caller.
fn non_numeric_value() -> Value {
    Value::from_symbol("__nonNumeric")
}

pub(crate) fn gcd_scalar(a: &Value, b: &Value) -> Result<Value> {
    if exact_real_of(a).is_none() || exact_real_of(b).is_none() {
        return Err(non_numeric(&[a, b]));
    }
    match (integer_of(a), integer_of(b)) {
        (Ok(x), Ok(y)) => Ok(Value::from_fraction(Fraction::new(
            crate::types::fraction_arithmetic::balanced_bigint_gcd(&x, &y),
            BigInt::from(1),
        ))),
        (Err(projection), _) | (_, Err(projection)) => Ok(projection),
    }
}

fn ratio_scalar(value: &Value) -> Result<Value> {
    Ok(match exact_real_of(value) {
        Some(ExactReal::Rational(q)) => {
            let (n, d) = q.to_bigint_pair();
            let (n, d) = if d.is_negative() { (-n, -d) } else { (n, d) };
            Value::from_vector(vec![
                Value::from_fraction(Fraction::new(n, BigInt::from(1))),
                Value::from_fraction(Fraction::new(d, BigInt::from(1))),
            ])
        }
        Some(ExactReal::Algebraic(_)) => nil(NilReason::DomainMiss),
        None => return Err(non_numeric(&[value])),
    })
}

fn binary(interp: &mut Interpreter, leaf: &dyn Fn(&Value, &Value) -> Result<Value>) -> Result<()> {
    let operands = extract_operands(interp, 2)?;
    match lift_binary_numeric(&operands[0], &operands[1], leaf) {
        Ok(result) => {
            interp.stack.push(result);
            Ok(())
        }
        Err(e) => {
            interp.stack.extend(operands);
            Err(e)
        }
    }
}

pub(crate) fn op_pow(interp: &mut Interpreter) -> Result<()> {
    if record_ops::lift_binary(interp, &op_pow)? {
        return Ok(());
    }
    let budget = crate::interpreter::arithmetic_meter::RadicandBudget::of(interp);
    let limits = *interp.runtime_limits();
    let operands = extract_operands(interp, 2)?;
    let lifted = lift_binary_numeric(&operands[0], &operands[1], &|x, y| {
        pow_scalar(x, y, &budget, &limits)
    });
    // What the estimate let through is held to the ceilings as every other
    // arithmetic result is.
    let checked = budget.settle(interp).and(lifted).and_then(|result| {
        crate::interpreter::arithmetic_meter::check_result_size(interp, &result)?;
        Ok(result)
    });
    match checked {
        Ok(result) => {
            interp.stack.push(result);
            Ok(())
        }
        Err(e) => {
            interp.stack.extend(operands);
            Err(e)
        }
    }
}

/// `GCD`, charged before it runs at what `ADD` costs on the same operands:
/// Euclid on two integers walks their limbs at least as often as a rational
/// sum's cross-multiplication does.
pub(crate) fn op_gcd(interp: &mut Interpreter) -> Result<()> {
    if record_ops::lift_binary(interp, &op_gcd)? {
        return Ok(());
    }
    let len = interp.stack.len();
    if len >= 2 {
        let slots = interp.stack.as_slice();
        let (left, right) = (
            measure_operand(&slots[len - 2]),
            measure_operand(&slots[len - 1]),
        );
        interp.charge_numeric_work(broadcast_numeric_work(left, right, 1))?;
    }
    binary(interp, &gcd_scalar)
}

pub(crate) fn op_ratio(interp: &mut Interpreter) -> Result<()> {
    if record_ops::lift_unary(interp, &op_ratio)? {
        return Ok(());
    }
    let operands = extract_operands(interp, 1)?;
    match lift_unary_numeric(&operands[0], &ratio_scalar) {
        Ok(result) => {
            interp.stack.push(result);
            Ok(())
        }
        Err(e) => {
            interp.stack.extend(operands);
            Err(e)
        }
    }
}
