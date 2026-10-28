//! `JSON-ENCODE` — write values and Records as JSON text without rounding
//! (LANG.VALUES.DISJOINT, LANG.RECORDS.STRUCTURE, LANG.VALUES.EXACT).
//!
//! The inverse of `JSON-DECODE` on everything JSON can carry: a Record with
//! String keys is an object in key order, a Vector an array, a String a
//! string, a Boolean a literal, a NIL `null`. A number is written as a JSON
//! number only when JSON can spell it exactly — a rational whose reduced
//! denominator is `2^a·5^b` has a finite decimal — and every other rational
//! is written as its Ajisai lexeme inside a string, so `1/3` leaves as
//! `"1/3"` and not as a digit string that is a different number. Without
//! that rule the encoder would be the language's one silent rounding.
//!
//! A value with no JSON image — a Symbol, an irrational, a Record keyed by
//! anything but Strings — projects `domainMiss`: the operand is well formed,
//! the codomain simply does not contain it.

use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};

use super::ordering_ops::{restore, take_operand};
use crate::error::{NilReason, Result};
use crate::interpreter::cast::cast_conversions::format_fraction_to_string;
use crate::interpreter::collection_meter;
use crate::interpreter::Interpreter;
use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;
use crate::types::{Value, ValueData};

/// How many times `factor` divides `n`, and what is left.
fn strip_factor(mut n: BigInt, factor: u32) -> (u32, BigInt) {
    let factor = BigInt::from(factor);
    let mut count = 0;
    while (&n % &factor).is_zero() {
        n /= &factor;
        count += 1;
    }
    (count, n)
}

/// The finite decimal spelling of `f`, or `None` when it has none — a
/// rational whose denominator has a factor other than 2 and 5, and each of
/// the two points over zero.
fn decimal_spelling(f: &Fraction) -> Option<String> {
    if !f.is_finite() {
        return None;
    }
    let (numerator, denominator) = f.to_bigint_pair();
    let gcd = crate::types::fraction_arithmetic::balanced_bigint_gcd(&numerator, &denominator);
    let (numerator, denominator) = if gcd.is_zero() {
        (numerator, denominator)
    } else {
        (numerator / &gcd, denominator / &gcd)
    };
    let (numerator, denominator) = if denominator.is_negative() {
        (-numerator, -denominator)
    } else {
        (numerator, denominator)
    };
    let (twos, rest) = strip_factor(denominator.clone(), 2);
    let (fives, rest) = strip_factor(rest, 5);
    if !rest.is_one() {
        return None;
    }
    let places = twos.max(fives);
    let scaled = numerator * BigInt::from(10).pow(places) / denominator;
    let magnitude = scaled.abs().to_string();
    let places = places as usize;
    let mut body = if magnitude.len() <= places {
        format!("{}{}", "0".repeat(places + 1 - magnitude.len()), magnitude)
    } else {
        magnitude
    };
    if places > 0 {
        body.insert(body.len() - places, '.');
        let trimmed = body.trim_end_matches('0').trim_end_matches('.');
        body = trimmed.to_string();
    }
    if scaled.is_negative() {
        body.insert(0, '-');
    }
    Some(body)
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_rational(out: &mut String, f: &Fraction) {
    match decimal_spelling(f) {
        Some(decimal) => out.push_str(&decimal),
        None => write_string(out, &format_fraction_to_string(f)),
    }
}

/// Append `value`'s JSON image, or answer `None` when it has none.
fn encode(out: &mut String, value: &Value) -> Option<()> {
    match &value.data {
        ValueData::Truths(t) => return encode(out, &t.to_vector()),
        ValueData::Nil(_) => out.push_str("null"),
        ValueData::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        ValueData::Text(s) => write_string(out, s),
        ValueData::Scalar(f) => write_rational(out, f),
        ValueData::ExactScalar(exact) => match &**exact {
            ExactReal::Rational(f) => write_rational(out, f),
            // An irrational has no JSON image.
            ExactReal::Algebraic(_) => return None,
        },
        ValueData::Symbol(_) => return None,
        ValueData::Vector(_) | ValueData::Tensor { .. } => {
            out.push('[');
            for i in 0..value.len() {
                if i > 0 {
                    out.push(',');
                }
                let child = value.child(i).expect("i < len has a child");
                encode(out, &child)?;
            }
            out.push(']');
        }
        ValueData::Record(record) => {
            out.push('{');
            for (i, (key, member)) in record.entries().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key.as_text()?);
                out.push(':');
                encode(out, member)?;
            }
            out.push('}');
        }
    }
    Some(())
}

/// `JSON-ENCODE ( [ value ] -> [ text ] )`: projects `domainMiss`.
pub(crate) fn op_json_encode(interp: &mut Interpreter) -> Result<()> {
    let operand = take_operand(interp)?;
    // One walk over the value, charged before it runs.
    if let Err(e) = collection_meter::charge_walk_of(interp, &operand) {
        restore(interp, operand);
        return Err(e);
    }
    let mut out = String::new();
    match encode(&mut out, &operand) {
        Some(()) => interp.stack.push(Value::from_string(&out)),
        None => interp
            .stack
            .push(Value::nil_with_reason(NilReason::DomainMiss)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spelling(n: i64, d: i64) -> Option<String> {
        decimal_spelling(&Fraction::new(BigInt::from(n), BigInt::from(d)))
    }

    #[test]
    fn finite_decimals_are_spelled_exactly_and_others_not_at_all() {
        assert_eq!(spelling(1, 4).as_deref(), Some("0.25"));
        assert_eq!(spelling(-1, 8).as_deref(), Some("-0.125"));
        assert_eq!(spelling(3, 1).as_deref(), Some("3"));
        assert_eq!(spelling(0, 1).as_deref(), Some("0"));
        assert_eq!(spelling(2, 4).as_deref(), Some("0.5"));
        assert_eq!(spelling(1, 10).as_deref(), Some("0.1"));
        assert_eq!(spelling(1234, 100).as_deref(), Some("12.34"));
        assert_eq!(spelling(1, 3), None);
        assert_eq!(spelling(7, 6), None);
    }

    #[test]
    fn strings_escape_what_json_requires() {
        let mut out = String::new();
        write_string(&mut out, "a\"b\\c\n\u{1}é");
        assert_eq!(out, r#""a\"b\\c\n\u0001é""#);
    }
}
