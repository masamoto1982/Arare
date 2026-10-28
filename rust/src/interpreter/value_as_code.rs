//! Bridges a `Value::Vector`'s elements back into `Vec<Token>` so the
//! existing token-based execution engine (`execute_nested_block`, the
//! contract inference walker, a user Word's stored body) keeps running
//! unmodified now that code and data share one Vector domain: any Vector is
//! executable.
//!
//! `EXEC`, `CONTRACT`, `DEF`, and the higher-order words (`MAP`/`FILTER`/
//! `FOLD`/`SCAN`) all reach a Vector value that needs to run as
//! instructions. Rather than a second execution loop keyed on `&[Value]`,
//! this converts the elements back to tokens and hands them to the existing,
//! already-correct loop (tail-call elimination, error
//! diagnosis context) — the same design `REFLECT` used for its canonical
//! wire format before this unification removed it.
//!
//! A literal's original lexeme is not preserved: a `Value::Scalar` that came
//! from source `1.0` re-synthesizes as `1`. This is not a loss the language
//! cares about — LANG.VALUES.DENOTATION already says a value's construction
//! history is not part of the value, so `1.0` and `1` denoting the same
//! Scalar are `EQ` and were always meant to be indistinguishable once built.
//! A value no source text denotes — an exact irrational, a NIL with its
//! reason, a Record — crosses as a `Token::Value` carrying it whole, so
//! running the Vector pushes exactly the element it holds. `DEF` is the one
//! caller that keeps the tokens: a definition is its source, so it writes
//! such a value back as the source that builds it
//! (`value_elements_to_source_tokens`).

use crate::error::{AjisaiError, Result};
use crate::types::exact::value::ExactReal;
use crate::types::fraction::Fraction;
use crate::types::{Token, Value, ValueData};
use num_bigint::BigInt;
use num_traits::One;

pub(crate) fn value_elements_to_tokens(elements: &[Value]) -> Result<Vec<Token>> {
    let mut tokens = Vec::with_capacity(elements.len());
    for element in elements {
        push_value_as_tokens(element, &mut tokens)?;
    }
    Ok(tokens)
}

fn push_value_as_tokens(value: &Value, out: &mut Vec<Token>) -> Result<()> {
    match &value.data {
        ValueData::Truths(t) => return push_value_as_tokens(&t.to_vector(), out),
        ValueData::Symbol(name) => out.push(Token::Symbol(name.into())),
        ValueData::Text(s) => out.push(Token::String(s.clone())),
        // The value is already in hand, so it is carried across rather than
        // formatted into a string for the executor to parse back. That round
        // trip — parsed value → string → parsed value — happened once per
        // element of every higher-order block holding a number.
        ValueData::Scalar(f) => out.push(Token::number_from_value(f.clone())),
        ValueData::Boolean(true) => out.push(Token::Symbol("TRUE".into())),
        ValueData::Boolean(false) => out.push(Token::Symbol("FALSE".into())),
        // A value no source text denotes is carried across whole: a NIL
        // keeps its reason (the `NIL` name would denote a literal NIL, a
        // different value), and a Record has no literal of its own.
        ValueData::Nil(_) | ValueData::Record(_) => out.push(Token::Value(Box::new(value.clone()))),
        ValueData::Vector(children) => {
            out.push(Token::VectorStart);
            for child in children.iter() {
                push_value_as_tokens(child, out)?;
            }
            out.push(Token::VectorEnd);
        }
        ValueData::Tensor { .. } => {
            let nested = value
                .as_vector_view()
                .expect("Tensor always has a Vector view");
            out.push(Token::VectorStart);
            for child in nested.iter() {
                push_value_as_tokens(child, out)?;
            }
            out.push(Token::VectorEnd);
        }
        // An exact irrational has no number-literal lexeme (LANG.VALUES.EXACT:
        // a literal denotes a rational), so it too is carried whole.
        ValueData::ExactScalar(_) => out.push(Token::Value(Box::new(value.clone()))),
    }
    Ok(())
}

