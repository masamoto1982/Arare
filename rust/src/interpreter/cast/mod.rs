//! The text Words: `CHARS`, `JOIN`, `TRIM`, `UPPER`, `LOWER`, `TOKENIZE`,
//! `SEARCH`, `REPLACE` — and, in `cast_conversions`, the two casts `STR` and
//! `NUM` that cross between the String and number domains
//! (LANG.VALUES.DISJOINT).

pub(crate) mod cast_conversions;

pub use cast_conversions::{op_num, op_str};

use crate::error::{AjisaiError, NilReason, Result};
use crate::interpreter::cast::cast_conversions::is_string_value;
use crate::interpreter::ceiling_refusal::materialization_refused;
use crate::interpreter::collection_meter::{charge_materialization, charge_text_built};
use crate::interpreter::value_extraction_helpers::extract_operands;
use crate::interpreter::Interpreter;
use crate::types::Value;

/// Admit `count` parts holding `bytes` of text between them — what `CHARS`
/// and `TOKENIZE` cut a String into — or refuse them before they are built
/// (LANG.COLLECTIONS.BUDGET). Each part is a fresh element, so the count is
/// held to `materializedElements` as `RANGE`'s is, and priced as `RANGE`
/// prices one, plus the text the parts copy.
fn admit_parts(interp: &mut Interpreter, count: usize, bytes: usize) -> Result<()> {
    let limit = interp.runtime_limits.max_materialized_elements;
    if count > limit {
        return Err(materialization_refused(limit, Some(count as u128)));
    }
    charge_materialization(interp, count)?;
    charge_text_built(interp, bytes)
}

pub fn op_chars(interp: &mut Interpreter) -> Result<()> {
    let val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    let Some(text) = val.as_text() else {
        let got = val.domain_name();
        interp.stack.push(val);
        return Err(AjisaiError::declared(
            "nonText",
            format!("expected a String, got {got}"),
        ));
    };

    let (count, bytes) = (text.chars().count(), text.len());
    if let Err(e) = admit_parts(interp, count, bytes) {
        interp.stack.push(val);
        return Err(e);
    }
    let chars: Vec<Value> = val
        .as_text()
        .unwrap_or_default()
        .chars()
        .map(|c| Value::from_string(&c.to_string()))
        .collect();
    interp.stack.push(Value::from_vector(chars));
    Ok(())
}

pub fn op_join(interp: &mut Interpreter) -> Result<()> {
    let val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    let Some(children) = val.as_vector_view().map(|v| v.into_owned()) else {
        let got = val.domain_name();
        interp.stack.push(val);
        return Err(AjisaiError::declared(
            "nonVector",
            format!("expected a Vector, got {got}"),
        ));
    };

    // Only Strings: a String is a value domain of its own, not a Vector of
    // code points (LANG.VALUES.DISJOINT), and CHARS, JOIN's inverse, yields
    // one-character Strings. Taking an integer as a code point here was the
    // one place a number still stood for a character, and nothing produced
    // one to pass.
    //
    // A NIL element is a part of the text that is not known, so the text is
    // not known either: the first one is the answer, reason and all, as SQL's
    // `'a' || NULL` is NULL. Every element is still checked first — a
    // non-String is the program being wrong, which an absence does not hide.
    let mut first_absent: Option<&Value> = None;
    let mut bytes = 0usize;
    for (i, elem) in children.iter().enumerate() {
        if elem.is_nil() {
            first_absent.get_or_insert(elem);
            continue;
        }
        let Some(text) = elem.as_text() else {
            let got = elem.domain_name();
            interp.stack.push(val);
            return Err(AjisaiError::declared(
                "nonText",
                format!("expected a Vector of Strings, got {got} at index {i}"),
            ));
        };
        bytes += text.len();
    }
    if let Some(absent) = first_absent {
        let absent = absent.clone();
        interp.stack.push(absent);
        return Ok(());
    }

    if let Err(e) = charge_text_built(interp, bytes) {
        interp.stack.push(val);
        return Err(e);
    }
    let mut result = String::with_capacity(bytes);
    for elem in children.iter() {
        result.push_str(elem.as_text().unwrap_or_default());
    }
    interp.stack.push(Value::from_string(&result));
    Ok(())
}

