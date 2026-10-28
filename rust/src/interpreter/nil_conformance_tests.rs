//! Contract-driven NIL / NIL-Projection-Rule behavioral conformance (phase B).
//!
//! Earlier coverage asserted the *metadata* of the Coreword registry
//! (every word has a contract, fields are internally consistent). This
//! suite closes the complementary gap: it drives the interpreter and
//! asserts the *runtime* honors each word's declared `nil_policy` and the
//! NIL Projection Rule (LANG.FAILURE.PROJECT, LANG.FAILURE.PASSTHROUGH, LANG.FAILURE.PROJECT).
//!
//! The completeness tests are registry-driven: a newly added Core
//! passthrough / projecting word without a behavioral probe here fails.
//!
//! Trace: docs/quality/TRACEABILITY_MATRIX.md (NIL projection conformance).

use crate::coreword_registry::{get_builtin_word_registry, NilPolicy};
use crate::error::NilReason;
use crate::interpreter::Interpreter;
use crate::test_support::{reason, run_ok, top_of};
use crate::types::Value;

async fn run(code: &str) -> Result<Vec<Value>, String> {
    let mut interp = Interpreter::new();
    interp.execute(code).await.map_err(|e| e.to_string())?;
    Ok(interp.get_stack().to_vec())
}

fn is_nil(v: &Value) -> bool {
    v.is_nil()
}
fn reason_of(v: &Value) -> Option<NilReason> {
    v.nil_reason().cloned()
}

// --- Core passthrough classification (LANG.FAILURE.PASSTHROUGH) -------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum NilClass {
    /// Any NIL operand collapses the result to NIL (arithmetic, comparison).
    BinaryBlanket,
    /// A NIL operand passes through as the result (FLOOR, ROUND).
    UnaryBlanket,
    /// AND/NOT: strong-Kleene absorption (`kleene_truth_conformance_tests`).
    ThreeValAnd,
    ThreeValNot,
    /// SELECT: an UNKNOWN truth chooses neither candidate and answers that
    /// same absence. Not a blanket collapse — an absent *candidate* is only
    /// NIL in the result when the truth actually chose it.
    ThreeValSelect,
}

/// The Core (canonical-home == Core) passthrough words and their NIL
/// behavior class. `core_passthrough_completeness` keeps this in lockstep
/// with the registry: adding a Core arithmetic/comparison/logic passthrough
/// word without classifying it here fails the build.
const CORE_PASSTHROUGH: &[(&str, NilClass)] = &[
    ("ADD", NilClass::BinaryBlanket),
    ("SUB", NilClass::BinaryBlanket),
    ("MUL", NilClass::BinaryBlanket),
    // Division is total (LANG.VALUES.EXACT): a quotient by zero is a number.
    ("DIV", NilClass::BinaryBlanket),
    ("FLOOR", NilClass::UnaryBlanket),
    ("ROUND", NilClass::UnaryBlanket),
    // Equality decides over every number (LANG.VALUES.EXACT), so `EQ`
    // projects nothing of its own. Order does not reach `0/0`, so `LT`, `GT`,
    // `MIN` and `MAX` are projecting Words probed in `PROJECTING_WORDS`.
    ("EQ", NilClass::BinaryBlanket),
    ("NOT", NilClass::ThreeValNot),
    ("AND", NilClass::ThreeValAnd),
    ("SELECT", NilClass::ThreeValSelect),
];

/// Families whose Core passthrough words this suite is responsible for.
const COVERED_FAMILIES: &[&str] = &["knowledgeOrder", "truthOrder"];

fn lookup_class(name: &str) -> Option<NilClass> {
    CORE_PASSTHROUGH
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
}

#[test]
fn core_passthrough_completeness() {
    // Every Core passthrough word in a covered category must be classified,
    // and every classified word must still be a Core passthrough word.
    for meta in get_builtin_word_registry() {
        let probed = meta.nil_policy == NilPolicy::Passthrough
            || meta.nil_policy == NilPolicy::KleeneAbsorbing;
        let covered = probed && COVERED_FAMILIES.contains(&meta.family.as_str());
        if covered {
            assert!(
                lookup_class(&meta.name).is_some(),
                "Core passthrough/Kleene word `{}` (family {}) is not classified in \
                 CORE_PASSTHROUGH; add its NIL behavior class",
                meta.name,
                meta.family
            );
        }
    }

    for (name, class) in CORE_PASSTHROUGH {
        let meta = get_builtin_word_registry()
            .iter()
            .find(|m| &m.name == name)
            .unwrap_or_else(|| panic!("classified word `{name}` is not registered"));
        let want = match class {
            NilClass::BinaryBlanket | NilClass::UnaryBlanket => NilPolicy::Passthrough,
            _ => NilPolicy::KleeneAbsorbing,
        };
        assert_eq!(
            meta.nil_policy, want,
            "`{name}` is classified as {class:?} but registry says {:?}",
            meta.nil_policy
        );
    }
}

