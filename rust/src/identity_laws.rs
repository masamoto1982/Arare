//! The identity law, held against the implementation.
//!
//! `spec/identity.json` states one law — identity is denotation — and what each
//! of the three levels that decide it can actually deliver. This file runs the
//! third level's claims, the ones about User Word content identity, because
//! that is the level whose procedure is neither total nor obviously sound and
//! the only one with nothing testing it: `word_identity_tests.rs` covers the
//! BLAKE3 digest function (hash vectors, shape, distinct inputs) and stops
//! there, saying nothing about what a Word's identity *means*.
//!
//! Soundness is load-bearing rather than decorative. The host deduplicates user
//! words on import by content identity (`src/gui/interpreter-state-persistence.ts`),
//! so two Words sharing an identity are merged into one. If an identity were
//! ever unsound, that merge would silently replace one of a reader's Words with
//! a different one.
//!
//! Lives in `src/` rather than `tests/` because `word_identity` is
//! `pub(crate)`: asserting on identities directly is worth more than widening
//! the public API to reach them.

use crate::interpreter::Interpreter;
use serde_json::Value as Json;

fn spec() -> Json {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../spec/identity.json");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read the identity law at {path}: {e}"));
    serde_json::from_str(&text).expect("spec/identity.json is valid JSON")
}

fn level(s: &Json, id: &str) -> Json {
    s["levels"]
        .as_array()
        .expect("levels")
        .iter()
        .find(|l| l["id"] == id)
        .unwrap_or_else(|| panic!("no level {id}"))
        .clone()
}

fn run_to_interpreter(src: &str) -> Interpreter {
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
    })
}

fn identity_of(src: &str, word: &str) -> String {
    let interp = run_to_interpreter(src);
    interp
        .word_identity(word)
        .cloned()
        .unwrap_or_else(|| panic!("{src:?} defined no Word named {word}"))
}

/// Whole-stack observation, the way the conformance runner reads a result.
fn observe(src: &str) -> String {
    let interp = run_to_interpreter(src);
    interp
        .get_stack()
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn pair(entry: &Json) -> (String, String, String, String) {
    (
        entry["left"].as_str().expect("left").to_string(),
        entry["leftWord"].as_str().expect("leftWord").to_string(),
        entry["right"].as_str().expect("right").to_string(),
        entry["rightWord"].as_str().expect("rightWord").to_string(),
    )
}

/// Every normalization the law says identity sees through really is invisible
/// to it. These are what make content identity more than a hash of the source
/// text — without them it would answer `unknown` for two definitions that
/// differ only in how they were typed.
#[test]
fn identity_is_invariant_under_every_declared_normalization() {
    let word_level = level(&spec(), "userWordContentIdentity");
    let cases = word_level["invariantUnder"]
        .as_array()
        .expect("invariantUnder");
    assert!(!cases.is_empty(), "the law must declare invariances");

    for case in cases {
        let id = case["id"].as_str().expect("case id");
        let (left, left_word, right, right_word) = pair(case);
        assert_eq!(
            identity_of(&left, &left_word),
            identity_of(&right, &right_word),
            "identity should be invariant under {id}, but {left:?} and {right:?} differ",
        );
    }
}

/// What the law says identity does distinguish, it distinguishes. Without this
/// the invariance test above would be satisfied by an identity that answered
/// `same` for everything.
#[test]
fn identity_distinguishes_what_the_law_says_it_does() {
    let word_level = level(&spec(), "userWordContentIdentity");
    let cases = word_level["distinguishes"]
        .as_array()
        .expect("distinguishes");
    assert!(
        !cases.is_empty(),
        "the law must declare what identity separates"
    );

    for case in cases {
        let id = case["id"].as_str().expect("case id");
        let (left, left_word, right, right_word) = pair(case);
        assert_ne!(
            identity_of(&left, &left_word),
            identity_of(&right, &right_word),
            "identity should distinguish {id}, but {left:?} and {right:?} share one",
        );
    }
}

/// The incompleteness the law admits, demonstrated: two definitions that denote
/// one function and carry two identities. This is why an identity mismatch is
/// `unknown` and never `different` — and why the host may deduplicate on a
/// match but must not conclude anything from a mismatch.
#[test]
fn a_different_identity_does_not_mean_a_different_denotation() {
    let word_level = level(&spec(), "userWordContentIdentity");
    let witness = &word_level["incompletenessWitness"];
    let (left, left_word, right, right_word) = pair(witness);

    assert_ne!(
        identity_of(&left, &left_word),
        identity_of(&right, &right_word),
        "the incompleteness witness is pointless if the two already share an identity",
    );

    let inputs: Vec<String> = witness["agreeOn"]
        .as_array()
        .expect("agreeOn")
        .iter()
        .map(|v| v.as_str().expect("an input program").to_string())
        .collect();
    assert!(
        !inputs.is_empty(),
        "the witness must name inputs to agree on"
    );

    for input in inputs {
        let left_result = observe(&format!("{left}\n{input} {left_word}"));
        let right_result = observe(&format!("{right}\n{input} {right_word}"));
        assert_eq!(
            left_result, right_result,
            "the witness claims these denote one function, but on {input:?} \
             {left_word} gave {left_result:?} and {right_word} gave {right_result:?}",
        );
    }
}

/// Soundness, in the direction the host relies on: definitions that observably
/// disagree must never share an identity. A violation here is not a
/// specification nit — it is import deduplication merging two different Words.
#[test]
fn definitions_that_disagree_never_share_an_identity() {
    // Bodies chosen to differ observably on the same input while staying close
    // enough in shape that a careless normalization could collapse them.
    let bodies = [
        "1 ADD",
        "2 ADD",
        "1 SUB",
        "1 MUL",
        "2 MUL",
        "1 ADD 1 ADD",
        "-1 MUL",
        "1 ADD -1 MUL",
    ];
    let probe = "7";

    let mut seen: Vec<(String, String, String)> = Vec::new();
    for (index, body) in bodies.iter().enumerate() {
        let name = format!("IDSOUND{index}");
        let define = format!("[ {body} ] '{name}' DEF");
        let identity = identity_of(&define, &name);
        let result = observe(&format!("{define}\n{probe} {name}"));
        seen.push((identity, result, (*body).to_string()));
    }

    for i in 0..seen.len() {
        for j in (i + 1)..seen.len() {
            let (id_a, out_a, body_a) = &seen[i];
            let (id_b, out_b, body_b) = &seen[j];
            if out_a != out_b {
                assert_ne!(
                    id_a, id_b,
                    "[ {body_a} ] and [ {body_b} ] answer {out_a:?} and {out_b:?} on {probe}, so \
                     they denote different functions, yet they share content identity {id_a} — \
                     import deduplication would merge them",
                );
            }
        }
    }
}

/// The value levels, which the law says are already what it asks for: the
/// algebraic field decides, and a Record comparison decides because each of its
/// pairs does.
#[test]
fn the_value_levels_answer_as_the_law_states() {
    let s = spec();
    for level_id in ["scalarAlgebraicField", "recordDenotation"] {
        let lvl = level(&s, level_id);
        for witness in lvl["witnesses"].as_array().expect("witnesses") {
            let source = witness["source"].as_str().expect("source");
            let expect = witness["expect"].as_str().expect("expect");
            let observed = observe(source);
            assert!(
                observed.contains(expect),
                "{level_id}: {source:?} should answer {expect}, got {observed:?}",
            );
        }
    }
}
