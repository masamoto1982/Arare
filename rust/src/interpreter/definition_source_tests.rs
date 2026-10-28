//! A definition is kept as its source (LANG.DICTIONARY.MUTATION).
//!
//! A body built from a computed Vector can carry a value no source text
//! denotes. `DEF` writes it back as the source that builds it
//! (`value_as_code::value_elements_to_source_tokens`), so the body the
//! dictionary holds, shows and saves restores to the same Word, with the same
//! identity, in every session.

use crate::interpreter::Interpreter;

fn definition_text(interp: &Interpreter, name: &str) -> String {
    interp
        .lookup_word_definition_tokens(name)
        .expect("a User Word has a definition")
}

async fn restored_from_text(name: &str, text: &str) -> Interpreter {
    let mut fresh = Interpreter::new();
    let skipped = fresh
        .restore_user_word_definitions([(name.to_string(), text.to_string(), None)])
        .expect("a restore of source text");
    assert!(skipped.is_empty(), "{skipped:?}");
    fresh
}

#[tokio::test]
async fn a_record_carried_whole_is_kept_as_the_source_that_builds_it() {
    let mut interp = Interpreter::new();
    interp
        .execute("[ 'k' ] [ 5 ] RECORD 1 COLLECT [ 'k' GET ] CONCAT 'W' DEF W")
        .await
        .unwrap();
    assert_eq!(
        format!("{}", interp.get_stack().last().expect("a result")),
        "5/1"
    );
    let text = definition_text(&interp, "W");
    assert_eq!(text, "[ 'k' ] [ 5 ] RECORD 'k' GET");

    let mut fresh = restored_from_text("W", &text).await;
    fresh.execute("W").await.expect("the restored Word runs");
    assert_eq!(
        format!("{}", fresh.get_stack().last().expect("a result")),
        "5/1"
    );
    assert_eq!(
        interp.word_identity("W"),
        fresh.word_identity("W"),
        "the same source is the same Word in every session"
    );
}

#[tokio::test]
async fn an_irrational_carried_whole_is_kept_as_its_normal_form() {
    let mut interp = Interpreter::new();
    interp
        .execute("2 SQRT 3 ADD 1 2 DIV 3 SQRT MUL ADD 1 COLLECT 'S' DEF S")
        .await
        .unwrap();
    let shown = format!("{}", interp.get_stack().last().expect("a result"));
    let text = definition_text(&interp, "S");
    assert!(
        text.starts_with("0 ") && text.ends_with(" ADD") && text.contains("SQRT"),
        "{text}"
    );

    let mut fresh = restored_from_text("S", &text).await;
    fresh.execute("S").await.unwrap();
    assert_eq!(
        format!("{}", fresh.get_stack().last().expect("a result")),
        shown
    );
}

#[tokio::test]
async fn a_symbol_beside_a_record_stays_data() {
    let mut interp = Interpreter::new();
    interp
        .execute("[ 'k' ] [ 1 ] RECORD [ V ] 0 GET 2 COLLECT 1 COLLECT 'M' DEF M")
        .await
        .unwrap();
    assert_eq!(
        format!("{}", interp.get_stack().last().expect("a result")),
        "[ 'k' ] [ 1/1 ] RECORD [ V ] 0 GET 2 COLLECT"
    );
    let text = definition_text(&interp, "M");
    assert_eq!(text, "[ 'k' ] [ 1 ] RECORD [ V ] 0 GET 2 COLLECT");
    let mut fresh = restored_from_text("M", &text).await;
    fresh.execute("M").await.unwrap();
    assert_eq!(
        format!("{}", fresh.get_stack().last().expect("a result")),
        "[ 'k' ] [ 1/1 ] RECORD [ V ] 0 GET 2 COLLECT"
    );
}

#[tokio::test]
async fn a_nested_record_rebuilds_inside_out() {
    let mut interp = Interpreter::new();
    interp
        .execute("[ 'k' ] [ 'a' ] [ 1 ] RECORD 1 COLLECT RECORD 1 COLLECT 'N' DEF")
        .await
        .unwrap();
    assert_eq!(
        definition_text(&interp, "N"),
        "[ 'k' ] [ 'a' ] [ 1 ] RECORD 1 COLLECT RECORD"
    );
}

/// The `NIL` name denotes the literal NIL and no other, so a body holding
/// a NIL that carries a reason has no source and is refused; a literal
/// NIL is written as its name.
#[tokio::test]
async fn a_reasoned_nil_has_no_source_and_is_refused() {
    let mut interp = Interpreter::new();
    let error = interp
        .execute("-1 SQRT 1 COLLECT 'Z' DEF")
        .await
        .expect_err("no source denotes a reasoned NIL");
    assert!(
        matches!(
            crate::error::ErrorCategory::from_error(&error),
            Some(crate::error::ErrorCategory::Declared(
                "invalidDefinitionBody"
            ))
        ),
        "{error}"
    );
    assert!(error.to_string().contains("domainMiss"), "{error}");
    assert!(!interp.user_words.contains_key("Z"));

    interp.execute("NIL 1 COLLECT 'L' DEF").await.unwrap();
    assert_eq!(definition_text(&interp, "L"), "NIL");
}

