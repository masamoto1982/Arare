//! `STR` and `NUM` (LANG.VALUES.DISJOINT), and the value predicates and
//! renderings the cast and text Words share.

use crate::error::{AjisaiError, NilReason, Result};
use crate::interpreter::Interpreter;
use crate::types::fraction::Fraction;
use crate::types::{Value, ValueData};

/// Whether a value is a String (LANG.VALUES.DISJOINT).
///
/// String is a domain of its own, so the question is answered by the tag.
pub(crate) fn is_string_value(val: &Value) -> bool {
    val.is_text()
}

pub(crate) fn is_number_value(val: &Value) -> bool {
    val.is_scalar()
}

pub(crate) fn apply_unary_cast(
    interp: &mut Interpreter,
    convert: fn(&Value) -> Result<Value>,
) -> Result<()> {
    let value: Value = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    match convert(&value) {
        Ok(result) => {
            interp.stack.push(result);
            Ok(())
        }
        Err(error) => {
            interp.stack.push(value);
            Err(error)
        }
    }
}

pub(crate) fn format_fraction_to_string(f: &Fraction) -> String {
    if f.is_integer() {
        format!("{}", f.numerator())
    } else {
        format!("{}/{}", f.numerator(), f.denominator())
    }
}

pub(crate) fn format_value_to_string_repr(value: &Value) -> String {
    if value.is_nil() {
        return "NIL".to_string();
    }

    if let Some(text) = value.as_text() {
        return text.to_string();
    }

    if is_number_value(value) {
        if let Some(f) = value.as_scalar() {
            return format_fraction_to_string(f);
        }
    }

    fn collect_fractions(val: &Value) -> Vec<String> {
        match &val.data {
            ValueData::Truths(t) => collect_fractions(&t.to_vector()),
            ValueData::Nil(_) => vec!["NIL".to_string()],
            ValueData::Boolean(b) => vec![if *b { "TRUE" } else { "FALSE" }.to_string()],
            ValueData::Scalar(f) => vec![format_fraction_to_string(f)],
            ValueData::ExactScalar(er) => {
                use num_bigint::BigInt;
                match er.best_rational_approximation(&BigInt::from(1_000_000u64)) {
                    Some(approx) => vec![format_fraction_to_string(&approx)],
                    None => vec!["NIL".to_string()],
                }
            }
            ValueData::Vector(children) => children.iter().flat_map(collect_fractions).collect(),
            // Through the lane, not its `Fraction`: `format_fraction_to_string`
            // would render an absent lane as the unreadable number its pair
            // spells. `[ 1 2 ] [ 1 0 ] DIV STR` is `'1 NIL'`.
            ValueData::Tensor { data, .. } => (0..data.len())
                .flat_map(|lane| collect_fractions(&Value::from_dense_lane(data, lane)))
                .collect(),
            ValueData::Text(s) => vec![s.to_string()],
            ValueData::Symbol(name) => vec![name.to_string()],
            // A Record casts as its display form, whole: it is not a sequence
            // of lanes to join.
            ValueData::Record(_) => vec![val.to_string()],
        }
    }

    collect_fractions(value).join(" ")
}

/// Whether any number inside `value` has no lexeme in the sealed numeric
/// grammar — an irrational the source language cannot spell.
///
/// The grammar writes integers and ratios and nothing else, so `2 SQRT` has no
/// faithful text. `STR` used to answer with a continued-fraction convergent
/// anyway: `'665857/470832'`, a *different number*, with no error and no NIL.
/// That is precisely the silent-wrong-answer failure LANG.FAILURE.TRICHOTOMY
/// rules out, and it was worst where it was least visible — `STR` is how a
/// runtime value becomes a `number` token for `REFLECT`, so building a partial
/// application over an exact irrational quietly replaced it with a rational
/// look-alike, in the one value class exactness is the whole point of.
fn has_no_exact_lexeme(value: &Value) -> bool {
    match &value.data {
        ValueData::Truths(t) => has_no_exact_lexeme(&t.to_vector()),
        ValueData::ExactScalar(exact) => exact.to_fraction().is_none(),
        ValueData::Vector(children) => children.iter().any(has_no_exact_lexeme),
        ValueData::Record(record) => record.values().iter().any(has_no_exact_lexeme),
        ValueData::Boolean(_)
        | ValueData::Text(_)
        | ValueData::Scalar(_)
        | ValueData::Tensor { .. }
        | ValueData::Nil(_)
        | ValueData::Symbol(_) => false,
    }
}

