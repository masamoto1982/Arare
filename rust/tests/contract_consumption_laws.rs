//! Property-based contract / consumption / mass-conservation laws (Phase 3 ⭐).
//!
//! Encodes the algebraic content of the consumption / contract /
//! mass-conservation model (Phase 3):
//!
//! 1. **Consumption** (`LANG.STACK.CONSUMPTION`): every Word consumes the
//!    operands it reads; a value is reused only by naming it with `BIND`.
//! 2. **Coreword contracts** (`LANG.CONTRACT.REGISTRY`): the `partiality` /
//!    `nil_policy` / purity-and-effects declarations, with contract absence =
//!    conformance violation.
//! 3. **Static mass conservation** (`LANG.STACK.CONSUMPTION`): consumption/production as a
//!    resource (linear) discipline, observed here via stack-depth deltas.
//!
//! Every law was checked against the reference implementation with a throwaway
//! probe before being written (roadmap §1.2-(T) discipline). Probe findings are
//! recorded as §9-quater E.5 findings; the two that are tracked oracles
//! are asserted as guarded
//! invariants so a future drift is loud.

mod test_support;

use ajisai_core::coreword_registry::{
    get_builtin_word_registry, get_coreword_metadata, NilPolicy, Partiality, Purity,
};
use proptest::prelude::*;
use test_support::generators::small;
use test_support::observe::{render, run};

// ─────────────────────────── observation helpers ───────────────────────────

/// Whole-stack rendering (one value per element), the conformance observation.
fn obs(src: &str) -> Vec<String> {
    run(src).iter().map(render).collect()
}

/// Stack depth after running `src` (mass observation).
fn depth(src: &str) -> usize {
    run(src).len()
}

/// Total binary scalar→scalar words (never error / NIL on integer operands).
fn binary_arith() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("ADD"), Just("MUL"), Just("SUB")]
}

// ───────────────────────── consumption (§6, LANG.STACK.CONSUMPTION) ─────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// **Mass conservation** (LANG.MACHINE.WORD/LANG.STACK.CONSUMPTION): a binary word consumes
    /// both operands it reads and produces one result, so a value beneath its
    /// operands is untouched and the depth drops by exactly one.
    #[test]
    fn binary_words_consume_both_operands(a in small(), b in small(), w in binary_arith()) {
        let below = obs(&format!("9 {a} {b} {w}"));
        prop_assert_eq!(below.len(), 2);
        prop_assert_eq!(&below[0], "9/1");
    }

    // ──────────── partiality contract ↔ observable behavior (LANG.CONTRACT.REGISTRY) ──────────

    /// A `Total` word never errors on well-shaped input: it always leaves a
    /// value (Hoare `ensures` discharged), here over total binary arithmetic.
    #[test]
    fn total_words_do_not_error(a in small(), b in small(), w in binary_arith()) {
        prop_assert_eq!(depth(&format!("{a} {b} {w}")), 1);
    }
}

// ─────────────────── consumption across abstraction (LANG.STACK.CONSUMPTION) ──────────────────

/// A User Word call consumes its operands like a Core Word; reusing a value is
/// done by naming it with `BIND`.
#[test]
fn a_user_word_call_consumes_its_operands() {
    assert_eq!(obs("[ 2 MUL ] 'TWICE' DEF 5 TWICE"), vec!["10/1"]);
    assert_eq!(obs("[ ADD ] 'PLUS' DEF 3 5 PLUS"), vec!["8/1"]);
    assert_eq!(
        obs("[ 2 MUL ] 'TWICE' DEF 5 'N' BIND N N TWICE"),
        vec!["5/1", "10/1"]
    );
}

// ───────────────── projecting words project onto NIL for domain misses ──────

/// `Projecting`/`CreatesNil` words project a well-formed domain miss onto NIL
/// rather than raising (LANG.CONTRACT.REGISTRY, NIL Projection Rule LANG.FAILURE.PROJECT): a
/// negative radicand and an out-of-range `GET` both yield NIL, not an error.
/// Division by zero is total and yields a number (LANG.VALUES.EXACT).
#[test]
fn projecting_words_project_onto_nil_for_domain_misses() {
    assert_eq!(obs("-1 SQRT"), vec!["NIL"]);
    assert_eq!(obs("1 0 DIV"), vec!["1/0"]);
    // GET consumes what it reads (LANG.STACK.CONSUMPTION): both
    // operands leave the stack and the projected NIL is all that remains.
    assert_eq!(obs("[ 1 2 3 ] 9 GET"), vec!["NIL"]);
}

