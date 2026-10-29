//! AR-VER-211: every Word's normal behaviour (AR-LLR-013 to AR-LLR-016),
//! written as programs and their final stacks.

use arare_interp::{run, Machine, Reason, Value};

fn shown(src: &str) -> String {
    run(src)
        .unwrap_or_else(|e| panic!("{src}: {e}"))
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join("  ")
}

#[test]
fn ar_ver_211_arithmetic() {
    assert_eq!(
        shown("2 3 ADD 2 3 SUB 2 3 MUL 7 2 DIV 5 NEG"),
        "5/1  -1/1  6/1  4/1  -5/1"
    );
    assert_eq!(shown("[ 1 2 3 ] [ 4 5 6 ] ADD"), "[ 5/1 7/1 9/1 ]");
    assert_eq!(
        shown("[ 1 2 3 ] 10 MUL 10 [ 1 2 ] SUB"),
        "[ 10/1 20/1 30/1 ]  [ 9/1 8/1 ]"
    );
    assert_eq!(
        shown("[ [ 1 2 ] [ 3 ] ] [ 10 ] ADD"),
        "[ [ 11/1 12/1 ] [ 13/1 ] ]"
    );
    assert_eq!(shown("[ 1 2 3 ] [ 1 0 2 ] DIV"), "[ 1/1 1/0 2/1 ]");
    assert_eq!(
        shown("1 0 DIV 1 0 DIV ADD 1 0 DIV 0 MUL 5 1 0 DIV DIV"),
        "0/0  0/0  0/1"
    );
    assert_eq!(
        shown("0.75 8 -4 QUANTIZE 3 ADD 1 16 -12 QUANTIZE 3 DIV"),
        "15/4  1365/4096"
    );
}

#[test]
fn ar_ver_211_scales() {
    assert_eq!(
        shown("7 1 RESCALE 5 1 RESCALE 300 8 NARROW 100 8 NARROW"),
        "8/1  4/1  1/0  100/1"
    );
    assert_eq!(
        shown("0.875 8 -3 QUANTIZE 8 -1 QUANTIZE 3 16 2 QUANTIZE"),
        "1/1  4/1"
    );
    assert_eq!(shown("[ 1 2 ] 1 RESCALE"), "[ 0/1 2/1 ]");
    // QUANTIZE's operands may be integers at any scale.
    assert_eq!(shown("5 8 2 4 RESCALE QUANTIZE"), "5/1");
}

#[test]
fn ar_ver_211_comparison_and_truth() {
    assert_eq!(
        shown("1 2 LT 2 1 LT 2 2 EQ [ 1 2 ] [ 1 2 ] EQ 0 0 DIV 0 0 DIV EQ"),
        "TRUE  FALSE  TRUE  TRUE  TRUE"
    );
    assert_eq!(shown("1 0 DIV 2 LT"), "NIL(orderOfPoint)");
    assert_eq!(shown("[ 1 2 3 ] 2 LT"), "[ TRUE FALSE FALSE ]");
    assert_eq!(
        shown("TRUE FALSE AND TRUE NIL AND FALSE NIL AND NIL NOT FALSE NOT"),
        "FALSE  NIL(literal)  FALSE  NIL(literal)  TRUE"
    );
    assert_eq!(
        shown("[ TRUE FALSE NIL ] TRUE AND [ TRUE NIL ] NOT"),
        "[ TRUE FALSE NIL(literal) ]  [ FALSE NIL(literal) ]"
    );
    // An UNKNOWN keeps the reason it came with.
    assert_eq!(shown("1 0 DIV 1 LT TRUE AND"), "NIL(orderOfPoint)");
}

#[test]
fn ar_ver_211_select_and_aggregate() {
    assert_eq!(
        shown("1 2 TRUE SELECT 1 2 FALSE SELECT 1 2 NIL SELECT"),
        "1/1  2/1  2/1"
    );
    assert_eq!(
        shown("[ 1 2 3 ] 0 [ TRUE NIL FALSE ] SELECT"),
        "[ 1/1 0/1 0/1 ]"
    );
    assert_eq!(
        shown("[ 1 2 3 ] SUM [ 1 NIL 3 ] SUM [ NIL NIL ] SUM [ ] SUM"),
        "6/1  4/1  NIL(emptyAggregate)  NIL(emptyAggregate)"
    );
}

