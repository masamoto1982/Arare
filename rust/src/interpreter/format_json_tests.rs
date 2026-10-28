//! Behavioral probes for `FORMAT`, `JSON-DECODE` and `JSON-ENCODE`: the
//! boundary Words, where a value becomes text under a stated rule and text
//! becomes a value without a guess (LANG.VALUES.EXACT, LANG.RECORDS.STRUCTURE).

use crate::interpreter::Interpreter;
use crate::test_support::{error_of, top};

#[tokio::test]
async fn format_rounds_a_tie_away_from_zero_like_round() {
    assert_eq!(top("1/3 5 FORMAT").await, "'0.33333'");
    assert_eq!(top("2/3 2 FORMAT").await, "'0.67'");
    assert_eq!(top("5/2 0 FORMAT").await, "'3'");
    assert_eq!(top("7/2 0 FORMAT").await, "'4'");
    assert_eq!(top("1/8 2 FORMAT").await, "'0.13'");
    assert_eq!(top("3/8 2 FORMAT").await, "'0.38'");
    assert_eq!(top("-1/8 2 FORMAT").await, "'-0.13'");
    // One rule in the language: FORMAT agrees with ROUND.
    assert_eq!(
        top("5/2 ROUND -5/2 ROUND 1/8 100 MUL ROUND 100 DIV").await,
        "3/1 -3/1 13/100"
    );
    assert_eq!(top("-1/1000 2 FORMAT").await, "'0.00'");
    assert_eq!(top("12345 2 FORMAT").await, "'12345.00'");
    assert_eq!(top("0 3 FORMAT").await, "'0.000'");
    assert_eq!(top("2 SQRT 3 FORMAT").await, "'1.414'");
    assert_eq!(top("2 SQRT -1 MUL 3 FORMAT").await, "'-1.414'");
    assert_eq!(top("2 SQRT 3 SQRT ADD 4 FORMAT").await, "'3.1463'");
    // Text, not a number: the rounded quantity never re-enters arithmetic.
    assert_eq!(error_of("1/3 2 FORMAT 1 ADD").await, "nonNumeric");
}

#[tokio::test]
async fn format_refuses_malformed_use_and_restores_operands() {
    assert_eq!(error_of("'x' 2 FORMAT").await, "nonNumeric");
    // A Vector value lifts FORMAT over its elements.
    assert_eq!(top("[ 1 2 ] 2 FORMAT").await, "[ '1.00' '2.00' ]");
    assert_eq!(error_of("1 -1 FORMAT").await, "invalidInteger");
    assert_eq!(error_of("1 1/2 FORMAT").await, "invalidInteger");
    assert_eq!(error_of("1 'x' FORMAT").await, "invalidInteger");
    // Both operands are data: an absent one passes through.
    assert_eq!(top("NIL 2 FORMAT NIL?").await, "TRUE");
    assert_eq!(top("1 NIL FORMAT NIL?").await, "TRUE");
    let mut interp = Interpreter::new();
    let _ = interp.execute("1 -1 FORMAT").await;
    assert_eq!(interp.stack.len(), 2);
    assert_eq!(
        top("1/3 'X' BIND 2 'N' BIND X N X N FORMAT").await,
        "1/3 2/1 '0.33'"
    );
}

#[tokio::test]
async fn json_decode_lands_each_json_kind_on_its_domain() {
    assert_eq!(
        top("'{\"a\": 1, \"b\": [true, null, \"x\"]}' JSON-DECODE").await,
        "[ 'a' 'b' ] [ 1/1 [ TRUE NIL 'x' ] ] RECORD"
    );
    assert_eq!(top("'0.1' JSON-DECODE").await, "1/10");
    assert_eq!(top("'0.1' JSON-DECODE 10 MUL 1 EQ").await, "TRUE");
    assert_eq!(top("'-1.5e2' JSON-DECODE").await, "-150/1");
    // Zero is zero at any scale, an exponent past `u32` included.
    assert_eq!(top("'0e9999999999' JSON-DECODE").await, "0/1");
    assert_eq!(
        top("'-0.0E-99999999999999999999999' JSON-DECODE").await,
        "0/1"
    );
    assert_eq!(top("'[]' JSON-DECODE").await, "[ ]");
    assert_eq!(top("'{}' JSON-DECODE").await, "[ ] [ ] RECORD");
    assert_eq!(top("'null' JSON-DECODE NIL-REASON").await, "'literal'");
    assert_eq!(top("'\"caf\\u00e9\"' JSON-DECODE").await, "'café'");
    assert_eq!(
        top("'{\"k\": {\"n\": [1, [2]]}}' JSON-DECODE 'k' GET 'n' GET 1 GET").await,
        "[ 2/1 ]"
    );
}

