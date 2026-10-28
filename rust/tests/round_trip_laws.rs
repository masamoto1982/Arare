//! A value's display is source that rebuilds it.
//!
//! Ajisai renders a stack value as text a reader can copy back into the
//! editor and run wherever the value's domain has a literal: `[ 1/1 2/1 ]`,
//! `'ab'`, `TRUE`, `NIL`, `1/3`.
//!
//! The law is stated by executing it: render a value, run what was rendered,
//! and require the result to be the same value. A law about a display that is
//! only ever compared against another string would pin the spelling, which is
//! exactly what may change; this pins the property that makes the spelling
//! worth having.
//!
//! A Record has no literal — only `[ ]` delimits — so it renders as the phrase
//! that builds it, `[ keys ] [ values ] RECORD`, and a Vector holding one as
//! its elements followed by `n COLLECT`; both are source, and both are held to
//! the law below. So does a String holding a quote right before whitespace,
//! which no literal spells (the quote would close it): it renders as the
//! literals that do spell its pieces, joined, `[ 'a''' ' b' ] JOIN`.
//!
//! Three kinds of value are deliberately out of scope, because the display
//! does not claim to round-trip them and `types/display.rs` says so:
//!
//! - A Symbol on its own renders as its bare name, which calls a Word rather
//!   than pushing the name. (Inside a `COLLECT` phrase it is written
//!   `[ NAME ] 0 GET`, which does round-trip.)
//! - An irrational scalar renders its normal form as one token,
//!   `1/1+sqrt(2)`: no literal denotes an irrational, and the source that
//!   builds one (`1 2 SQRT ADD`) would read as three elements inside a
//!   Vector literal.
//! - A NIL carrying a reason renders as `NIL`, the name that denotes only the
//!   literal NIL.

use ajisai_core::interpreter::Interpreter;

/// Run `source` and render whatever it left on the stack.
async fn run(source: &str) -> Vec<String> {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute(source)
        .await
        .unwrap_or_else(|e| panic!("`{source}` should run, got: {e}"));
    interpreter
        .get_stack()
        .iter()
        .map(ToString::to_string)
        .collect()
}

/// The law: running a value's display leaves exactly that value.
///
/// Re-rendering rather than comparing values directly is what makes this
/// checkable through the public surface, and it is not weaker: the renderer is
/// total and deterministic, so two values that render alike are
/// indistinguishable to every observation surface there is
/// (LANG.OBSERVATION.PROTOCOL).
async fn assert_round_trips(program: &str) {
    let rendered = run(program).await;
    assert_eq!(
        rendered.len(),
        1,
        "`{program}` should leave exactly one value to round-trip, left {rendered:?}"
    );
    let display = &rendered[0];

    let again = run(display).await;
    assert_eq!(
        again.len(),
        1,
        "the display `{display}` should leave exactly one value, left {again:?} — \
         every fragment the renderer writes must net one stack value, or fragments \
         stop composing when they nest"
    );
    assert_eq!(
        &again[0], display,
        "`{program}` rendered as `{display}`, which rebuilt a different value"
    );
}

/// A Vector renders as its own literal, and the spelling is the one a reader
/// writes: this is the half of the law a round trip alone cannot fix, since a
/// wrong-but-consistent spelling would round-trip too.
#[tokio::test]
async fn a_vector_renders_as_its_own_literal() {
    for (program, expected) in [
        ("[ 1 2 3 ]", "[ 1/1 2/1 3/1 ]"),
        ("[ ]", "[ ]"),
        ("[ 'a' 'b' ]", "[ 'a' 'b' ]"),
        ("[ [ 1 ] [ 2 ] ]", "[ [ 1/1 ] [ 2/1 ] ]"),
    ] {
        let rendered = run(program).await;
        assert_eq!(
            rendered,
            [expected],
            "`{program}` should render as a literal"
        );
        assert_round_trips(program).await;
    }
}

/// Every other domain with a literal round-trips.
#[tokio::test]
async fn the_other_domains_round_trip() {
    for program in [
        "42",
        "-7",
        "1/3",
        "0.25",
        "'hello'",
        "TRUE",
        "FALSE",
        "NIL",
        "[ 1 2 ] [ 3 4 ] ADD",
    ] {
        assert_round_trips(program).await;
    }
}

