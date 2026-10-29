//! AR-VER-212: robustness — every way a Word refuses malformed use, as an
//! ERROR that stops the program and names the Word (AR-HLR-025, AR-LLR-015).

use arare_interp::{run, ErrorKind as K, RunError};

fn error(src: &str) -> (String, K) {
    match run(src) {
        Err(RunError::Interp(e)) => (e.word.name().to_string(), e.kind),
        other => panic!("{src}: expected an error, got {other:?}"),
    }
}

#[test]
fn ar_ver_212_underflow() {
    for (src, needed, had) in [
        ("ADD", 2, 0),
        ("1 SUB", 2, 1),
        ("NEG", 1, 0),
        ("1 2 QUANTIZE", 3, 2),
        ("[ 1 ] FOLD", 3, 1),
    ] {
        assert_eq!(error(src).1, K::StackUnderflow { needed, had }, "{src}");
    }
}

#[test]
fn ar_ver_212_types() {
    assert_eq!(error("TRUE 1 ADD"), ("ADD".into(), K::Type("a number")));
    assert_eq!(error("1 TRUE LT").1, K::Type("a number"));
    assert_eq!(
        error("1 TRUE AND"),
        ("AND".into(), K::Type("a truth value"))
    );
    assert_eq!(error("1 NOT").1, K::Type("a truth value"));
    assert_eq!(
        error("1 2 3 SELECT"),
        ("SELECT".into(), K::Type("a truth value"))
    );
    assert_eq!(error("[ 1 ] 2 MAP"), ("MAP".into(), K::Type("a block")));
    assert_eq!(error("1 [ 2 MUL ] MAP").1, K::Type("a vector"));
    assert_eq!(
        error("[ 1 2 ] [ 2 MUL ] FILTER"),
        (
            "FILTER".into(),
            K::Type("a predicate answering a truth value")
        )
    );
    assert_eq!(error("[ TRUE ] SUM").1, K::Type("a vector of numbers"));
    assert_eq!(error("5 SUM").1, K::Type("a vector"));
}

#[test]
fn ar_ver_212_shapes_and_operands() {
    assert_eq!(
        error("[ 1 2 ] [ 1 2 3 ] ADD"),
        ("ADD".into(), K::Shape { left: 2, right: 3 })
    );
    assert_eq!(
        error("[ TRUE FALSE ] [ TRUE TRUE TRUE ] AND").1,
        K::Shape { left: 2, right: 3 }
    );
    assert_eq!(
        error("[ 1 2 ] 0 [ TRUE FALSE TRUE ] SELECT").1,
        K::Shape { left: 2, right: 3 }
    );
    assert_eq!(
        error("1 63 RESCALE").1,
        K::BadOperand("a shift from 0 to 62")
    );
    assert_eq!(
        error("1 -1 RESCALE").1,
        K::BadOperand("a shift from 0 to 62")
    );
    assert_eq!(
        error("1 12 NARROW").1,
        K::BadOperand("a width of 8, 16, 32 or 64")
    );
    assert_eq!(
        error("1 1 3 DIV NARROW").1,
        K::BadOperand("a width of 8, 16, 32 or 64")
    );
    assert_eq!(
        error("1 8 32 QUANTIZE").1,
        K::BadOperand("an exponent within ±31")
    );
    // The element stays on the block's stack: it ends with three values.
    assert_eq!(
        error("[ 1 ] [ 1 2 ] MAP"),
        ("MAP".into(), K::BlockResult(3))
    );
    assert_eq!(
        error("[ 1 ] 0 [ ADD ADD ] FOLD").1,
        K::StackUnderflow { needed: 2, had: 1 }
    );
}

#[test]
fn ar_ver_212_messages() {
    for (src, message) in [
        ("1 TRUE ADD", "ADD: expected a number"),
        (
            "[ 1 2 ] [ 1 2 3 ] ADD",
            "ADD: vectors of length 2 and 3 do not pair",
        ),
        (
            "1 TRUE NARROW",
            "NARROW: accepts a width of 8, 16, 32 or 64",
        ),
        (
            "[ 1 ] 0 [ ] FOLD",
            "FOLD: a block must leave exactly one value, not 2",
        ),
        ("ADD", "ADD: needs 2 operand(s), the stack has 0"),
    ] {
        assert_eq!(run(src).unwrap_err().to_string(), message, "{src}");
    }
}

#[test]
fn ar_ver_212_scale_overflow() {
    assert_eq!(
        error("1 8 20 QUANTIZE 1 8 20 QUANTIZE MUL"),
        ("MUL".into(), K::ScaleOverflow)
    );
    assert_eq!(
        error("1 8 30 QUANTIZE 2 RESCALE"),
        ("RESCALE".into(), K::ScaleOverflow)
    );
    let shown = run("1 8 30 QUANTIZE 2 RESCALE").unwrap_err().to_string();
    assert_eq!(shown, "RESCALE: the result exponent would leave ±31");
}

#[test]
fn ar_ver_212_front_refusals_reach_run() {
    assert!(matches!(run("1 FOO"), Err(RunError::Front(_))));
    assert!(run("0.5").unwrap_err().to_string().contains("QUANTIZE"));
}
