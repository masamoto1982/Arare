//! `FORMAT` — render an exact scalar as decimal text at a stated precision
//! (LANG.VALUES.EXACT, LANG.VALUES.DISJOINT).
//!
//! Arithmetic never rounds and `STR` refuses a number with no exact lexeme,
//! so the language had no place a program could ask for `1/3` to three
//! places without first rounding the *value* (`1000 MUL ROUND 1000 DIV`) and then spelling
//! the rounded number. `FORMAT` is that place, and it is the only one: what
//! leaves it is text, so the rounded quantity never re-enters arithmetic as
//! if it were exact. The rule is fixed — a tie rounds away from zero, the
//! one rule `ROUND` already applies — because a choice of
//! rounding mode is a family of Words, and the language keeps one rule.
//!
//! The decision is exact at every tier. A rational scales and rounds
//! outright. An algebraic irrational compares its scaled fraction part against
//! one half through the field's total order, which never ties. Every digit
//! is therefore decided; none is guessed.

use num_bigint::BigInt;
use num_traits::{One, Signed};
use std::cmp::Ordering;

use crate::error::{AjisaiError, NilReason, Result};
use crate::interpreter::runtime_limits::{binary_numeric_work, exact_work_bits};
use crate::interpreter::value_extraction_helpers::{exact_real_of, extract_operands};
use crate::interpreter::Interpreter;
use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;
use crate::types::Value;

/// The most digits one `FORMAT` may ask for. Every digit is a decimal place
/// of big-integer work, and the meter charges each one, so the cap only
/// stops a single request from allocating a number the meter would refuse
/// anyway.
const MAX_DIGITS: u64 = 1 << 16;

fn digit_count(value: &Value) -> Option<u64> {
    let f = value.as_scalar()?;
    if !f.is_integer() || f.numerator().is_negative() {
        return None;
    }
    let n = f.numerator();
    (n <= BigInt::from(MAX_DIGITS)).then(|| n.to_string().parse().ok())?
}

/// What deciding `digits` places of `x` costs the work meter.
///
/// A rational scales and rounds once: every digit is a decimal place of
/// big-integer work. An algebraic value is scaled, floored and compared with
/// one half, and each of its terms is refined to the full scaled width to do
/// it — a bignum product per term, priced limb×limb as arithmetic prices one
/// (`binary_numeric_work`), plus one more for the comparison. Measured, six
/// square roots at 65,536 places ran 7.3 s natively and were charged 86,065
/// units at one unit a digit; this prices them at the floor rate the budget
/// is derived from.
fn format_work(x: &ExactReal, digits: u64) -> u64 {
    if x.as_rational().is_some() {
        return digits + 1;
    }
    // log2(10) < 10/3: the scaled width, over-estimated slightly.
    let width = (digits.saturating_mul(10) / 3).saturating_add(exact_work_bits(x));
    let terms = x.algebraic_term_count() as u64;
    binary_numeric_work(width, width).saturating_mul(terms + 1)
}

/// `x * 10^digits`, rounded to an integer with a tie away from zero.
fn round_scaled(x: &ExactReal, digits: u64) -> BigInt {
    let scale = Fraction::new(BigInt::from(10).pow(digits as u32), BigInt::one());
    if let Some(f) = x.as_rational() {
        return f.mul(&scale).round().numerator();
    }
    let scaled = x.mul(&ExactReal::from_fraction(scale));
    let floor = scaled.floor();
    let floor_int = floor
        .as_rational()
        .expect("a floor is an integer")
        .numerator();
    let fraction_part = scaled.sub(&floor);
    let half = ExactReal::from_fraction(Fraction::new(BigInt::one(), BigInt::from(2)));
    match fraction_part.cmp_exact(&half) {
        Some(Ordering::Less) => floor_int,
        // A tie cannot occur for an irrational, but the rule is stated all
        // the same: away from zero.
        Some(Ordering::Equal) if floor_int.is_negative() => floor_int,
        _ => floor_int + 1,
    }
}

/// Spell `n / 10^digits` in decimal: a sign only for a non-zero value, at
/// least one digit before the point, exactly `digits` after it, and no
/// point at all when `digits` is zero.
fn spell(n: &BigInt, digits: u64) -> String {
    let magnitude = n.abs().to_string();
    let digits = digits as usize;
    let mut body = if magnitude.len() <= digits {
        let mut padded = "0".repeat(digits + 1 - magnitude.len());
        padded.push_str(&magnitude);
        padded
    } else {
        magnitude
    };
    if digits > 0 {
        body.insert(body.len() - digits, '.');
    }
    if n.is_negative() {
        body.insert(0, '-');
    }
    body
}

/// `FORMAT ( [ x ] [ digits ] -> [ text ] )`.
pub(crate) fn op_format(interp: &mut Interpreter) -> Result<()> {
    let operands = extract_operands(interp, 2)?;
    let Some(x) = exact_real_of(&operands[0]) else {
        let got = operands[0].domain_name();
        interp.stack.extend(operands);
        return Err(AjisaiError::declared(
            "nonNumeric",
            format!("expected a Scalar as the value, got {got}"),
        ));
    };
    let Some(digits) = digit_count(&operands[1]) else {
        interp.stack.extend(operands);
        return Err(AjisaiError::declared(
            "invalidInteger",
            "expected a non-negative integer digit count",
        ));
    };
    if let Err(e) = interp.charge_numeric_work(format_work(&x, digits)) {
        interp.stack.extend(operands);
        return Err(e);
    }
    // No decimal lies at or beyond the two points over zero, so there is
    // no text to render: a well-formed operand outside the operation's
    // domain projects `domainMiss`, as `STR` does for an irrational.
    if !x.is_finite() {
        interp
            .stack
            .push(Value::nil_with_reason(NilReason::DomainMiss));
        return Ok(());
    }
    let n = round_scaled(&x, digits);
    interp.stack.push(Value::from_string(&spell(&n, digits)));
    Ok(())
}