/// An irrational displays its exact normal form as one token, so a Vector of
/// them still reads element by element; the display is not source, and
/// reading it back is a call of an unknown Word.
#[tokio::test]
async fn an_irrational_displays_but_is_not_source() {
    for (program, expected) in [
        ("2 SQRT", "sqrt(2)"),
        ("1 2 SQRT ADD", "1/1+sqrt(2)"),
        ("2 SQRT 3 SQRT SUB", "sqrt(2)-sqrt(3)"),
        ("2 SQRT 3 2 COLLECT", "[ sqrt(2) 3/1 ]"),
    ] {
        assert_eq!(run(program).await, [expected]);
    }
    let mut interpreter = Interpreter::new();
    assert!(interpreter.execute("sqrt(2)").await.is_err());
}

/// A Record renders as the phrase that builds it, and that phrase is source:
/// running the display leaves the same Record. A Vector holding a Record
/// renders as a `COLLECT` phrase for the same reason, and a Symbol inside
/// that phrase is read out of a literal rather than called.
#[tokio::test]
async fn a_record_renders_as_the_phrase_that_builds_it() {
    for program in [
        "[ 'x' 'y' ] [ 1 2 ] RECORD",
        "[ ] [ ] RECORD",
        "[ 1 TRUE ] [ 'one' 'yes' ] RECORD",
        "[ 'a' ] [ 1 ] RECORD 1 COLLECT",
        "1 [ 'a' ] [ 2 ] RECORD 2 COLLECT",
        "[ 'a' ] [ 1 ] RECORD 1 COLLECT 1 COLLECT",
        "[ 'v' 'r' ] [ 1 2 ] [ 'k' ] [ 3 ] RECORD 2 COLLECT RECORD",
        "[ 'k' ] [ 1 ] RECORD [ V ] 0 GET 2 COLLECT",
    ] {
        assert_round_trips(program).await;
    }
    assert_eq!(
        run("[ 'x' 'y' ] [ 1 2 ] RECORD").await,
        ["[ 'x' 'y' ] [ 1/1 2/1 ] RECORD"]
    );
    assert_eq!(run("[ ] [ ] RECORD").await, ["[ ] [ ] RECORD"]);
    assert_eq!(
        run("[ 'a' ] [ 1 ] RECORD 1 COLLECT").await,
        ["[ 'a' ] [ 1/1 ] RECORD 1 COLLECT"]
    );
    // `{` is an ordinary name, so the old `{ key value }` spelling is a call
    // of an unknown Word, not a Record.
    let mut interpreter = Interpreter::new();
    assert!(interpreter.execute("{ 'x' 1/1 }").await.is_err());
}

/// A quote right before whitespace closes a String literal, so no literal
/// holds that pair: `'a'' b'` reads as the String `a'` and the name `b'`. Such
/// a String renders as the phrase that builds it, cut after each such quote,
/// and a Vector or Record holding one renders as a phrase too, since inside
/// `[ ]` the phrase would be data.
#[tokio::test]
async fn a_string_no_literal_spells_renders_as_the_phrase_that_builds_it() {
    for program in [
        "[ 'a''' ' b' ] JOIN",
        "[ 'a''' ' ' ] JOIN",
        "[ 'x''' ' y''' ' z' ] JOIN",
        "[ 'k' ] [ 'a''' ' b' ] JOIN 2 COLLECT",
        "[ 'a''' ' b' ] JOIN 1 COLLECT [ 1 ] RECORD",
        "[ 1 ] [ 'a''' ' b' ] JOIN 1 COLLECT RECORD",
    ] {
        assert_round_trips(program).await;
    }
    assert_eq!(run("[ 'a''' ' b' ] JOIN").await, ["[ 'a''' ' b' ] JOIN"]);
    // A quote that is not followed by whitespace is content, and the literal
    // still spells it.
    assert_eq!(run("'It's fine'").await, ["'It's fine'"]);
}
