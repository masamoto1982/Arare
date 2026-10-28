//! The text Words are charged for the text they build, before they build it.
//!
//! A String is one leaf to the collection meter's element price however long
//! it is, and `JOIN`, `REPLACE`, `TRIM`, `UPPER`, `LOWER`, `CHARS` and
//! `TOKENIZE` used to charge nothing. So text was the one thing a program
//! could grow with no ceiling seeing it: thirty doublings of `'ab'` asked for
//! 2 GiB from a hundred bytes of source and a hundred steps, and the run ended
//! in an out-of-memory trap — on the playground, one that also left its
//! WebAssembly instance unusable — instead of a refusal that names a ceiling.

use crate::error::{AjisaiError, ResourceLimit};
use crate::interpreter::runtime_limits::RuntimeLimits;
use crate::interpreter::Interpreter;

/// `'ab'` joined to itself once per element of `1 n RANGE`: 2^(n+1) bytes.
fn doubling(n: u32) -> String {
    format!("1 {n} RANGE 'ab' [ 'E' BIND 'A' BIND A A 2 COLLECT JOIN ] FOLD")
}

/// Collection work charged by `word` alone, with `setup` left on the stack
/// beforehand (`execute` keeps the stack and resets the counters).
async fn charged_by_word(setup: &str, word: &str) -> u64 {
    let mut interp = Interpreter::new();
    interp
        .execute(setup)
        .await
        .unwrap_or_else(|e| panic!("setup `{setup}` must compute, got: {e:?}"));
    interp
        .execute(word)
        .await
        .unwrap_or_else(|e| panic!("`{word}` must compute, got: {e:?}"));
    interp.collection_work_used()
}

/// The ceiling that refused the Word `word` run over the operands `setup`
/// leaves, under `limits`, with the stack checked to be exactly what `setup`
/// left.
async fn refused_in_place(setup: &str, word: &str, limits: RuntimeLimits) -> ResourceLimit {
    let mut interp = Interpreter::new();
    interp
        .execute(setup)
        .await
        .unwrap_or_else(|e| panic!("setup `{setup}` must compute, got: {e:?}"));
    let before: Vec<_> = interp.get_stack().iter().cloned().collect();
    interp.set_runtime_limits(limits);
    let err = interp
        .execute(word)
        .await
        .expect_err("the Word must be refused");
    assert_eq!(
        interp.get_stack().iter().cloned().collect::<Vec<_>>(),
        before,
        "a refused `{word}` must leave its operands where the program put them"
    );
    match err {
        AjisaiError::ResourceLimitExceeded { resource, .. } => resource,
        other => panic!("`{word}` must hit a ceiling, got: {other:?}"),
    }
}

fn collection_budget(units: u64) -> RuntimeLimits {
    RuntimeLimits {
        max_collection_work: units,
        ..RuntimeLimits::default()
    }
}

#[tokio::test]
async fn doubling_text_is_refused_by_the_collection_budget() {
    // The agent profile's 20,000,000 units admit about 20 MB of text; the
    // doubling crosses them at 2^25 bytes and is refused, long before memory
    // is at risk.
    let mut interp = Interpreter::new();
    interp.set_runtime_limits(collection_budget(20_000_000));
    let err = interp
        .execute(&doubling(30))
        .await
        .expect_err("2 GiB of text must be refused");
    assert!(
        matches!(
            err,
            AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::CollectionWork,
                ..
            }
        ),
        "got: {err:?}"
    );
}

#[tokio::test]
async fn expanding_replacement_is_refused_by_the_collection_budget() {
    // Each REPLACE makes the text eight times longer: 8^12 bytes asked for.
    let mut interp = Interpreter::new();
    interp.set_runtime_limits(collection_budget(20_000_000));
    let err = interp
        .execute("1 12 RANGE 'a' [ 'E' BIND 'a' 'aaaaaaaa' REPLACE ] FOLD")
        .await
        .expect_err("68 GB of text must be refused");
    assert!(
        matches!(
            err,
            AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::CollectionWork,
                ..
            }
        ),
        "got: {err:?}"
    );
}

#[tokio::test]
async fn a_text_word_is_charged_the_bytes_it_builds() {
    // One unit per byte built: what each result holds, not what was read.
    assert_eq!(charged_by_word("[ 'abc' 'de' ]", "JOIN").await, 5);
    assert_eq!(charged_by_word("'  abc  '", "TRIM").await, 3);
    assert_eq!(charged_by_word("'abc'", "UPPER").await, 3);
    assert_eq!(charged_by_word("'ABC'", "LOWER").await, 3);
    assert_eq!(charged_by_word("'aXa'", "'X' 'yyyy' REPLACE").await, 6);
    assert_eq!(charged_by_word("'aXa'", "'Z' 'yyyy' REPLACE").await, 3);
    // A case mapping can lengthen text: `ΐ` (2 bytes) upper-cases to three
    // characters (6 bytes), and the charge is for those six.
    assert_eq!(charged_by_word("'ΐ'", "UPPER").await, 6);
}

#[tokio::test]
async fn cutting_text_into_parts_is_charged_as_materializing_them() {
    // Each part is a fresh element, priced as `RANGE` prices one, plus the
    // text the parts copy — so cutting costs more than the bytes alone.
    assert!(charged_by_word("'abcd'", "CHARS").await > 4);
    assert!(charged_by_word("'a,b'", "',' TOKENIZE").await > 2);
    assert!(
        charged_by_word("'abcdefgh'", "CHARS").await > charged_by_word("'abcd'", "CHARS").await,
        "more parts must cost more"
    );
}

#[tokio::test]
async fn cutting_more_parts_than_the_host_materializes_is_refused_in_place() {
    let limits = RuntimeLimits {
        max_materialized_elements: 3,
        ..RuntimeLimits::default()
    };
    for (setup, word) in [
        ("'abcd'", "CHARS"),
        ("'a,b,c,d' ','", "TOKENIZE"),
        ("'abcd' ''", "TOKENIZE"),
    ] {
        assert_eq!(
            refused_in_place(setup, word, limits).await,
            ResourceLimit::MaterializedElements,
            "`{setup} {word}`"
        );
    }
    // At the ceiling, not past it, the parts are built.
    let mut interp = Interpreter::new();
    interp.set_runtime_limits(limits);
    interp
        .execute("'abc' CHARS 'a,b,c' ',' TOKENIZE")
        .await
        .expect("three parts fit a ceiling of three");
}

#[tokio::test]
async fn a_refused_text_word_leaves_its_operands_in_place() {
    for (setup, word) in [
        ("[ 'abc' 'de' ]", "JOIN"),
        ("'  abc  '", "TRIM"),
        ("'abc'", "UPPER"),
        ("'ABC'", "LOWER"),
        ("'aXa' 'X' 'yyyy'", "REPLACE"),
        ("'abcd'", "CHARS"),
        ("'a,b' ','", "TOKENIZE"),
    ] {
        assert_eq!(
            refused_in_place(setup, word, collection_budget(2)).await,
            ResourceLimit::CollectionWork,
            "`{setup} {word}`"
        );
    }
}
