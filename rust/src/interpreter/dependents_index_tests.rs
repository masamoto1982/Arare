//! Tests for the reverse-dependency index reads (`collect_dependents` /
//! `collect_transitive_dependents`).
//!
//! `collect_dependents` reads the maintained `dependents` inverted index
//! instead of rescanning every definition. Its body carries a
//! `debug_assert_eq!` that cross-checks the index against the authoritative
//! full scan on every call; because the test suite runs with debug assertions
//! enabled, every `collect_dependents` call in these scenarios — and in the
//! rest of the suite — also verifies that the maintained index has not drifted
//! from ground truth. The assertions below additionally pin the *values*
//! returned across DEF / redefine / DEL sequences.

use crate::interpreter::Interpreter;
use std::collections::HashSet;

fn set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// A word that references another user word records a direct dependency, so
/// the referenced word's `collect_dependents` reports the referrer.
#[tokio::test]
async fn direct_dependent_is_reported() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ] 'A' DEF").await.unwrap();
    interp.execute("[ A ] 'B' DEF").await.unwrap();

    assert_eq!(
        interp.collect_dependents("A"),
        set(&["B"]),
        "B references A, so A's dependents must be {{B}}"
    );
    assert!(
        interp.collect_dependents("B").is_empty(),
        "nothing references B, so B has no dependents"
    );
}

/// `collect_transitive_dependents` walks the whole reverse chain, while
/// `collect_dependents` reports only the direct referrers.
#[tokio::test]
async fn transitive_chain_is_followed() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ] 'A' DEF").await.unwrap();
    interp.execute("[ A ] 'B' DEF").await.unwrap();
    interp.execute("[ B ] 'C' DEF").await.unwrap();

    assert_eq!(
        interp.collect_dependents("A"),
        set(&["B"]),
        "direct dependents of A is just B"
    );
    assert_eq!(
        interp.collect_transitive_dependents("A"),
        set(&["B", "C"]),
        "transitive dependents of A reach C through B"
    );
    assert_eq!(
        interp.collect_transitive_dependents("B"),
        set(&["C"]),
        "transitive dependents of B is just C"
    );
    assert!(
        interp.collect_transitive_dependents("C").is_empty(),
        "C is a leaf in the dependency chain"
    );
}

/// A word may be referenced by several others; all of them appear.
#[tokio::test]
async fn multiple_direct_dependents() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ] 'A' DEF").await.unwrap();
    interp.execute("[ A ] 'B' DEF").await.unwrap();
    interp.execute("[ A ] 'C' DEF").await.unwrap();

    assert_eq!(
        interp.collect_dependents("A"),
        set(&["B", "C"]),
        "both B and C reference A"
    );
}

/// Redefining a word so it no longer references its former dependency drops
/// the reverse edge. The `debug_assert_eq!` inside `collect_dependents`
/// guarantees the maintained index still matches a full scan after the
/// redefinition's incremental edge removal.
#[tokio::test]
async fn redefine_drops_stale_reverse_edge() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ] 'A' DEF").await.unwrap();
    interp.execute("[ A ] 'B' DEF").await.unwrap();
    assert_eq!(interp.collect_dependents("A"), set(&["B"]));

    // B no longer references A. B has no dependents, so no force is needed.
    interp.execute("[ [ 2 ] ] 'B' DEF").await.unwrap();

    assert!(
        interp.collect_dependents("A").is_empty(),
        "after redefining B without A, A must have no dependents"
    );
}

/// Deleting a referrer removes it from the referenced word's dependents.
#[tokio::test]
async fn delete_referrer_clears_reverse_edge() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ] 'A' DEF").await.unwrap();
    interp.execute("[ A ] 'B' DEF").await.unwrap();
    assert_eq!(interp.collect_dependents("A"), set(&["B"]));

    // B is a leaf (nothing depends on it), so a plain DEL is allowed.
    interp.execute("'B' DEL").await.unwrap();

    assert!(
        interp.collect_dependents("A").is_empty(),
        "after deleting B, A must have no dependents"
    );
}

/// A word with no dependents reports the empty set (index miss path), which
/// must agree with the full scan via the in-call debug assertion.
#[tokio::test]
async fn unknown_word_has_no_dependents() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ] 'A' DEF").await.unwrap();

    assert!(
        interp.collect_dependents("NOPE").is_empty(),
        "a name nothing references has no dependents"
    );
    assert!(
        interp.collect_transitive_dependents("NOPE").is_empty(),
        "an unreferenced name has no transitive dependents"
    );
}

