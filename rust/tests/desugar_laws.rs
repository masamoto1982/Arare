//! Syntax / desugar soundness as executable laws.
//!
//! Companion to `algebraic_laws.rs`: the surface
//! desugaring of LANG.SOURCE.DESUGAR / LANG.SOURCE.NORMALIZE is *observationally transparent*:
//! word names are case-normalized (LANG.SOURCE.NORMALIZE), and a symbol the
//! language has not allocated reaches the dictionary as an ordinary name. Each law is the compressed form
//! of infinitely many tokenizer conformance cases: if desugaring were not
//! `⟦desugar(s)⟧ = ⟦s⟧`, some generated pair would render differently.
//!
//! Observation is structured, not a display-string fragment: laws compare stack
//! renders plus semantic axes (including NIL/UNKNOWN absence diagnosis), effect
//! trace, and error category.

mod test_support;

use proptest::prelude::*;
use test_support::observe::{observe_program, ProgramObservation};

fn assert_law(name: &str, lhs: &str, rhs: &str) {
    let l = observe_program(lhs);
    let r = observe_program(rhs);
    assert_eq!(
        l, r,
        "law `{name}` broken:\n  {lhs:?} => {l:#?}\n  {rhs:?} => {r:#?}"
    );
}

fn observed(src: &str) -> ProgramObservation {
    observe_program(src)
}

fn small() -> impl Strategy<Value = i64> {
    -50i64..=50
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    // ── An unallocated symbol is not a silent no-op ──
    //
    // Desugaring is semantics-preserving (LANG.SOURCE.DESUGAR), which cuts both
    // ways: a symbol the language has not allocated must not quietly disappear
    // from a program, and neither must a retired spelling. Each of these reaches
    // the dictionary as an ordinary name and fails there: `~` was never
    // allocated, `&` no longer spells AND, `<>`, `,,`, `%`, `<=` and `>=`
    // are retired spellings (the last three went with MOD, LTE and GTE), and
    // `ADD SUB MUL DIV EQ LT GT` were the second spellings of ADD SUB MUL DIV EQ LT GT
    // until a Word was given exactly one name.
    #[test]
    fn an_unallocated_symbol_is_not_a_silent_noop(a in small(), b in small()) {
        for symbol in ["~", "&", "<>", ",,", "%", "<=", ">=", "+", "-", "*", "/", "=", "<", ">"] {
            let observation = observed(&format!("{a} {b} {symbol} ADD"));
            prop_assert_eq!(
                observation.error_category,
                Some("unknownWord"),
                "`{}` must reach the dictionary as a name",
                symbol
            );
        }
    }

    // ── Word-name case normalization (LANG.SOURCE.NORMALIZE): add ≡ Add ≡ ADD ──
    #[test]
    fn case_normalization(a in small(), b in small()) {
        assert_law("case-lower", &format!("{a} {b} add"), &format!("{a} {b} ADD"));
        assert_law("case-mixed", &format!("{a} {b} Add"), &format!("{a} {b} ADD"));
    }

}