/// A literal written inside a stream bridged from a Vector (`EXEC`) can
/// carry a value whole; `DEF` still writes it back as source, so the
/// Word is the same Word, with the same identity, as one defined from
/// the Vector directly.
#[tokio::test]
async fn a_value_carried_whole_through_exec_is_written_as_source() {
    let mut interp = Interpreter::new();
    interp
        .execute("[ 'k' ] [ 5 ] RECORD 1 COLLECT 1 COLLECT [ 'X' DEF ] CONCAT EXEC")
        .await
        .unwrap();
    assert_eq!(definition_text(&interp, "X"), "[ 'k' ] [ 5 ] RECORD");

    let mut direct = Interpreter::new();
    direct
        .execute("[ 'k' ] [ 5 ] RECORD 1 COLLECT 'X' DEF")
        .await
        .unwrap();
    assert_eq!(
        interp.word_identity("X"),
        direct.word_identity("X"),
        "the route to DEF is not part of the definition"
    );

    let mut fresh = restored_from_text("X", &definition_text(&interp, "X")).await;
    fresh.execute("X").await.expect("the restored Word runs");
    assert_eq!(
        format!("{}", fresh.get_stack().last().expect("a result")),
        "[ 'k' ] [ 5/1 ] RECORD"
    );
}

/// The refusal of a reasoned NIL holds on that route too.
#[tokio::test]
async fn a_reasoned_nil_reaching_def_through_exec_is_refused() {
    let mut interp = Interpreter::new();
    let error = interp
        .execute("-1 SQRT 1 COLLECT 1 COLLECT [ 'Z' DEF ] CONCAT EXEC")
        .await
        .expect_err("no source denotes a reasoned NIL");
    assert!(
        matches!(
            crate::error::ErrorCategory::from_error(&error),
            Some(crate::error::ErrorCategory::Declared(
                "invalidDefinitionBody"
            ))
        ),
        "{error}"
    );
    assert!(!interp.user_words.contains_key("Z"));
}

/// A quote closes a String literal only when whitespace follows it, so a
/// text holding a quote right before whitespace has no literal of its
/// own; it is written as the pieces that do, joined. A quote anywhere
/// else stays inside one literal.
#[tokio::test]
async fn a_text_no_literal_spells_is_joined_from_its_pieces() {
    let mut interp = Interpreter::new();
    interp
        .execute("'\"a\\u0027 b\"' JSON-DECODE 1 COLLECT 'Q' DEF Q")
        .await
        .unwrap();
    let shown = format!("{}", interp.get_stack().last().expect("a result"));
    // The display is source too, so it writes the same phrase the
    // definition does (`types/display.rs`).
    assert_eq!(shown, "[ 'a'' ' b' ] JOIN");
    let text = definition_text(&interp, "Q");
    assert_eq!(text, "[ 'a'' ' b' ] JOIN");

    let mut fresh = restored_from_text("Q", &text).await;
    fresh.execute("Q").await.expect("the restored Word runs");
    assert_eq!(
        format!("{}", fresh.get_stack().last().expect("a result")),
        shown
    );

    interp
        .execute("'\"a\\u0027\"' JSON-DECODE 1 COLLECT 'E' DEF '\"\\u0027 x\"' JSON-DECODE 1 COLLECT 'S' DEF")
        .await
        .unwrap();
    assert_eq!(definition_text(&interp, "E"), "'a''");
    assert_eq!(definition_text(&interp, "S"), "[ ''' ' x' ] JOIN");
    let mut fresh = restored_from_text("S", &definition_text(&interp, "S")).await;
    fresh.execute("S").await.expect("the restored Word runs");
    assert_eq!(
        format!("{}", fresh.get_stack().last().expect("a result")),
        "[ ''' ' x' ] JOIN"
    );
}

/// The source written for an irrational takes the root of its radicand,
/// and `SQRT` factors a radicand against the numeric-work ceiling. `MUL`
/// builds √(pq) from √p and √q without factoring, so `DEF` takes the
/// root itself before committing: under a ceiling that cannot factor
/// pq the definition is refused with both operands put back, and under
/// one that can it commits and the Word runs.
#[tokio::test]
async fn an_irrational_whose_radicand_the_ceiling_cannot_factor_is_refused() {
    let mut interp = Interpreter::new();
    interp
        .execute("1000000007 SQRT 1000000009 SQRT MUL 1 COLLECT")
        .await
        .unwrap();
    let affordable = interp.runtime_limits.max_numeric_work;

    interp.runtime_limits.max_numeric_work = 20_000;
    let error = interp
        .execute("'R' DEF")
        .await
        .expect_err("the ceiling cannot factor the product of two large primes");
    assert!(
        matches!(
            error,
            crate::error::AjisaiError::ResourceLimitExceeded {
                resource: crate::error::ResourceLimit::NumericWork,
                ..
            }
        ),
        "{error}"
    );
    assert!(!interp.user_words.contains_key("R"));
    assert_eq!(interp.get_stack().len(), 2, "both operands are put back");

    interp.runtime_limits.max_numeric_work = affordable;
    // Both operands are back on the stack, so `DEF` alone retries.
    interp.execute("DEF R").await.expect("affordable to factor");
    let text = definition_text(&interp, "R");
    assert!(text.contains("1000000016000000063 SQRT"), "{text}");
    let shown = format!("{}", interp.get_stack().last().expect("a result"));
    let mut fresh = restored_from_text("R", &text).await;
    fresh.execute("R").await.expect("the restored Word runs");
    assert_eq!(
        format!("{}", fresh.get_stack().last().expect("a result")),
        shown
    );
}

/// A body with no carried value is kept as it is.
#[tokio::test]
async fn a_literal_body_is_kept_as_written() {
    let mut interp = Interpreter::new();
    interp
        .execute("[ 1 [ 2 ] 'x' TRUE NIL ] 1 COLLECT 'U' DEF")
        .await
        .unwrap();
    assert_eq!(definition_text(&interp, "U"), "[ 1 [ 2 ] 'x' TRUE NIL ]");
}
