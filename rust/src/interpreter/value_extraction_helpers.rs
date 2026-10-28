use crate::error::{AjisaiError, Result};
use crate::interpreter::Interpreter;
use crate::types::exact::ExactReal;
use crate::types::{Value, ValueData};
use num_bigint::{BigInt, Sign};
use num_traits::ToPrimitive;

/// The text a test reads off a value. The text Words read a String's own
/// `as_text` instead: this widens every character to a `char` on the way, four
/// bytes each, which on a long String was most of what a Word allocated.
#[cfg(test)]
pub(crate) fn value_as_string(val: &Value) -> Option<String> {
    fn collect_chars(val: &Value) -> Vec<char> {
        match &val.data {
            ValueData::Truths(t) => collect_chars(&t.to_vector()),
            ValueData::Text(s) => s.chars().collect(),
            ValueData::Nil(_) => vec![],
            ValueData::Scalar(f) => f
                .to_i64()
                .and_then(|n| {
                    if (0..=0x10FFFF).contains(&n) {
                        char::from_u32(n as u32)
                    } else {
                        None
                    }
                })
                .map(|c| vec![c])
                .unwrap_or_default(),
            ValueData::Vector(children) => children.iter().flat_map(collect_chars).collect(),
            ValueData::Tensor { data, .. } => data
                .iter()
                .filter_map(|f| {
                    f.to_i64().and_then(|n| {
                        if (0..=0x10FFFF).contains(&n) {
                            char::from_u32(n as u32)
                        } else {
                            None
                        }
                    })
                })
                .collect(),
            ValueData::ExactScalar(_) => vec![],
            ValueData::Boolean(_) | ValueData::Symbol(_) | ValueData::Record(_) => vec![],
        }
    }

    let chars = collect_chars(val);
    if chars.is_empty() {
        None
    } else {
        Some(chars.into_iter().collect())
    }
}

/// An operand that is not an integer, described for the message the caller
/// raises. Not an `AjisaiError`: GET, TAKE, PUT, COLLECT and RANGE all raise
/// `invalidInteger` for it, each with a message naming what the integer was
/// for (an index, a count, a bound).
#[derive(Debug, Clone)]
pub(crate) struct NotAnInteger {
    pub got: String,
}

impl NotAnInteger {
    fn of(value: &Value) -> Self {
        NotAnInteger {
            got: crate::types::display::describe_operand(value),
        }
    }
}

fn extract_integer_bigint(value: &Value) -> std::result::Result<BigInt, NotAnInteger> {
    match &value.data {
        ValueData::Scalar(f) if f.is_integer() => Ok(f.numerator()),
        // No one-element Vector stands for its element here: `[ 2 ]` is a
        // Vector (LANG.VALUES.DISJOINT), and a Vector where an index or a
        // count is read lifts the Word over it (LANG.COLLECTIONS.LIFT).
        _ => Err(NotAnInteger::of(value)),
    }
}

/// An integer operand read as a position or count in a Vector: any integer,
/// with one outside `i64` clamped to the nearer end of it. No Vector is that
/// long, so a clamped position is past the end exactly when the integer was,
/// and GET, TAKE, DROP and PUT project it as `indexOutOfBounds` rather than
/// refusing an integer as `invalidInteger` for its size.
pub(crate) fn extract_position_from_value(value: &Value) -> std::result::Result<i64, NotAnInteger> {
    let integer = extract_integer_bigint(value)?;
    Ok(integer
        .to_i64()
        .unwrap_or(if integer.sign() == Sign::Minus {
            i64::MIN
        } else {
            i64::MAX
        }))
}

pub(crate) fn extract_bigint_from_value(
    value: &Value,
) -> std::result::Result<BigInt, NotAnInteger> {
    extract_integer_bigint(value)
}

/// The Word name a value denotes. A name is a String
/// (`{ ... } 'INC' DEF`), so only the String domain supplies one.
///
/// This used to flatten the value to a list of fractions and decode each as a
/// codepoint, which accepted `[ 73 78 67 ]` as the name `INC` just as readily
/// as `'INC'` — a Vector of numbers naming a Word. Reading the String domain
/// closes that, and `nonText` is the honest error for everything else — DEF
/// and DEL both declare it (`spec/words.json`) for exactly this call.
pub(crate) fn extract_word_name_from_value(value: &Value) -> Result<String> {
    if value.is_nil() {
        return Err(AjisaiError::declared(
            "nonText",
            "expected a name (String), got NIL",
        ));
    }

    match value.as_text() {
        Some(name) => Ok(name.to_uppercase()),
        None => Err(AjisaiError::declared(
            "nonText",
            format!("expected a name (String), got {}", value.domain_name()),
        )),
    }
}