fn convert_value_to_string(val: &Value) -> Result<Value> {
    if val.is_nil() {
        return Ok(Value::nil_inheriting_absence_from(val));
    }

    if val.is_text() {
        return Ok(val.clone());
    }

    if is_number_value(val) {
        if let Some(f) = val.as_scalar() {
            let string_repr = format_fraction_to_string(f);
            return Ok(Value::from_string(&string_repr));
        }
    }

    // No lexeme exists, so there is no text to answer with. The value is
    // well-formed and outside what text can spell, so `STR` projects
    // `domainMiss` — the reason `JSON-ENCODE` projects for a value with no JSON
    // image; `invalidEncoding` belongs to the reading direction, text that
    // spells nothing. A program that wants a rational stand-in writes one out —
    // `10000 MUL ROUND 10000 DIV` — where the denominator is the caller's
    // choice and the approximation is visible in the source.
    if has_no_exact_lexeme(val) {
        return Ok(Value::nil_with_reason(NilReason::DomainMiss));
    }

    let string_repr = format_value_to_string_repr(val);
    Ok(Value::from_string(&string_repr))
}

pub fn op_str(interp: &mut Interpreter) -> Result<()> {
    apply_unary_cast(interp, convert_value_to_string)
}

/// `NUM` accepts exactly the lexemes a source literal accepts (LANG.SOURCE.TEXT):
/// the sealed numeric grammar is the one definition of what text denotes a
/// number, so `'.5' NUM` fails for the same reason `.5` is not a literal.
///
/// The gate is deliberately the tokenizer's own validator rather than
/// `Fraction::from_str`, which is the *internal* value parser and stays more
/// permissive — it accepts truncated decimals and underscore digit separators
/// (`1_000`), neither of which is Ajisai source. Nothing outside the program can
/// reach this Word, because the vocabulary has no input Word: every operand was
/// written as a literal or built by the program, so leniency here would only
/// create a second numeric language for a program to trip over.
fn convert_value_to_number(val: &Value) -> Result<Value> {
    if let Some(s) = val.as_text() {
        return Ok(number_from_text(
            s,
            crate::tokenizer::is_number_token_lexeme(s),
        ));
    }

    // A number is not Text: NUM parses, it does not pass through. Accepting
    // numbers here would make NUM's `nonText` contract false for exactly the
    // operand kind a caller is most likely to pass by mistake.
    Err(AjisaiError::declared(
        "nonText",
        format!("expected a String, got {}", val.domain_name()),
    ))
}

/// What `NUM` answers for `text`, given whether it is a numeric lexeme: the
/// number it denotes, or an `invalidEncoding` NIL.
fn number_from_text(text: &str, is_lexeme: bool) -> Value {
    match is_lexeme.then(|| Fraction::from_str(text).ok()).flatten() {
        Some(fraction) => Value::from_fraction(fraction),
        None => Value::nil_with_reason(NilReason::InvalidEncoding),
    }
}

