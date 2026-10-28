//! Property-based algebraic-law conformance.
//!
//! These tests encode the language's core algebraic laws as executable
//! properties: instead of enumerating finitely many input/output pairs (as the
//! HTML conformance suite does), each law asserts an equation that must hold for
//! *all* inputs in a generated sample. A law is the compressed form of
//! infinitely many conformance cases, so this file is the "equation-level
//! continuous verification" companion to `tests/conformance/`.
//!
//! Scope: the laws asserted here are consistent with `SPECIFICATION.html`. They
//! include the strong-Kleene K3 logic laws over {TRUE, FALSE, UNKNOWN}, which
//! pass now that truth values are a distinct data-plane kind rendering
//! uniformly as TRUE/FALSE/UNKNOWN (findings B1/B2) and irrationals render as
//! exact nested continued fractions (finding C).
//!
//! Observation: two programs are "equal" when their whole-stack rendering
//! (`Value::to_string`, the same surface the conformance runner observes) is
//! identical.

use ajisai_core::interpreter::Interpreter;
use proptest::prelude::*;

/// Run an Ajisai program and render the whole final stack value-by-value, the
/// same observation the conformance runner uses. Panics on execution error so a
/// malformed law program is loud rather than silently skipped.
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

/// Assert two Ajisai programs are observationally equal.
fn assert_law(name: &str, lhs: &str, rhs: &str) {
    let l = eval(lhs);
    let r = eval(rhs);
    assert_eq!(
        l, r,
        "law `{name}` broken:\n  {lhs:?} => {l}\n  {rhs:?} => {r}"
    );
}

// Small integer operands keep generated programs cheap while still exercising
// BigInt-backed exact arithmetic across sign and zero.
fn small() -> impl Strategy<Value = i64> {
    -50i64..=50
}
fn nonzero() -> impl Strategy<Value = i64> {
    (1i64..=50).prop_flat_map(|n| prop_oneof![Just(n), Just(-n)])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // ─────────────────── Monoid of state transformers (§2) ───────────────────

    /// The empty program is the identity of composition: `p ∘ ε ≡ p`. There is
    /// no IDLE Word; the identity is the empty token sequence itself.
    #[test]
    fn monoid_identity(a in small()) {
        assert_law("monoid-identity", &format!("{a} "), &format!("{a}"));
    }

    // ─────────────────── Exact-rational field laws (§3, 𝔸) ───────────────────

    #[test]
    fn add_commutative(a in small(), b in small()) {
        assert_law("add-comm", &format!("{a} {b} ADD"), &format!("{b} {a} ADD"));
    }

    #[test]
    fn add_associative(a in small(), b in small(), c in small()) {
        assert_law(
            "add-assoc",
            &format!("{a} {b} ADD {c} ADD"),
            &format!("{a} {b} {c} ADD ADD"),
        );
    }

    #[test]
    fn mul_commutative(a in small(), b in small()) {
        assert_law("mul-comm", &format!("{a} {b} MUL"), &format!("{b} {a} MUL"));
    }

    #[test]
    fn mul_associative(a in small(), b in small(), c in small()) {
        assert_law(
            "mul-assoc",
            &format!("{a} {b} MUL {c} MUL"),
            &format!("{a} {b} {c} MUL MUL"),
        );
    }

    /// Multiplication distributes over addition: `(a + b) · c = a·c + b·c`.
    #[test]
    fn mul_distributes_over_add(a in small(), b in small(), c in small()) {
        assert_law(
            "mul-distrib",
            &format!("{a} {b} ADD {c} MUL"),
            &format!("{a} {c} MUL {b} {c} MUL ADD"),
        );
    }

    /// Additive identity, multiplicative identity, and additive inverse.
    #[test]
    fn field_units_and_inverse(a in small()) {
        assert_law("add-ident", &format!("{a} 0 ADD"), &format!("{a}"));
        assert_law("mul-ident", &format!("{a} 1 MUL"), &format!("{a}"));
        assert_law("self-sub-zero", &format!("{a} {a} SUB"), "0");
    }

    /// Exact division round-trips: `(a / b) · b = a` for `b ≠ 0`. This is the
    /// rational-domain analogue of finding C's `x a ADD a SUB ≡ x`; it holds
    /// here because finite CFs decide and never approximate.
    #[test]
    fn div_mul_roundtrip(a in small(), b in nonzero()) {
        assert_law(
            "div-mul-roundtrip",
            &format!("{a} {b} DIV {b} MUL"),
            &format!("{a}"),
        );
    }

    /// Exact finite comparisons are dual observations over the shared exact-order primitive.
    #[test]
    fn comparison_dualities(a in small(), b in small()) {
        assert_law("lt-gt-dual", &format!("{a} {b} LT"), &format!("{b} {a} GT"));
        assert_law("not-gt-not-lt-dual", &format!("{a} {b} GT NOT"), &format!("{b} {a} LT NOT"));
    }

    // ─────────────────── NIL-projection monad (§5) ───────────────────

    /// NIL passthrough: any arithmetic on a domain-miss projection stays NIL.
    #[test]
    fn nil_passthrough(a in small()) {
        assert_law("nil-passthrough", &format!("-1 SQRT {a} ADD"), "NIL");
    }

    /// Division by zero is total, and the two points over zero obey the
    /// pair formulas (LANG.VALUES.EXACT): a quotient by zero is `1/0` for a
    /// nonzero dividend of either sign and `0/0` for zero, and arithmetic
    /// after it stays on the two points.
    #[test]
    fn division_by_zero_is_a_point_over_zero(a in small()) {
        let point = i64::from(a != 0);
        assert_law("div-by-zero", &format!("{a} 0 DIV"), &format!("{point}/0"));
        assert_law("div-by-zero-absorbs-add", &format!("{a} 0 DIV {a} ADD"), &format!("{point}/0"));
        assert_law("div-by-point-is-zero", &format!("{a} 1/0 DIV"), "0/1");
        assert_law("point-minus-itself", &format!("{a} 0 DIV {a} 0 DIV SUB"), "0/0");
        assert_law("nullity-absorbs", &format!("0/0 {a} ADD"), "0/0");
    }

    /// Absence handler: a projected NIL is replaced by the fallback, a present
    /// value is kept (LANG.FAILURE.RECOVERY). `NIL?` consumes its subject and
    /// answers whether it was absent, which is exactly `SELECT`'s truth
    /// operand, so the handler names the subject once and reads it twice:
    /// `subject 'S' BIND fallback S S NIL? SELECT`.
    #[test]
    fn absence_handler(a in small()) {
        assert_law("absence-recovers-projection", &format!("-1 SQRT 'S' BIND {a} S S NIL? SELECT"), &format!("{a}"));
        // A present value is its own result regardless of the fallback.
        assert_law("absence-present", &format!("{a} 'S' BIND 999 S S NIL? SELECT"), &format!("{a}"));
    }
}