pub(crate) fn normalize_index(index: i64, length: usize) -> Option<usize> {
    // Resolve the bounds check entirely in i64 before narrowing. An in-memory
    // vector length always fits i64, and a previous `index as usize` truncated
    // out-of-range positive indices on 32-bit wasm (e.g. 2^32 + 1 wrapping to a
    // valid-looking small index). Keeping `actual` in [0, length) guarantees the
    // final `as usize` is exact on both 32- and 64-bit targets.
    let len_i64 = length as i64;
    let actual = if index < 0 {
        len_i64.checked_add(index)?
    } else {
        index
    };

    if actual >= 0 && actual < len_i64 {
        Some(actual as usize)
    } else {
        None
    }
}

pub(crate) fn extract_operands(interp: &mut Interpreter, count: usize) -> Result<Vec<Value>> {
    if interp.stack.len() < count {
        return Err(AjisaiError::stack_underflow());
    }

    Ok(interp.stack.drain(interp.stack.len() - count..).collect())
}

/// Exact-real view of a numeric operand: a rational `Scalar` lifts to
/// `ExactReal::Rational`; a lazy `ExactScalar` is taken as-is. Non-numeric
/// kinds return `None` — the malformed-use path.
pub(crate) fn exact_real_of(value: &Value) -> Option<ExactReal> {
    match &value.data {
        ValueData::Scalar(f) => Some(ExactReal::from_fraction(f.clone())),
        ValueData::ExactScalar(er) => Some((**er).clone()),
        _ => None,
    }
}

pub(crate) fn nil_passthrough_unary(interp: &mut Interpreter) -> bool {
    let stack_len = interp.stack.len();
    if stack_len == 0 {
        return false;
    }
    if !interp.stack[stack_len - 1].is_nil() {
        return false;
    }
    let inherited = Value::nil_inheriting_absence_from(&interp.stack[stack_len - 1]);
    interp.stack.pop();
    interp.stack.push(inherited);
    true
}

pub(crate) fn nil_passthrough_binary(interp: &mut Interpreter) -> bool {
    let stack_len = interp.stack.len();
    if stack_len < 2 {
        return false;
    }
    let a_nil = interp.stack[stack_len - 2].is_nil();
    let b_nil = interp.stack[stack_len - 1].is_nil();
    if !(a_nil || b_nil) {
        return false;
    }
    let inherited = if a_nil {
        Value::nil_inheriting_absence_from(&interp.stack[stack_len - 2])
    } else {
        Value::nil_inheriting_absence_from(&interp.stack[stack_len - 1])
    };
    interp.stack.pop();
    interp.stack.pop();
    interp.stack.push(inherited);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::fraction::Fraction;
    use num_traits::One;

    #[test]
    fn test_normalize_index_positive() {
        assert_eq!(normalize_index(0, 5), Some(0));
        assert_eq!(normalize_index(4, 5), Some(4));
        assert_eq!(normalize_index(5, 5), None);
    }

    #[test]
    fn test_normalize_index_negative() {
        assert_eq!(normalize_index(-1, 5), Some(4));
        assert_eq!(normalize_index(-5, 5), Some(0));
        assert_eq!(normalize_index(-6, 5), None);
    }

    #[test]
    fn test_extract_position_from_value() {
        let wrapped = Value::from_fraction(Fraction::new(BigInt::from(42), BigInt::one()));
        let result = extract_position_from_value(&wrapped).unwrap();
        assert_eq!(result, 42);
        let huge = Value::from_fraction(Fraction::new(BigInt::from(10).pow(20), BigInt::one()));
        assert_eq!(extract_position_from_value(&huge).unwrap(), i64::MAX);
        let negative =
            Value::from_fraction(Fraction::new(-BigInt::from(10).pow(20), BigInt::one()));
        assert_eq!(extract_position_from_value(&negative).unwrap(), i64::MIN);
    }
}

#[cfg(kani)]
#[path = "value_extraction_helpers_proofs.rs"]
mod proofs;