/// A definition body with every carried value written back as source.
///
/// A definition is kept as its source (LANG.DICTIONARY.MUTATION): the
/// dictionary saves, exports, shows and identifies a Word by the text of its
/// body, and a session restores it by running that text through `DEF` again.
/// A body built from a computed Vector can carry a value no source text
/// denotes — `value_elements_to_tokens` would carry it as a `Token::Value`;
/// its saved text was a display form, and a restored session read
/// `{ 'k' 5/1 }` as `unknownWord: {`. So `DEF` writes each such value back as
/// the source that builds it — a Record as `[ keys ] [ values ] RECORD`, an
/// exact irrational as its normal form `0 m SQRT c MUL ADD …`, a Vector that
/// holds either as `… n COLLECT` (a Symbol inside one is read out of a
/// literal, `[ V ] 0 GET`, so it stays data) — and the body it keeps is the
/// same body in every session. The one value no source can build, a NIL
/// carrying a reason (the `NIL` name denotes the literal NIL and no other),
/// is refused.
///
/// The conversion works from the elements, not from tokens: a value carried
/// inside a Vector literal cannot be expanded in place, since inside `[ ]`
/// the expansion would be data.
pub(crate) fn value_elements_to_source_tokens(elements: &[Value]) -> Result<Vec<Token>> {
    let mut tokens = Vec::with_capacity(elements.len());
    for element in elements {
        match &element.data {
            // At the top level of a body a bare name is a call, as it is for
            // any Vector run as code.
            ValueData::Symbol(name) => tokens.push(Token::Symbol(name.into())),
            _ => push_source_expression(element, &mut tokens)?,
        }
    }
    Ok(tokens)
}

/// Whether `value` is written as itself inside a vector literal: a number, a
/// String the lexer reads back whole, a Boolean, a literal NIL, a Symbol
/// (data inside `[ ]`), or a Vector of such. A Record, an algebraic
/// irrational, a reasoned NIL and a String holding a quote right before
/// whitespace (`tokenizer::is_string_token_content`) are not.
fn writes_as_literal(value: &Value) -> bool {
    match &value.data {
        ValueData::Truths(t) => writes_as_literal(&t.to_vector()),
        ValueData::Symbol(_) | ValueData::Scalar(_) | ValueData::Boolean(_) => true,
        ValueData::Text(text) => crate::tokenizer::is_string_token_content(text),
        ValueData::ExactScalar(exact) => matches!(**exact, ExactReal::Rational(_)),
        ValueData::Nil(_) => value
            .nil_reason()
            .is_none_or(|reason| matches!(reason, crate::error::NilReason::Literal)),
        ValueData::Record(_) => false,
        ValueData::Vector(children) => children.iter().all(writes_as_literal),
        ValueData::Tensor { .. } => value
            .as_vector_view()
            .is_some_and(|children| children.iter().all(writes_as_literal)),
    }
}

/// The literal spelling of a value `writes_as_literal` admits.
fn push_literal(value: &Value, out: &mut Vec<Token>) {
    match &value.data {
        ValueData::Truths(t) => push_literal(&t.to_vector(), out),
        ValueData::Symbol(name) => out.push(Token::Symbol(name.into())),
        ValueData::Text(s) => out.push(Token::String(s.clone())),
        ValueData::Scalar(f) => out.push(Token::number_from_value(f.clone())),
        ValueData::ExactScalar(exact) => match &**exact {
            ExactReal::Rational(f) => out.push(Token::number_from_value(f.clone())),
            ExactReal::Algebraic(_) => unreachable!("writes_as_literal admits no irrational"),
        },
        ValueData::Boolean(true) => out.push(Token::Symbol("TRUE".into())),
        ValueData::Boolean(false) => out.push(Token::Symbol("FALSE".into())),
        ValueData::Nil(_) => out.push(Token::Symbol("NIL".into())),
        ValueData::Vector(_) | ValueData::Tensor { .. } => {
            out.push(Token::VectorStart);
            if let Some(children) = value.as_vector_view() {
                for child in children.iter() {
                    push_literal(child, out);
                }
            }
            out.push(Token::VectorEnd);
        }
        ValueData::Record(_) => unreachable!("writes_as_literal admits no Record"),
    }
}

