//! `RECONCILE` as the join of the information order (LANG.VALUES.TRUTH),
//! and the laws that make it safe to fold; and how the three truth values
//! meet it and `SELECT` and `FILTER`. The strong Kleene tables for
//! `AND`/`NOT` are `kleene_truth_conformance_tests`.

use crate::error::NilReason;
use crate::test_support::run_ok;
use crate::types::Value;

async fn one(code: &str) -> Value {
    let stack = run_ok(code).await;
    assert_eq!(stack.len(), 1, "`{code}` must leave exactly one value");
    stack[0].clone()
}

#[tokio::test]
async fn reconcile_agrees_yields_and_conflicts() {
    for (code, expected) in [
        ("1 1 RECONCILE", "1/1"),
        ("NIL 5 RECONCILE", "5/1"),
        ("5 NIL RECONCILE", "5/1"),
        ("TRUE TRUE RECONCILE", "TRUE"),
        ("NIL FALSE RECONCILE", "FALSE"),
        ("[ 1 2 ] [ 1 2 ] RECONCILE", "[ 1/1 2/1 ]"),
        ("0/0 5 RECONCILE", "5/1"),
        ("-1/0 0/0 RECONCILE", "1/0"),
        ("0/0 0/0 RECONCILE", "0/0"),
        ("NIL 0/0 RECONCILE", "0/0"),
        ("0/0 NIL RECONCILE", "0/0"),
    ] {
        assert_eq!(one(code).await.to_string(), expected, "`{code}`");
    }
    for code in [
        "'Tokyo' 'Osaka' RECONCILE",
        "1 2 RECONCILE",
        "TRUE 1 RECONCILE",
        "TRUE FALSE RECONCILE",
        "[ 1 2 ] [ 1 3 ] RECONCILE",
        "1 2 RECONCILE 1 RECONCILE",
        "1 2 RECONCILE NIL RECONCILE",
        "NIL 1 2 RECONCILE RECONCILE",
        "0/0 TRUE RECONCILE",
        "'a' 0/0 RECONCILE",
        "0/0 1 2 RECONCILE RECONCILE",
    ] {
        assert_eq!(
            one(code).await.nil_reason(),
            Some(&NilReason::Conflict),
            "`{code}` is a conflict"
        );
    }
    assert_eq!(
        one("-1 SQRT NIL RECONCILE").await.nil_reason(),
        Some(&NilReason::DomainMiss),
        "two absences: the left one, reason intact"
    );
    assert_eq!(
        one("'Tokyo' 'Osaka' RECONCILE NIL-REASON")
            .await
            .to_string(),
        "'conflict'"
    );
}

/// `RECONCILE` is a semilattice up to which absence is kept: commutative,
/// associative and idempotent over a sample of every kind of operand, so a
/// FOLD of it over many sources does not depend on their order.
#[tokio::test]
async fn reconcile_is_a_semilattice() {
    let sample = [
        "NIL",
        "-1 SQRT",
        "TRUE",
        "FALSE",
        "TRUE FALSE RECONCILE",
        "1",
        "2",
        "'a'",
        "1 2 RECONCILE",
        "0/0",
        "1/0",
        "2 SQRT",
    ];
    let present = |v: &Value| (v.is_nil(), v.nil_reason().copied(), v.to_string());
    let absent_reason = |v: &Value| v.is_nil() && v.nil_reason() != Some(&NilReason::Conflict);
    for a in sample {
        let aa = one(&format!("{a} {a} RECONCILE")).await;
        assert_eq!(present(&aa), present(&one(a).await), "`{a}` is idempotent");
        for b in sample {
            let ab = one(&format!("{a} {b} RECONCILE")).await;
            let ba = one(&format!("{b} {a} RECONCILE")).await;
            if !(absent_reason(&ab) && absent_reason(&ba)) {
                assert_eq!(present(&ab), present(&ba), "`{a}` and `{b}` commute");
            }
            for c in sample {
                let left = one(&format!("{a} {b} RECONCILE {c} RECONCILE")).await;
                let right = one(&format!("{a} {b} {c} RECONCILE RECONCILE")).await;
                assert_eq!(
                    present(&left),
                    present(&right),
                    "`{a}`, `{b}`, `{c}` associate"
                );
            }
        }
    }
}

/// Two sources that disagree on a truth value are a conflict like any other
/// disagreement, and the reason says so; an UNKNOWN chooses neither branch,
/// and `FILTER` keeps only what its predicate calls TRUE, as SQL's `WHERE`
/// does.
#[tokio::test]
async fn a_truth_disagreement_is_a_conflict_and_unknown_selects_nothing() {
    for code in ["TRUE FALSE RECONCILE", "FALSE TRUE RECONCILE"] {
        assert_eq!(
            one(code).await.nil_reason(),
            Some(&NilReason::Conflict),
            "`{code}`"
        );
    }
    assert_eq!(
        one("TRUE FALSE RECONCILE 'M' BIND 1 2 M SELECT NIL-REASON")
            .await
            .to_string(),
        "'conflict'",
        "the conflict reads as UNKNOWN in truth position, reason kept"
    );
    assert_eq!(
        one("[ 1 0/0 2 ] [ 0 GT ] FILTER").await.to_string(),
        "[ 1/1 2/1 ]"
    );
    assert_eq!(
        one("[ TRUE FALSE NIL ] [ ] FILTER").await.to_string(),
        "[ TRUE ]"
    );
}