/// The String on top of the stack, taken off it, or the `nonText` every text
/// Word declares with the operand left in place.
fn pop_text(interp: &mut Interpreter) -> Result<Value> {
    let val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    if is_string_value(&val) {
        return Ok(val);
    }
    let got = val.domain_name();
    interp.stack.push(val);
    Err(AjisaiError::declared(
        "nonText",
        format!("expected a String, got {got}"),
    ))
}

pub fn op_trim(interp: &mut Interpreter) -> Result<()> {
    let val = pop_text(interp)?;
    let bytes = val.as_text().unwrap_or_default().trim().len();
    if let Err(e) = charge_text_built(interp, bytes) {
        interp.stack.push(val);
        return Err(e);
    }
    interp
        .stack
        .push(Value::from_string(val.as_text().unwrap_or_default().trim()));
    Ok(())
}

/// Map every character of the String on top through `map`, charging the
/// mapped text's exact length before building it: a case mapping can
/// lengthen text (`'ΐ' UPPER` is three characters), so the input's length
/// does not bound what is built.
fn map_chars<I: Iterator<Item = char>>(interp: &mut Interpreter, map: fn(char) -> I) -> Result<()> {
    let val = pop_text(interp)?;
    let text = val.as_text().unwrap_or_default();
    let bytes = text.chars().flat_map(map).map(char::len_utf8).sum();
    if let Err(e) = charge_text_built(interp, bytes) {
        interp.stack.push(val);
        return Err(e);
    }
    let mapped: String = val
        .as_text()
        .unwrap_or_default()
        .chars()
        .flat_map(map)
        .collect();
    interp.stack.push(Value::from_string(&mapped));
    Ok(())
}

/// `UPPER`: Unicode's default, locale-independent upper-case mapping,
/// applied character by character — the table is Unicode's own, which no
/// definition over `CHARS` and `JOIN` could carry, so the Word is native.
/// `'straße' UPPER` is `'STRASSE'`.
pub fn op_upper(interp: &mut Interpreter) -> Result<()> {
    map_chars(interp, char::to_uppercase)
}

/// `LOWER`: the default lower-case mapping, character by character. No
/// context- or language-specific rule (final sigma, Turkish dotless i) is
/// applied — `str::to_lowercase` would apply the final-sigma rule — so the
/// same text lowers the same way wherever it is run, and `CHARS LOWER` per
/// character agrees with `LOWER` of the whole.
pub fn op_lower(interp: &mut Interpreter) -> Result<()> {
    map_chars(interp, char::to_lowercase)
}

pub fn op_tokenize(interp: &mut Interpreter) -> Result<()> {
    let sep_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    let src_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow());
    let src_val = match src_val {
        Ok(v) => v,
        Err(e) => {
            interp.stack.push(sep_val);
            return Err(e);
        }
    };

    let restore = |interp: &mut Interpreter, a: Value, b: Value| {
        interp.stack.push(a);
        interp.stack.push(b);
    };

    if !is_string_value(&src_val) {
        let got = src_val.domain_name();
        restore(interp, src_val, sep_val);
        return Err(AjisaiError::declared(
            "nonText",
            format!("expected a String, got {got}"),
        ));
    }
    if !is_string_value(&sep_val) {
        let got = sep_val.domain_name();
        restore(interp, src_val, sep_val);
        return Err(AjisaiError::declared(
            "nonText",
            format!("expected a String separator, got {got}"),
        ));
    }

    let src = src_val.as_text().unwrap_or_default();
    let sep = sep_val.as_text().unwrap_or_default();

    // The parts hold the source less every separator cut out of it.
    let (count, bytes) = if sep.is_empty() {
        (src.chars().count(), src.len())
    } else {
        let cuts = src.matches(sep).count();
        (cuts + 1, src.len() - cuts * sep.len())
    };
    if let Err(e) = admit_parts(interp, count, bytes) {
        restore(interp, src_val, sep_val);
        return Err(e);
    }

    // The empty separator splits between every character, the same reading
    // SEARCH and REPLACE give the empty pattern (it matches everywhere). This
    // keeps TOKENIZE total over Text × Text.
    let parts: Vec<Value> = if sep.is_empty() {
        src.chars()
            .map(|c| Value::from_string(&c.to_string()))
            .collect()
    } else {
        src.split(sep).map(Value::from_string).collect()
    };
    interp.stack.push(Value::from_vector(parts));
    Ok(())
}