#[tokio::test]
async fn passthrough_blanket_collapses_to_nil() {
    for (name, class) in CORE_PASSTHROUGH {
        match class {
            NilClass::BinaryBlanket => {
                // LANG.FAILURE.PASSTHROUGH: any NIL operand => single NIL result.
                for code in [
                    format!("NIL NIL {name}"),
                    format!("1 NIL {name}"),
                    format!("NIL 1 {name}"),
                ] {
                    let stack = run_ok(&code).await;
                    assert_eq!(stack.len(), 1, "`{code}` must leave exactly one value");
                    assert!(is_nil(&stack[0]), "`{code}` must produce NIL");
                }
            }
            NilClass::UnaryBlanket => {
                let code = format!("NIL {name}");
                let stack = run_ok(&code).await;
                assert_eq!(stack.len(), 1, "`{code}` must leave exactly one value");
                assert!(is_nil(&stack[0]), "`{code}` must produce NIL");
            }
            // Not a blanket collapse; see `kleene_truth_conformance_tests`.
            NilClass::ThreeValAnd | NilClass::ThreeValNot | NilClass::ThreeValSelect => {}
        }
    }
}

// --- NIL projection: Projecting / CreatesNil words (LANG.FAILURE.PROJECT) ----------

/// Projecting words: a well-formed domain miss yields a reasoned NIL with a
/// reason; malformed use raises an ordinary error.
// `RANGE`/`SQRT`/`STR` are probed
// beside their own Words; `SHAPE` (a ragged operand) and `RESHAPE`
// (the materialization ceiling) in `shape_words_tests`; `BSEARCH` and `SEARCH`
// (an absent key or needle) in `search_words_tests`;
// `ABSENT` (the reason a program states) in `declared_outcomes_tests`. `GET`/`TAKE`/`PUT` are probed together
// in `index_projection_tests`: what they must agree on is one condition
// answered across three Words, not anything about one of them.
#[rustfmt::skip]
const PROJECTING_WORDS: &[&str] = &[
    "ABSENT", "BSEARCH", "CONTRACT", "DROP", "FORMAT", "GCD", "GET", "GT",
    "JSON-DECODE", "JSON-ENCODE", "LT", "MAX", "MIN", "NIL-REASON", "NUM", "ORDER", "POW", "PUT",
    "RATIO", "RECONCILE", "SEARCH", "SHAPE", "SORT", "SQRT", "STR", "TAKE", "WITHOUT",
];

/// The order Words project on a point over zero: `1/0` and `0/0` are numbers
/// (LANG.VALUES.EXACT) with no place in the order, so asking where one stands
/// is a question outside the Word's domain — `domainMiss`, the reason `SQRT`
/// gives a negative radicand. Equality is not order: `0/0 0/0 EQ` is TRUE.
#[tokio::test]
async fn nil_projection_order_words_project_on_nullity() {
    for code in [
        "0 0 DIV 1 LT",
        "1 0 0 DIV GT",
        "0 0 DIV 0 0 DIV LT",
        "0 0 DIV 1 MIN",
        "1 0 0 DIV MAX",
        // Inside a literal the point is spelled as the pair it is.
        "[ 1 0/0 ] SORT",
        "[ 0/0 1 ] ORDER",
        "[ 1 2 3 ] 0 0 DIV BSEARCH",
        "[ 1 0/0 ] 2 BSEARCH",
        // Lifted over lanes, the projection lands in the lane that asked.
        "[ 1 0/0 ] 1 LT 1 GET",
        "[ 0/0 1 ] 1 MAX 0 GET",
        "1 0 DIV 1 GT",
        "-1 0 DIV 1 LT",
        "1 0 DIV 5 MAX",
        "-1 0 DIV 5 MIN",
        "[ 1/0 0 ] SORT",
    ] {
        assert_eq!(
            reason(code).await.as_deref(),
            Some("domainMiss"),
            "`{code}` must project domainMiss"
        );
    }
    // Equality still decides at both points: identity is denotation.
    for (code, expected) in [
        ("0 0 DIV 0 0 DIV EQ", "TRUE"),
        ("1 0 DIV -1 0 DIV EQ", "TRUE"),
        ("1 0 DIV 0 0 DIV EQ", "FALSE"),
    ] {
        assert_eq!(top_of(code).await.to_string(), expected, "`{code}`");
    }
}