/// A body naming a word defined later depends on it from the moment it is
/// defined: the name resolves when the body runs, so the edge is real
/// whichever was written first.
#[tokio::test]
async fn forward_reference_becomes_an_edge_when_its_target_is_defined() {
    let mut interp = Interpreter::new();
    interp.execute("[ B 1 ADD ] 'A' DEF").await.unwrap();
    assert!(
        interp.collect_dependents("B").is_empty(),
        "B does not exist yet"
    );

    interp.execute("[ 1 ] 'B' DEF").await.unwrap();
    assert_eq!(interp.collect_dependents("B"), set(&["A"]));

    assert!(
        interp.execute("'B' DEL").await.is_err(),
        "A calls B, so B is not deletable"
    );
    interp.execute("'A' DEL").await.unwrap();
    assert!(interp.collect_dependents("B").is_empty());
    interp.execute("'B' DEL").await.unwrap();
}

/// A refused redefinition changes nothing: the old body stays, and so do
/// its edges. The old order dropped the edges before the checks that could
/// still refuse, so a refused `[ A B ] 'A' DEF` left B deletable while A
/// still called it.
#[tokio::test]
async fn a_refused_redefinition_leaves_the_index_intact() {
    let mut interp = Interpreter::new();
    interp.execute("[ 1 ] 'B' DEF").await.unwrap();
    interp.execute("[ B ] 'A' DEF").await.unwrap();

    assert!(
        interp.execute("[ A B ] 'A' DEF").await.is_err(),
        "the body names itself"
    );
    assert_eq!(interp.collect_dependents("B"), set(&["A"]));
    assert!(interp.execute("'B' DEL").await.is_err(), "A still calls B");

    assert!(
        interp.execute("[ ] 'A' DEF").await.is_err(),
        "an empty body is refused"
    );
    assert_eq!(interp.collect_dependents("B"), set(&["A"]));
    assert!(interp.execute("'B' DEL").await.is_err(), "A still calls B");

    interp.execute("A").await.unwrap();
    assert_eq!(
        format!("{}", interp.get_stack().last().expect("a result")),
        "1/1"
    );
}

/// The refusal names the locking words in one order every time.
#[tokio::test]
async fn a_refusal_names_the_dependents_in_sorted_order() {
    let mut interp = Interpreter::new();
    interp.execute("[ 1 ] 'W' DEF").await.unwrap();
    for caller in ["Q", "B", "M", "A"] {
        interp
            .execute(&format!("[ W ] '{caller}' DEF"))
            .await
            .unwrap();
    }
    let message = interp
        .execute("'W' DEL")
        .await
        .expect_err("W is locked")
        .to_string();
    assert!(message.contains("referenced by A, B, M, Q"), "{message}");
    let message = interp
        .execute("[ 2 ] 'W' DEF")
        .await
        .expect_err("W is locked")
        .to_string();
    assert!(message.contains("referenced by A, B, M, Q"), "{message}");
}

/// A literal captured as the body of a `DEF` that never ran is not the
/// body of the next run's `DEF`. The capture is taken when the literal is
/// built; if the `DEF` dispatch then fails before `op_def` takes it — here
/// at the step ceiling — the next run starts clean.
#[tokio::test]
async fn a_failed_def_dispatch_leaves_no_body_for_the_next_run() {
    let mut interp = Interpreter::new();
    interp.set_max_execution_steps(0);
    assert!(
        interp.execute("[ 1 2 3 ] 'X' DEF").await.is_err(),
        "the DEF dispatch is refused at the step ceiling"
    );
    interp.set_max_execution_steps(1_000);
    // The refused run left its operands on the stack; only the next run's
    // result is of interest.
    interp.stack.clear();

    // A computed one-element Vector: the body is `5`, and Y pushes it.
    // With the stale capture the body would be `1 2 3`.
    interp.execute("5 1 COLLECT 'Y' DEF Y").await.unwrap();
    let stack: Vec<String> = interp.get_stack().iter().map(|v| format!("{v}")).collect();
    assert_eq!(
        stack,
        vec!["5/1".to_string()],
        "Y is the computed Vector's body, not the literal a failed DEF captured"
    );
}

// The refusal names the shortest chain that closes, start to start.
#[tokio::test]
async fn a_reference_cycle_is_reported_as_its_closing_chain() {
    for (source, chain) in [
        ("[ X ] 'X' DEF", "(X -> X)"),
        (
            "[ B ] 'A' DEF [ C ] 'B' DEF [ A ] 'C' DEF",
            "(C -> A -> B -> C)",
        ),
        ("[ B C ] 'A' DEF [ A ] 'C' DEF", "(C -> A -> C)"),
    ] {
        let mut interp = Interpreter::new();
        let err = interp
            .execute(source)
            .await
            .expect_err("a cycle is refused");
        assert!(err.to_string().contains(chain), "{source}: {err}");
    }
}