// Text search Words: `SEARCH` and `REPLACE` (LANG.VALUES.DISJOINT).
//
// `INDEX-OF` and a substitution, for Text. Spelled over `CHARS` each is a
// window compared at every position; the Word is the one pass
// (docs/dev/ajisai-minimal-core-identity.md 付録 B).
/// The String operands, taken off the stack, or the `nonText` every text Word
/// declares with them left in place.
fn texts(interp: &mut Interpreter, count: usize) -> Result<Vec<Value>> {
    let operands = extract_operands(interp, count)?;
    if operands.iter().all(is_string_value) {
        return Ok(operands);
    }
    let got = operands
        .iter()
        .find(|operand| !is_string_value(operand))
        .map_or("NIL", |operand| operand.domain_name());
    interp.stack.extend(operands);
    Err(AjisaiError::declared(
        "nonText",
        format!("expected Strings, got {got}"),
    ))
}

/// `SEARCH ( [ text ] [ needle ] -> [ index ] )`: the character position at
/// which `needle` first occurs, counted as `CHARS` counts; `notFound`
/// when it does not occur. An empty needle is found at 0.
pub fn op_search(interp: &mut Interpreter) -> Result<()> {
    let operands = texts(interp, 2)?;
    let text = |i: usize| operands[i].as_text().unwrap_or_default();
    let (haystack, needle) = (text(0), text(1));
    match haystack.find(needle) {
        Some(byte_offset) => {
            let position = haystack[..byte_offset].chars().count();
            interp.stack.push(Value::from_int(position as i64));
        }
        None => interp
            .stack
            .push(Value::nil_with_reason(NilReason::NotFound)),
    }
    Ok(())
}