/// `FORMAT` projects on a number with no decimal spelling: the two points
/// over zero and `0/0` have no digits to write, so the answer is NIL rather
/// than an invented text.
#[tokio::test]
async fn nil_projection_format_projects_on_a_point_over_zero() {
    for code in ["1 0 DIV 2 FORMAT", "-1 0 DIV 0 FORMAT", "0 0 DIV 3 FORMAT"] {
        assert_eq!(
            reason(code).await.as_deref(),
            Some("domainMiss"),
            "`{code}` must project domainMiss"
        );
    }
    assert_eq!(
        text_answer("1 3 DIV 2 FORMAT").await.as_deref(),
        Some("0.33")
    );
}

/// Declaring a projection condition is a claim that the Word can hand back a
/// NIL it produced, so **every** Word that declares one carries a behavioral
/// probe. Declaring a NIL *policy* is not that claim: a Word that declared
/// `passthroughThenProject` with a projection of `never` could produce no NIL
/// of its own and would need no probe. The condition is what this set is
/// keyed on.
///
/// It used to be keyed on the policy as well — `createsNil` or
/// `passthroughThenProject`, *and* a declared condition. That conjunction let a
/// Word declare a projection and still escape every probe just by having some
/// other policy, and four Words did. Three of them were declaring something
/// untrue: `SORT` (`passthrough`) and `UNIQUE` (`rejectNil`) declared
/// `emptyVector`, a condition neither of them projects under — an empty
/// sequence sorts and dedupes to an empty sequence, not to an absence — and
/// `NIL?` (`consumeNil`) declared the same condition as
/// `NIL-REASON` but answers `FALSE`, never NIL. All
/// three are now `never`. The fourth, `NIL-REASON`, genuinely projects under
/// `consumeNil`, so keying on the condition alone brings it into the probed set
/// instead of leaving it unchecked.
#[test]
fn projecting_word_set_matches_registry() {
    let mut registry: Vec<&str> = crate::kernel::generated::GENERATED_WORDS
        .iter()
        .filter(|word| !word.projection.is_empty())
        .map(|word| word.name)
        .collect();
    registry.sort_unstable();
    let mut expected: Vec<&str> = PROJECTING_WORDS.to_vec();
    expected.sort_unstable();
    assert_eq!(
        registry, expected,
        "projecting word set drifted from PROJECTING_WORDS; add a NIL-projection \
         behavioral probe for any new word"
    );
}

/// The Text `code` leaves on top, for the non-projecting half of a probe.
async fn text_answer(code: &str) -> Option<String> {
    crate::interpreter::value_extraction_helpers::value_as_string(&top_of(code).await)
}

/// `STR` projects on the condition it declares: the sealed numeric grammar
/// writes integers and ratios, so an exact irrational has no lexeme and there
/// is no text to answer with.
///
/// It used to answer anyway, with a continued-fraction convergent — `2 SQRT STR`
/// gave `'665857/470832'`, so `2 SQRT STR NUM` was a *different number* than
/// `2 SQRT` with nothing on the stack to say so. That silence was worst inside
/// `REFLECT`-based partial application, where `STR` is how a runtime value
/// becomes a token: building a closure over an exact irrational quietly
/// substituted a rational look-alike for it.
#[tokio::test]
async fn nil_projection_str_projects_on_a_number_with_no_lexeme() {
    for code in [
        "2 SQRT STR",
        "2 SQRT 3 SQRT ADD STR",
        // A lifted STR projects per lane.
        "[ 1 2 ] 2 SQRT MUL STR 0 GET",
    ] {
        assert_eq!(reason(code).await.as_deref(), Some("domainMiss"));
    }

    // An exact real that collapses to a rational *does* have a lexeme, and a
    // rational is spelled exactly. Only the irrational case projects.
    assert_eq!(text_answer("4 SQRT STR").await.as_deref(), Some("2"));

    // The escape hatch is a Word, not a silent default: the caller names the
    // resolution, so the approximation is visible in the source.
    assert_eq!(
        text_answer("2 SQRT 10000 MUL ROUND 10000 DIV STR")
            .await
            .as_deref(),
        Some("7071/5000")
    );
}

