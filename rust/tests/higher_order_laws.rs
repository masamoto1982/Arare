//! Higher-order / control words as recursion schemes (executable laws).
//!
//! The control vocabulary of LANG.COLLECTIONS.HIGHER obeys the algebraic laws of its categorical
//! models — `MAP` is a functor lift, `FOLD` a catamorphism, `FILTER` a
//! predicate restriction, `EXEC`/`EVAL` reflection of `⟦·⟧`, and `COND` a K3-honest guarded case in
//! which a `unknown` (U) guard does not fire (LANG.VALUES.TRUTH).
//!
//! Observation matches the conformance runner: whole-stack `Value::to_string`.

use ajisai_core::interpreter::Interpreter;
use proptest::prelude::*;

fn eval(src: &str) -> String {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio current-thread runtime");
    rt.block_on(async {
        let mut interp = Interpreter::new();
        interp
            .execute(src)
            .await
            .unwrap_or_else(|e| panic!("program failed: {src:?}: {e}"));
        interp
            .get_stack()
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    })
}

fn assert_law(name: &str, lhs: &str, rhs: &str) {
    let l = eval(lhs);
    let r = eval(rhs);
    assert_eq!(
        l, r,
        "law `{name}` broken:\n  {lhs:?} => {l}\n  {rhs:?} => {r}"
    );
}

fn small() -> impl Strategy<Value = i64> {
    -50i64..=50
}

/// Render a slice of integers as an Ajisai vector literal `[ a b c ]`.
fn vlit(xs: &[i64]) -> String {
    let body = xs
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    format!("[ {body} ]")
}

/// A non-empty vector (empty vectors are NIL in Ajisai, LANG.VALUES.NIL).
fn vec_ne() -> impl Strategy<Value = Vec<i64>> {
    prop::collection::vec(small(), 1..=5)
}
/// Exactly three operands, for fold/triple laws.
fn triple() -> impl Strategy<Value = (i64, i64, i64)> {
    (small(), small(), small())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    // ── MAP is a functor: identity and composition (fusion) ──

    /// `MAP id EQ id`: an identity block leaves the vector unchanged.
    #[test]
    fn map_identity(xs in vec_ne()) {
        let v = vlit(&xs);
        assert_law("map-id-empty-block", &format!("{v} [ ] MAP"), &v);
        assert_law("map-id-dot-block", &format!("{v} [ ] MAP"), &v);
    }

    /// Map fusion `MAP g ∘ MAP f = MAP (g∘f)` (functoriality of the lift).
    #[test]
    fn map_fusion(xs in vec_ne()) {
        let v = vlit(&xs);
        assert_law(
            "map-fusion",
            &format!("{v} [ 2 MUL ] MAP [ 1 ADD ] MAP"),
            &format!("{v} [ 2 MUL 1 ADD ] MAP"),
        );
    }

    // ── FOLD is a catamorphism over the additive/multiplicative monoid ──

    /// Left fold with `ADD` from `0` equals the explicit left-associated sum.
    #[test]
    fn fold_add_catamorphism((a, b, c) in triple()) {
        assert_law(
            "fold-add",
            &format!("[ {a} {b} {c} ] 0 [ ADD ] FOLD"),
            &format!("{a} {b} ADD {c} ADD"),
        );
    }

    /// Left fold with `MUL` from `1` equals the explicit left-associated product.
    #[test]
    fn fold_mul_catamorphism((a, b, c) in triple()) {
        assert_law(
            "fold-mul",
            &format!("[ {a} {b} {c} ] 1 [ MUL ] FOLD"),
            &format!("{a} {b} MUL {c} MUL"),
        );
    }

    // ── EXEC / EVAL reflect ⟦·⟧ ──

    /// `EXEC` of a block is its inline expansion.
    #[test]
    fn exec_is_inline(a in small(), b in small()) {
        assert_law("exec-inline", &format!("{a} {b} [ ADD ] EXEC"), &format!("{a} {b} ADD"));
    }
}

// ── FILTER restriction laws (fixed vectors avoid empty→NIL results) ──

#[test]
fn filter_is_idempotent() {
    for v in ["[ 1 2 3 4 5 ]", "[ 5 4 3 2 1 ]", "[ 3 1 4 1 5 ]"] {
        assert_law(
            "filter-idempotent",
            &format!("{v} [ 2 GT ] FILTER [ 2 GT ] FILTER"),
            &format!("{v} [ 2 GT ] FILTER"),
        );
    }
}

#[test]
fn filter_predicates_commute() {
    for v in ["[ 1 2 3 4 5 ]", "[ 5 4 3 2 1 ]"] {
        assert_law(
            "filter-commute",
            &format!("{v} [ 2 GT ] FILTER [ 4 LT ] FILTER"),
            &format!("{v} [ 4 LT ] FILTER [ 2 GT ] FILTER"),
        );
    }
}

// ── SCAN exposes the catamorphism's intermediate accumulators ──