/// Integer projections are exact-real observations, not float round trips.
#[test]
fn integer_projection_examples() {
    assert_law("floor-positive", "7 3 DIV FLOOR", "2");
    assert_law("floor-negative", "-7 3 DIV FLOOR", "-3");
    assert_law("floor-of-integer", "4 FLOOR", "4");
    assert_law("round-positive-half", "5 2 DIV ROUND", "3");
    assert_law("round-negative-half", "-5 2 DIV ROUND", "-3");
}

// ─────────────────── Strong Kleene three-valued logic K3 (§4) ───────────────────
//
// K3 laws are checked exhaustively over the truth domain {TRUE, FALSE, U}.
// `U` is a NIL read in truth position (LANG.VALUES.TRUTH): every comparison
// over the numbers decides, so the bare `NIL` is how a program writes it.
// Each law renders both sides through the identical path,
// so the equation is independent of how a truth value is displayed (finding B).

/// The three truth-domain generators as Ajisai source fragments.
fn truths() -> [(&'static str, &'static str); 3] {
    [("T", "TRUE"), ("F", "FALSE"), ("U", "NIL")]
}

#[test]
fn k3_double_negation() {
    for (name, t) in truths() {
        assert_law(&format!("double-neg[{name}]"), &format!("{t} NOT NOT"), t);
    }
}

#[test]
fn k3_and_commutative() {
    for (na, a) in truths() {
        for (nb, b) in truths() {
            assert_law(
                &format!("and-comm[{na},{nb}]"),
                &format!("{a} {b} AND"),
                &format!("{b} {a} AND"),
            );
        }
    }
}

#[test]
fn k3_associativity_and_idempotence() {
    let ts = truths();
    for (na, a) in ts {
        // Idempotence: a ∧ a = a.
        assert_law(&format!("and-idem[{na}]"), &format!("{a} {a} AND"), a);
        for (nb, b) in ts {
            for (nc, c) in ts {
                assert_law(
                    &format!("and-assoc[{na},{nb},{nc}]"),
                    &format!("{a} {b} AND {c} AND"),
                    &format!("{a} {b} {c} AND AND"),
                );
            }
        }
    }
}