pub fn op_num(interp: &mut Interpreter) -> Result<()> {
    // Anything but a String — or nothing — is `convert_value_to_number`'s
    // refusal, with the operand restored.
    let Some(text) = interp.stack.last().and_then(Value::as_text) else {
        return apply_unary_cast(interp, convert_value_to_number);
    };
    // The text is read where it is held, and whether it is a numeric lexeme
    // is decided once for the ceiling and the parse together.
    let is_lexeme = crate::tokenizer::is_number_token_lexeme(text);
    // The numeric-literal ceiling holds here as it does in source: the text
    // is read by the same grammar, and `'1e99999999' NUM` would otherwise
    // spend minutes building a hundred-million-digit integer from eleven
    // characters. Refused by name, as the tokenizer refuses the same lexeme
    // in source (LANG.MACHINE.LIMITS); the operand stays where it was.
    let limit = interp.runtime_limits.max_numeric_literal_digits;
    if is_lexeme {
        let digits = crate::tokenizer::denoted_digit_count(text);
        if digits > limit as u64 {
            return Err(
                crate::interpreter::ceiling_refusal::numeric_literal_refused(limit, digits),
            );
        }
    }
    let number = number_from_text(text, is_lexeme);
    interp.stack.pop();
    interp.stack.push(number);
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Test suite for `crate::interpreter::cast`.

    use super::{format_value_to_string_repr, is_number_value, is_string_value, op_num, op_str};
    use crate::interpreter::value_extraction_helpers::value_as_string;
    use crate::interpreter::Interpreter;
    use crate::types::fraction::Fraction;
    use crate::types::Value;
    use num_bigint::BigInt;
    use num_traits::One;

    #[test]
    fn test_format_value_to_string_repr() {
        let num = Value::from_fraction(Fraction::new(BigInt::from(42), BigInt::one()));
        assert_eq!(format_value_to_string_repr(&num), "42");

        let bool_val = Value::from_bool(true);
        assert_eq!(format_value_to_string_repr(&bool_val), "TRUE");

        let nil = Value::nil();
        assert_eq!(format_value_to_string_repr(&nil), "NIL");
    }

    #[test]
    fn test_str_conversion() {
        let mut interp = Interpreter::new();

        interp.stack.push(Value::from_fraction(Fraction::new(
            BigInt::from(42),
            BigInt::one(),
        )));
        op_str(&mut interp).unwrap();

        if let Some(val) = interp.stack.last() {
            assert!(is_string_value(val));
            let s = value_as_string(val).unwrap();
            assert_eq!(s, "42");
        }
    }

    #[test]
    fn test_num_conversion() {
        let mut interp = Interpreter::new();

        interp.stack.push(Value::from_string("42"));
        op_num(&mut interp).unwrap();

        if let Some(val) = interp.stack.last() {
            assert!(is_number_value(val));
            if let Some(f) = val.as_scalar() {
                assert_eq!(f.numerator(), BigInt::from(42));
            }
        }

        interp.stack.clear();
        interp.stack.push(Value::from_string("1/3"));
        op_num(&mut interp).unwrap();

        if let Some(val) = interp.stack.last() {
            assert!(is_number_value(val));
            if let Some(f) = val.as_scalar() {
                assert_eq!(f.numerator(), BigInt::from(1));
                assert_eq!(f.denominator(), BigInt::from(3));
            }
        }

        interp.stack.clear();
        interp.stack.push(Value::from_string("ABC"));
        let result = op_num(&mut interp);
        assert!(result.is_ok());
        if let Some(val) = interp.stack.last() {
            assert!(val.is_nil());
        }

        interp.stack.clear();
        interp.stack.push(Value::from_fraction(Fraction::new(
            BigInt::from(123),
            BigInt::one(),
        )));
        let result = op_num(&mut interp);
        // NUM parses Text and nothing else: a number operand is `nonText`,
        // as the contract declares, not a passthrough.
        assert!(result.is_err());

        interp.stack.clear();
        interp.stack.push(Value::from_bool(true));
        let result = op_num(&mut interp);
        // NUM no longer accepts a Boolean: a truth value is not a number
        // (finding B2). TRUE is distinct from the scalar 1.
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_num_str_roundtrip() {
        let mut interp = Interpreter::new();

        interp.execute("'123' NUM STR").await.unwrap();
        if let Some(val) = interp.stack.last() {
            assert!(is_string_value(val));
            let s = value_as_string(val).unwrap();
            assert_eq!(s, "123");
        }

        interp.stack.clear();
        interp.execute("'1/3' NUM STR").await.unwrap();
        if let Some(val) = interp.stack.last() {
            assert!(is_string_value(val));
            let s = value_as_string(val).unwrap();
            assert_eq!(s, "1/3");
        }
    }

    #[tokio::test]
    async fn test_str_num_parse_fail() {
        let mut interp = Interpreter::new();

        interp.execute("'ABC' NUM").await.unwrap();
        assert_eq!(interp.stack.len(), 1);
        if let Some(val) = interp.stack.last() {
            assert!(val.is_nil());
        }
    }
    #[tokio::test]
    async fn test_str_boolean() {
        let mut interp = Interpreter::new();

        interp.execute("TRUE STR").await.unwrap();
        if let Some(val) = interp.stack.last() {
            assert!(is_string_value(val));
            let s = value_as_string(val).unwrap();
            assert_eq!(s, "TRUE");
        }

        interp.stack.clear();
        interp.execute("FALSE STR").await.unwrap();
        if let Some(val) = interp.stack.last() {
            assert!(is_string_value(val));
            let s = value_as_string(val).unwrap();
            assert_eq!(s, "FALSE");
        }
    }

    #[tokio::test]
    async fn test_str_nil() {
        let mut interp = Interpreter::new();

        interp.execute("NIL STR").await.unwrap();
        if let Some(val) = interp.stack.last() {
            assert!(val.is_nil(), "NIL STR should return NIL, not a string");
        }
    }
}