// ──────────────────────── contract lattice laws (LANG.CONTRACT.REGISTRY) ─────────────────────

/// Every built-in carries a contract reachable by its own name, with all three
/// classification fields in their declared domains. A Coreword without a
/// contract entry is a conformance violation (LANG.CONTRACT.REGISTRY).
#[test]
fn every_coreword_declares_a_reachable_contract() {
    let reg = get_builtin_word_registry();
    assert!(!reg.is_empty());
    for m in reg {
        assert!(
            get_coreword_metadata(&m.name).is_some(),
            "{} has no reachable contract",
            m.name
        );
        assert!(matches!(
            m.partiality,
            Partiality::Total | Partiality::Partial | Partiality::Projecting
        ));
        // The NIL policy is no longer checked against a hand-written list of
        // admissible values: it is generated from the schema's own enum, so
        // every value the specification admits is a variant and no other value
        // is representable. What is worth asserting is that the declaration
        // reaches the runtime at all.
        assert!(
            !m.nil_policy.as_spec_str().is_empty(),
            "{} declares no NIL policy",
            m.name
        );
    }
}

/// A Word declares effects exactly when it is `effectful`: purity and the
/// effect list are two declarations of one fact, so neither may hold without
/// the other. (A derived `A`/`B`/`D` "safety level" used to restate them a
/// third time; it was a label no specification declared, and it is gone.)
#[test]
fn effects_are_declared_exactly_by_effectful_words() {
    for m in get_builtin_word_registry() {
        assert_eq!(
            m.purity == Purity::Effectful,
            !m.effects.is_empty(),
            "{}: purity {:?} with effects {:?}",
            m.name,
            m.purity,
            m.effects
        );
    }
}

/// Concrete LANG.CONTRACT.REGISTRY anchor contracts (the narrative examples of LANG.CONTRACT.REGISTRY, pinned as
/// machine-checked facts).
#[test]
fn key_word_contracts_match_spec_7_14() {
    let c = |n: &str| get_coreword_metadata(n).unwrap_or_else(|| panic!("no contract {n}"));

    let add = c("ADD");
    assert_eq!(add.partiality, Partiality::Total);
    assert_eq!(add.nil_policy, NilPolicy::Passthrough);

    // DIV is total (LANG.VALUES.EXACT): a quotient by zero is a point over
    // zero, a number, so there is nothing to project. A NIL operand
    // passes through unchanged, exactly as it does for ADD.
    let div = c("DIV");
    assert_eq!(div.partiality, Partiality::Total);
    assert_eq!(div.nil_policy, NilPolicy::Passthrough);

    // SQRT declares `passthroughThenProject`, not `createsNil`: a NIL operand
    // passes through unchanged, and it is a *well-formed* negative radicand
    // that projects onto a fresh reasoned NIL. The hand-written table could
    // not say both, so it said only the second.
    let sqrt = c("SQRT");
    assert_eq!(sqrt.partiality, Partiality::Projecting);
    assert_eq!(sqrt.nil_policy, NilPolicy::PassthroughThenProject);

    // EQ declares a blanket `passthrough` and is total: equality decides over
    // every number the language holds (LANG.VALUES.EXACT). LT does not: `0/0`
    // has no place in the order, so LT passes a NIL through and projects
    // `domainMiss` on a well-formed operand it cannot order.
    let eq = c("EQ");
    assert_eq!(eq.partiality, Partiality::Total);
    assert_eq!(eq.nil_policy, NilPolicy::Passthrough);
    let lt = c("LT");
    assert_eq!(lt.partiality, Partiality::Projecting);
    assert_eq!(lt.nil_policy, NilPolicy::PassthroughThenProject);

    // `AND`/`NOT` declare `kleeneAbsorbing`, not a blanket `passthrough`:
    // a NIL operand does not always survive to the result (FALSE absorbs it
    // into `AND`), so the primitive must decide instead of a
    // generic projection (LANG.VALUES.TRUTH).
    for logic in ["AND", "NOT"] {
        let m = c(logic);
        assert_eq!(m.partiality, Partiality::Total, "{logic}");
        assert_eq!(m.nil_policy, NilPolicy::KleeneAbsorbing, "{logic}");
    }
}