/// `NIL-REASON` projects on the condition it declares: a value that is not an
/// operational NIL carrying a reason has no reason to read, so the answer is
/// NIL rather than an error. A reasoned NIL answers with its reason string.
///
/// The projected NIL carries the reason the contract registers, `domainMiss`:
/// a value that is not a NIL is a well-formed operand outside the accessor's
/// domain, the same reason `SQRT` gives a negative radicand. It used to be a
/// bare literal NIL, which made the declared projection reason the one no
/// program could observe — `5 NIL-REASON NIL-REASON` answered NIL instead of
/// naming why. `LANG.FAILURE.PROJECT` requires a projection to produce "NIL
/// with the reason its contract registers", and `LANG.VALUES.NIL` makes the
/// reason a NIL's entire observable content.
#[tokio::test]
async fn nil_projection_nil_reason_projects_on_a_reasonless_value() {
    for code in ["5 NIL-REASON", "[ 1 2 ] NIL-REASON", "'ab' NIL-REASON"] {
        assert_eq!(reason(code).await.as_deref(), Some("domainMiss"));
    }

    // Reading the projected NIL is what makes the registered reason
    // observable from inside the language.
    assert_eq!(
        text_answer("5 NIL-REASON NIL-REASON").await.as_deref(),
        Some("domainMiss"),
        "the projected NIL must name its own reason"
    );

    // A NIL that does carry a reason reads back as that reason rather than
    // projecting.
    let mut interp = Interpreter::new();
    interp.execute("-1 SQRT NIL-REASON").await.unwrap();
    let answer = interp.stack.last().expect("NIL-REASON pushes an answer");
    assert!(
        !answer.is_nil(),
        "a reasoned NIL must read back its reason, got NIL"
    );
}

/// `NIL?` answers a question; it never projects. It declared
/// `valueIsNotOperationalNilOrFieldAbsent`, the same condition
/// as `NIL-REASON`, but a predicate that returned NIL for "not a NIL" could not
/// be asked its question. Pinned so the declaration cannot drift back.
#[tokio::test]
async fn nil_check_answers_rather_than_projecting() {
    for (code, expected_nil) in [
        ("5 NIL?", false),
        ("[ 1 2 ] NIL?", false),
        ("'ab' NIL?", false),
        ("NIL NIL?", true),
        ("-1 SQRT NIL?", true),
        // A quotient by zero is a number (LANG.VALUES.EXACT).
        ("1 0 DIV NIL?", false),
    ] {
        let mut interp = Interpreter::new();
        interp
            .execute(code)
            .await
            .unwrap_or_else(|e| panic!("`{code}` must not error: {e}"));
        let answer = interp.stack.last().expect("NIL? pushes an answer");
        assert!(
            !answer.is_nil(),
            "`{code}` must answer TRUE or FALSE, never NIL"
        );
        assert_eq!(
            answer.as_truth(),
            Some(expected_nil),
            "`{code}` answered the wrong way"
        );
    }
}

#[tokio::test]
async fn nil_projection_comparison_nil_input() {
    // Comparison words are Total/Passthrough (LANG.CONTRACT.REGISTRY). A
    // NIL operand propagates as NIL output via the passthrough rule
    // (LANG.FAILURE.PASSTHROUGH).
    for name in &["EQ", "LT", "GT"] {
        for code in [
            format!("NIL 1 {name}"),
            format!("1 NIL {name}"),
            format!("NIL NIL {name}"),
        ] {
            let stack = run_ok(&code).await;
            assert_eq!(stack.len(), 1, "`{code}` must leave exactly one value");
            assert!(
                is_nil(&stack[0]),
                "`{code}` must produce NIL, got {:?}",
                stack[0]
            );
        }
    }
}

