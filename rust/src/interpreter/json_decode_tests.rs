//! The decoder alone, below the Word: what text decodes to, and what it is
//! refused as.

use super::*;

const MAX_NESTING: usize = crate::interpreter::runtime_limits::DEFAULT_MAX_NESTING_DEPTH;

fn decode(text: &str) -> Option<Value> {
    Decoder {
        bytes: text.as_bytes(),
        pos: 0,
        max_nesting: MAX_NESTING,
        max_digits: crate::interpreter::runtime_limits::DEFAULT_MAX_NUMERIC_LITERAL_DIGITS,
        max_elements: usize::MAX,
        elements: 0,
    }
    .decode()
    .ok()
}

fn reject(text: &str) -> Option<Reject> {
    Decoder {
        bytes: text.as_bytes(),
        pos: 0,
        max_nesting: MAX_NESTING,
        max_digits: crate::interpreter::runtime_limits::DEFAULT_MAX_NUMERIC_LITERAL_DIGITS,
        max_elements: usize::MAX,
        elements: 0,
    }
    .decode()
    .err()
}

#[test]
fn numbers_are_exact_rationals() {
    assert_eq!(decode("0.1").unwrap().to_string(), "1/10");
    assert_eq!(decode("-2.5e1").unwrap().to_string(), "-25/1");
    assert_eq!(decode("1E-2").unwrap().to_string(), "1/100");
    assert_eq!(decode("0e99999999").unwrap().to_string(), "0/1");
    for bad in ["01", "1.", ".5", "+1", "1e", "--1", "0x1"] {
        assert!(decode(bad).is_none(), "{bad} must not decode");
    }
}

#[test]
fn strings_decode_their_escapes() {
    assert_eq!(
        decode(r#""a\"b\\c\/\né😀""#).unwrap().as_text(),
        Some("a\"b\\c/\né😀")
    );
    assert!(decode("\"tab\there\"").is_none());
    assert!(decode(r#""\ud83d""#).is_none());
    assert!(decode("\"open").is_none());
}

#[test]
fn containers_nest_to_the_bound_and_are_refused_past_it() {
    let deep = "[".repeat(MAX_NESTING) + &"]".repeat(MAX_NESTING);
    assert!(decode(&deep).is_some());
    let deeper = "[".repeat(MAX_NESTING + 1) + &"]".repeat(MAX_NESTING + 1);
    assert_eq!(reject(&deeper), Some(Reject::TooDeep));
    // Too deep is decided at the opening bracket, before the text is
    // known to be well formed: the machine could not hold it either way.
    assert_eq!(reject(&"[".repeat(100_000)), Some(Reject::TooDeep));
    assert_eq!(reject(&"[".repeat(10)), Some(Reject::Malformed));
    assert_eq!(
        decode("[1,[2,[]],{}]").unwrap().to_string(),
        "1/1 [ 2/1 [ ] ] [ ] [ ] RECORD 3 COLLECT"
    );
    assert_eq!(
        decode(r#" { "a" : 1 , "b" : [ true , null ] } "#)
            .unwrap()
            .to_string(),
        "[ 'a' 'b' ] [ 1/1 [ TRUE NIL ] ] RECORD"
    );
}

#[test]
fn malformed_text_has_no_value() {
    for bad in [
        "",
        " ",
        "[1,]",
        "{\"a\":1,}",
        "{\"a\"}",
        "{a:1}",
        "[1] 2",
        "nul",
        "tru",
        "[1 2]",
        "{\"a\":1 \"b\":2}",
        "{\"a\":1,\"a\":2}",
        "{,}",
        "[,]",
    ] {
        assert!(decode(bad).is_none(), "{bad:?} must not decode");
    }
}