#[tokio::test]
async fn json_decode_projects_invalid_encoding_and_refuses_non_text() {
    for bad in [
        "''",
        "'[1,'",
        "'{\"a\":1,\"a\":2}'",
        "'[1] 2'",
        "'nul'",
        "'{a:1}'",
    ] {
        assert_eq!(
            top(&format!("{bad} JSON-DECODE NIL-REASON")).await,
            "'invalidEncoding'",
            "{bad}"
        );
    }
    assert_eq!(error_of("5 JSON-DECODE").await, "nonText");
    assert_eq!(top("NIL JSON-DECODE NIL?").await, "TRUE");
    assert_eq!(top("'[1]' 'S' BIND S S JSON-DECODE").await, "'[1]' [ 1/1 ]");
}

#[tokio::test]
async fn json_encode_writes_exactly_or_not_at_all() {
    assert_eq!(
        top("[ 'a' 'b' ] [ 1/4 [ TRUE NIL 'x' ] ] RECORD JSON-ENCODE").await,
        "'{\"a\":0.25,\"b\":[true,null,\"x\"]}'"
    );
    assert_eq!(top("1/3 JSON-ENCODE").await, "'\"1/3\"'");
    assert_eq!(top("-7/2 JSON-ENCODE").await, "'-3.5'");
    assert_eq!(top("100 JSON-ENCODE").await, "'100'");
    assert_eq!(top("NIL JSON-ENCODE").await, "'null'");
    assert_eq!(top("'a\"b' JSON-ENCODE").await, "'\"a\\\"b\"'");
    assert_eq!(
        top("[ [ 1 2 ] [ 3 4 ] ] JSON-ENCODE").await,
        "'[[1,2],[3,4]]'"
    );
    assert_eq!(top("[ ] JSON-ENCODE").await, "'[]'");
    for no_image in [
        "2 SQRT",
        "2 SQRT 3 SQRT ADD",
        "[ ADD ] 0 GET",
        "[ 1 ] [ 2 ] RECORD",
        "[ 1 2 SQRT ]",
    ] {
        assert_eq!(
            top(&format!("{no_image} JSON-ENCODE NIL-REASON")).await,
            "'domainMiss'",
            "{no_image}"
        );
    }
}

#[tokio::test]
async fn decode_after_encode_is_the_identity_on_the_json_image() {
    for value in [
        "[ 'a' 'b' ] [ 1/4 [ TRUE NIL 'x' ] ] RECORD",
        "-7/2",
        "'quote \" and \\ and newline\n'",
        "[ [ 1 2 ] [ 3 4 ] ]",
        "[ ]",
        "[ ] [ ] RECORD",
    ] {
        // BIND holds the value; DECODE rebuilds it beside a second
        // reading of it, and EQ takes both.
        assert_eq!(
            top(&format!("{value} 'V' BIND V V JSON-ENCODE JSON-DECODE EQ")).await,
            "TRUE",
            "{value}"
        );
    }
    // A rational with no finite decimal travels as its lexeme in a
    // string, and comes back as that String: NUM recovers the number,
    // and no digit was rounded on the way.
    assert_eq!(top("1/3 JSON-ENCODE JSON-DECODE").await, "'1/3'");
    assert_eq!(
        top("1/3 'V' BIND V V JSON-ENCODE JSON-DECODE NUM EQ").await,
        "TRUE"
    );
}