/// Source that, run at the top level of a body, pushes `value`.
fn push_source_expression(value: &Value, out: &mut Vec<Token>) -> Result<()> {
    match &value.data {
        ValueData::Truths(t) => push_source_expression(&t.to_vector(), out),
        // At the top level a bare name is a call; the Symbol stays data by
        // being read out of a literal.
        ValueData::Symbol(name) => {
            out.push(Token::VectorStart);
            out.push(Token::Symbol(name.into()));
            out.push(Token::VectorEnd);
            out.push(Token::number_from_value(integer(0)));
            out.push(Token::Symbol("GET".into()));
            Ok(())
        }
        // A text no one String literal spells — a quote right before
        // whitespace would close it early — is the texts that do have one,
        // joined: `[ 'a'' ' b' ] JOIN`. Each piece ends at such a quote, so
        // the whitespace that would have closed it opens the next piece
        // instead, and every piece has a literal.
        ValueData::Text(text) if !writes_as_literal(value) => {
            out.push(Token::VectorStart);
            for piece in crate::tokenizer::string_literal_pieces(text) {
                if !crate::tokenizer::is_string_token_content(&piece) {
                    return Err(AjisaiError::declared(
                        "invalidDefinitionBody",
                        format!(
                            "expected a definition body writable as source, got one holding a String no source text spells ({text:?})"
                        ),
                    ));
                }
                out.push(Token::String(piece.into()));
            }
            out.push(Token::VectorEnd);
            out.push(Token::Symbol("JOIN".into()));
            Ok(())
        }
        ValueData::Nil(_) if !writes_as_literal(value) => {
            let reason = value
                .nil_reason()
                .map(|reason| reason.as_protocol_str().to_string())
                .unwrap_or_default();
            Err(AjisaiError::declared(
                "invalidDefinitionBody",
                format!(
                    "expected a definition body writable as source, got one holding a NIL that carries a reason ({reason}) — no source text denotes it, so the definition could not be saved or restored. Produce the NIL inside the body instead."
                ),
            ))
        }
        _ if writes_as_literal(value) => {
            push_literal(value, out);
            Ok(())
        }
        // The multiquadratic normal form ∑ cₘ·√m, replayed through the
        // public arithmetic exactly as the persistence format does.
        ValueData::ExactScalar(exact) => {
            out.push(Token::number_from_value(integer(0)));
            for (monomial, coefficient) in exact
                .algebraic_terms()
                .expect("an algebraic value has normal-form terms")
            {
                out.push(Token::number_from_value(Fraction::new(
                    monomial.clone(),
                    BigInt::one(),
                )));
                out.push(Token::Symbol("SQRT".into()));
                out.push(Token::number_from_value(coefficient.clone()));
                out.push(Token::Symbol("MUL".into()));
                out.push(Token::Symbol("ADD".into()));
            }
            Ok(())
        }
        ValueData::Record(record) => {
            push_vector_expression(record.keys(), out)?;
            push_vector_expression(record.values(), out)?;
            out.push(Token::Symbol("RECORD".into()));
            Ok(())
        }
        ValueData::Vector(_) | ValueData::Tensor { .. } => {
            let children = value
                .as_vector_view()
                .expect("a Vector or Tensor has a Vector view");
            push_vector_expression(&children, out)
        }
        ValueData::Nil(_) | ValueData::Text(_) | ValueData::Scalar(_) | ValueData::Boolean(_) => {
            unreachable!("written as a literal above")
        }
    }
}

fn integer(n: i64) -> Fraction {
    Fraction::new(BigInt::from(n), BigInt::one())
}

/// Every radicand the source written for `elements` takes a root of: each
/// monomial of an algebraic value's normal form, wherever the value sits —
/// at the top level, inside a Vector, or inside a Record. `DEF` takes those
/// roots before it commits (`execute_def::check_source_radicands_within_budget`).
pub(crate) fn algebraic_radicands(elements: &[Value], out: &mut Vec<BigInt>) {
    for value in elements {
        match &value.data {
            ValueData::ExactScalar(exact) => {
                if let Some(terms) = exact.algebraic_terms() {
                    out.extend(terms.into_iter().map(|(monomial, _)| monomial));
                }
            }
            ValueData::Vector(children) => algebraic_radicands(children, out),
            ValueData::Record(record) => {
                algebraic_radicands(record.keys(), out);
                algebraic_radicands(record.values(), out);
            }
            // A dense tensor holds rationals only, a truth column truth
            // values; the other domains hold no number.
            ValueData::Tensor { .. }
            | ValueData::Truths(_)
            | ValueData::Scalar(_)
            | ValueData::Boolean(_)
            | ValueData::Text(_)
            | ValueData::Symbol(_)
            | ValueData::Nil(_) => {}
        }
    }
}

/// A Vector of `children`: the literal when every child has one, otherwise
/// each child's expression and then `n COLLECT`.
fn push_vector_expression(children: &[Value], out: &mut Vec<Token>) -> Result<()> {
    if children.iter().all(writes_as_literal) {
        out.push(Token::VectorStart);
        for child in children {
            push_literal(child, out);
        }
        out.push(Token::VectorEnd);
        return Ok(());
    }
    for child in children {
        push_source_expression(child, out)?;
    }
    out.push(Token::number_from_value(integer(
        i64::try_from(children.len()).expect("a Vector shorter than i64::MAX"),
    )));
    out.push(Token::Symbol("COLLECT".into()));
    Ok(())
}