#[tokio::test]
async fn projecting_arithmetic_nil_input_passes_through() {
    // FLOOR/ROUND are Total/Passthrough: a NIL operand propagates as NIL
    // (LANG.FAILURE.PASSTHROUGH), and every number has a floor and a nearest
    // integer.
    for name in &["FLOOR", "ROUND"] {
        let code = format!("NIL {name}");
        let stack = run_ok(&code).await;
        assert_eq!(stack.len(), 1, "`{code}` must leave exactly one value");
        assert!(is_nil(&stack[0]), "`{code}` must produce NIL");
    }
}

#[tokio::test]
async fn malformed_use_raises_error_not_a_projected_nil() {
    // LANG.FAILURE.ERROR: "そもそも使い方が違う -> エラー". A non-numeric DIV operand
    // is structural misuse, not a domain miss. The addressing Words' half of
    // the same rule is in `index_projection_tests`.
    assert!(
        run("'x' 1 DIV").await.is_err(),
        "non-numeric DIV operand must raise an error, not project onto NIL"
    );
}

// --- `NIL?` and `SELECT` replace a reasoned NIL with a fallback (LANG.FAILURE.RECOVERY) ---

#[tokio::test]
async fn a_chosen_fallback_replaces_a_reasoned_nil() {
    // bare NIL replaced by the fallback
    let stack = run_ok("NIL 'S' BIND [ 0 ] S S NIL? SELECT").await;
    assert_eq!(format!("{}", stack[0]), "[ 0/1 ]");

    // non-NIL value passes through unchanged
    let stack = run_ok("[ 42 ] 'S' BIND [ 0 ] S S NIL? SELECT").await;
    assert_eq!(format!("{}", stack[0]), "[ 42/1 ]");

    // a reasoned NIL (a negative radicand) is replaced; no NIL survives
    let stack = run_ok("-1 SQRT 'S' BIND [ 7 ] S S NIL? SELECT").await;
    assert!(
        !is_nil(&stack[0]),
        "the fallback must replace the reasoned NIL"
    );
    assert_eq!(format!("{}", stack[0]), "[ 7/1 ]");
}

// --- A raised error propagates; a well-formed projected NIL keeps its reason

#[tokio::test]
async fn raised_errors_propagate_instead_of_projecting_to_nil() {
    // stack underflow propagates as an error
    let mut interp = Interpreter::new();
    assert!(
        interp.execute("ADD").await.is_err(),
        "stack underflow must propagate, not project to NIL"
    );

    // unknown word propagates as an error
    let mut interp = Interpreter::new();
    assert!(
        interp.execute("__NO_SUCH_WORD__").await.is_err(),
        "unknown word must propagate, not project to NIL"
    );
}

#[tokio::test]
async fn direct_projection_preserves_its_own_reason() {
    // A well-formed projected NIL (a negative radicand) keeps its own reason.
    let stack = run_ok("-1 SQRT").await;
    assert!(is_nil(&stack[0]));
    assert_eq!(
        reason_of(&stack[0]),
        Some(NilReason::DomainMiss),
        "a direct projection keeps its DomainMiss reason"
    );
}

// --- Property: NIL passthrough is total over the numeric domain -----------

#[cfg(test)]
mod properties {
    use super::run;
    use crate::agent::block_on;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        // For any scalar, a NIL co-operand makes a blanket-passthrough binary
        // op total and NIL — never an error, never a definite value.
        #[test]
        fn binary_passthrough_with_nil_is_nil(a in -50i64..50) {
            for op in ["ADD", "SUB", "MUL", "LT", "GT", "EQ"] {
                let stack = block_on(run(&format!("{a} NIL {op}")))
                    .unwrap_or_else(|e| panic!("`{a} NIL {op}` errored: {e}"));
                prop_assert_eq!(stack.len(), 1);
                prop_assert!(stack[0].is_nil(), "`{} NIL {}` must be NIL", a, op);
            }
        }

        // Division by zero is total: `1/0` for a nonzero dividend, `0/0`
        // for zero.
        #[test]
        fn division_by_zero_is_a_point_over_zero(a in -50i64..50) {
            let stack = block_on(run(&format!("{a} 0 DIV")))
                .unwrap_or_else(|e| panic!("`{a} 0 DIV` errored: {e}"));
            prop_assert!(!stack[0].is_nil());
            prop_assert_eq!(format!("{}", stack[0]), format!("{}/0", i64::from(a != 0)));
        }
    }
}