/// `REPLACE ( [ text ] [ from ] [ to ] -> [ text' ] )`: every occurrence of
/// `from` replaced by `to`, found left to right without overlap. An empty
/// `from` matches nothing, so the text comes back unchanged rather than
/// growing at every position.
pub fn op_replace(interp: &mut Interpreter) -> Result<()> {
    let operands = texts(interp, 3)?;
    let text = |i: usize| operands[i].as_text().unwrap_or_default();
    let (source, from, to) = (text(0), text(1), text(2));
    // Every match trades `from`'s bytes for `to`'s, so the result's length is
    // known before it is built; a short `from` and a long `to` multiply the
    // text each time they run.
    let matches = if from.is_empty() {
        0
    } else {
        source.matches(from).count()
    };
    let bytes =
        (source.len() - matches * from.len()).saturating_add(matches.saturating_mul(to.len()));
    let replaced = match charge_text_built(interp, bytes) {
        Err(e) => {
            interp.stack.extend(operands);
            return Err(e);
        }
        Ok(()) if matches == 0 => source.to_owned(),
        Ok(()) => source.replace(from, to),
    };
    interp.stack.push(Value::from_string(&replaced));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::cast::cast_conversions::is_string_value;
    use crate::interpreter::value_extraction_helpers::value_as_string;

    #[tokio::test]
    async fn test_chars_basic() {
        let mut interp = Interpreter::new();

        interp.execute("'hello' CHARS JOIN").await.unwrap();
        assert_eq!(interp.stack.len(), 1);

        if let Some(val) = interp.stack.last() {
            assert!(is_string_value(val));
            let s = value_as_string(val).unwrap();
            assert_eq!(s, "hello");
        }
    }

    #[tokio::test]
    async fn test_chars_rejects_a_vector() {
        // `[ 42 ]` is a Vector of one number. It used to satisfy CHARS because
        // a codepoint vector *was* a String; CHARS' contract registers
        // `nonText`, and now that is what happens.
        let mut interp = Interpreter::new();
        let result = interp.execute("[ 42 ] CHARS").await;
        assert!(result.is_err(), "CHARS must reject a Vector");
    }

    #[tokio::test]
    async fn test_chars_yields_one_character_strings() {
        let mut interp = Interpreter::new();
        interp.execute("'ab' CHARS").await.unwrap();
        let val = interp.stack.last().expect("a result");
        let items = val.as_vector_view().expect("a vector");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_text(), Some("a"));
        assert_eq!(items[1].as_text(), Some("b"));
    }

    #[tokio::test]
    async fn test_join_basic() {
        let mut interp = Interpreter::new();
        interp
            .execute("[ 'h' 'e' 'l' 'l' 'o' ] JOIN")
            .await
            .unwrap();
        assert_eq!(interp.stack.len(), 1);

        if let Some(val) = interp.stack.last() {
            assert!(is_string_value(val));
            let s = value_as_string(val).unwrap();
            assert_eq!(s, "hello");
        }
    }

    #[tokio::test]
    async fn test_join_of_the_empty_vector_is_the_empty_string() {
        // Joining nothing is the identity of concatenation. Both endpoints of
        // this used to be inexpressible, so it had to be an error.
        let mut interp = Interpreter::new();
        interp.execute("[ ] JOIN").await.unwrap();
        assert_eq!(interp.stack.last().and_then(|v| v.as_text()), Some(""));
    }

    #[tokio::test]
    async fn test_chars_join_round_trips_the_empty_string() {
        let mut interp = Interpreter::new();
        interp.execute("'' CHARS JOIN").await.unwrap();
        assert_eq!(interp.stack.last().and_then(|v| v.as_text()), Some(""));
    }

    #[tokio::test]
    async fn test_chars_join_roundtrip() {
        let mut interp = Interpreter::new();

        interp.execute("'hello' CHARS JOIN").await.unwrap();

        if let Some(val) = interp.stack.last() {
            assert!(is_string_value(val));
            let s = value_as_string(val).unwrap();
            assert_eq!(s, "hello");
        }
    }

    #[tokio::test]
    async fn test_chars_reverse_join() {
        let mut interp = Interpreter::new();

        interp.execute("'hello' CHARS REVERSE JOIN").await.unwrap();

        if let Some(val) = interp.stack.last() {
            assert!(is_string_value(val));
            let s = value_as_string(val).unwrap();
            assert_eq!(s, "olleh");
        }
    }

    #[tokio::test]
    async fn test_nil_pushes_constant() {
        let mut interp = Interpreter::new();
        let result = interp.execute("NIL").await;
        assert!(result.is_ok());
        assert_eq!(interp.stack.len(), 1);

        if let Some(val) = interp.stack.last() {
            assert!(val.is_nil());
        }
    }

    #[tokio::test]
    async fn test_nil_multiple() {
        let mut interp = Interpreter::new();
        let result = interp.execute("NIL NIL NIL").await;
        assert!(result.is_ok());
        assert_eq!(interp.stack.len(), 3);

        for val in interp.stack.iter() {
            assert!(val.is_nil());
        }
    }

    fn top_str(interp: &Interpreter) -> String {
        let v = interp.stack.last().unwrap();
        assert!(is_string_value(v));
        value_as_string(v).unwrap()
    }

    #[tokio::test]
    async fn trim_both() {
        let mut interp = Interpreter::new();
        interp.execute("'  hello  ' TRIM").await.unwrap();
        assert_eq!(top_str(&interp), "hello");
    }

    #[tokio::test]
    async fn tokenize_basic() {
        let mut interp = Interpreter::new();
        interp.execute("'a,b,c' ',' TOKENIZE").await.unwrap();
        let v = interp.stack.last().unwrap();
        let parts = v.as_vector_view().unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(value_as_string(&parts[0]).unwrap(), "a");
        assert_eq!(value_as_string(&parts[1]).unwrap(), "b");
        assert_eq!(value_as_string(&parts[2]).unwrap(), "c");
    }

    #[tokio::test]
    async fn tokenize_no_match() {
        let mut interp = Interpreter::new();
        interp.execute("'abc' ',' TOKENIZE").await.unwrap();
        let v = interp.stack.last().unwrap();
        let parts = v.as_vector_view().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(value_as_string(&parts[0]).unwrap(), "abc");
    }

    #[tokio::test]
    async fn tokenize_empty_separator_splits_characters() {
        let mut interp = Interpreter::new();
        interp.execute("'abc' '' TOKENIZE").await.unwrap();
        let v = interp.stack.last().unwrap();
        let parts = v.as_vector_view().unwrap();
        let got: Vec<String> = parts.iter().map(|p| value_as_string(p).unwrap()).collect();
        assert_eq!(got, vec!["a", "b", "c"]);
    }

    #[tokio::test]
    async fn trim_passes_an_absent_operand_through() {
        let mut interp = Interpreter::new();
        interp.execute("-1 SQRT TRIM NIL-REASON").await.unwrap();
        let reason = interp
            .stack
            .last()
            .and_then(|v| v.as_text().map(str::to_string));
        assert_eq!(reason.as_deref(), Some("domainMiss"));
    }
}