#[test]
fn ar_ver_211_higher_order() {
    assert_eq!(
        shown("[ 1 2 3 4 ] 0 [ ADD ] FOLD [ 1 2 3 ] [ 2 MUL ] MAP"),
        "10/1  [ 2/1 4/1 6/1 ]"
    );
    assert_eq!(shown("[ 5 1 4 2 ] [ 3 LT ] FILTER"), "[ 1/1 2/1 ]");
    // A predicate answering NIL drops the element, as FALSE does.
    assert_eq!(shown("[ 1 0 3 ] [ 0 DIV 1 LT ] FILTER"), "[ ]");
    assert_eq!(shown("[ ] [ 2 MUL ] MAP [ ] 7 [ ADD ] FOLD"), "[ ]  7/1");
}

#[test]
fn ar_ver_211_nil_and_quotes() {
    assert_eq!(
        shown("NIL 1 ADD 1 NIL ADD [ 1 NIL 3 ] 2 MUL"),
        "NIL(literal)  NIL(literal)  [ 2/1 NIL(literal) 6/1 ]"
    );
    assert_eq!(
        shown("NIL [ 2 MUL ] MAP NIL [ TRUE ] FILTER NIL 0 [ ADD ] FOLD NIL SUM NIL 1 EQ"),
        "NIL(literal)  NIL(literal)  NIL(literal)  NIL(literal)  NIL(literal)"
    );
    // A quote used as data is evaluated into the vector of what it leaves.
    assert_eq!(shown("[ 1 2 ADD 4 ] 1 ADD"), "[ 4/1 5/1 ]");
    assert_eq!(
        shown("[ [ 1 2 ] [ 3 ] ] [ [ 1 ] ] ADD"),
        "[ [ 2/1 3/1 ] [ 4/1 ] ]"
    );
    assert_eq!(run("NIL").unwrap(), [Value::Nil(Reason::Literal)]);
    assert!(matches!(run("[ 1 ]").unwrap()[0], Value::Quote(_)));
}

#[test]
fn ar_ver_211_edges() {
    assert_eq!(
        shown("[ TRUE FALSE NIL ] [ NIL TRUE FALSE ] AND"),
        "[ NIL(literal) FALSE FALSE ]"
    );
    // AND reuses a one-element vector on either side.
    assert_eq!(
        shown("[ TRUE ] [ TRUE FALSE ] AND [ NIL FALSE ] [ TRUE ] AND"),
        "[ TRUE FALSE ]  [ NIL(literal) FALSE ]"
    );
    // EQ on truth values, on NILs inside vectors, on vectors of other lengths, across kinds.
    assert_eq!(
        shown("TRUE TRUE EQ TRUE FALSE EQ [ NIL ] [ NIL ] EQ [ 1 ] [ 1 2 ] EQ 1 TRUE EQ"),
        "TRUE  FALSE  TRUE  FALSE  FALSE"
    );
    // SELECT reuses a one-element THEN or ELSE across the lanes.
    assert_eq!(shown("[ 1 ] [ 2 3 ] [ TRUE FALSE ] SELECT"), "[ 1/1 3/1 ]");
    assert_eq!(run("[ 1 ]").unwrap()[0].to_string(), "[ …1 item(s) ]");
}

#[test]
fn ar_ver_211_steps() {
    let mut m = Machine::new(arare_front::parse("1 2 ADD").unwrap());
    assert!(!m.is_done());
    assert!(m.step().unwrap());
    assert_eq!(m.stack().len(), 1);
    assert!(m.step().unwrap() && m.step().unwrap());
    assert_eq!(m.stack()[0].to_string(), "3/1");
    assert!(m.is_done() && !m.step().unwrap());
}
