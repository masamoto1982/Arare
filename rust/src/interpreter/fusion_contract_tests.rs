//! The fused route's Words against their contracts (`fusion_contract`).
//!
//! Which Words may run on plain values inside a block is read from
//! spec/words.json. These hold the hand-written side to it: no op, kernel or
//! quickened `Kind` exists for a Word the contract does not admit, and every
//! Word it admits has one, or is listed here with the reason it has none.
//! Whether each one answers as the interpreted route does is held by the
//! route-equality suites (`fused_block_tests`, `quickened_tests`), whose
//! generators draw the kernel Words too.

use crate::interpreter::fused_block::Op;
use crate::interpreter::fused_block_lower::word_op;
use crate::interpreter::fusion_contract::{admits, kernel, kernels};
use crate::interpreter::quickened::Kind;
use crate::kernel::generated::{generated_word_by_id, Arity, GENERATED_WORDS};

/// Admitted Words with no op of their own, and why.
const ADMITTED_WITHOUT_OP: &[(&str, &str)] = &[
    (
        "TRUE",
        "a Word literal: lowered as `Op::PushWord`, never called by id",
    ),
    (
        "FALSE",
        "a Word literal: lowered as `Op::PushWord`, never called by id",
    ),
    (
        "RECONCILE",
        "two different plain values reconcile to a NIL, which is not plain, \
         so a walk would decline whenever the Word had anything to decide",
    ),
    (
        "SQRT",
        "its law leaves the rationals for every rational that is not a square, \
         so a walk would almost always be abandoned at its first element",
    ),
];

#[test]
fn every_op_is_for_a_word_the_contract_admits() {
    for word in GENERATED_WORDS {
        let has_op = word_op(word.id).is_some();
        let has_kind = Kind::of(word.id).is_some();
        if has_op || has_kind {
            assert!(
                admits(word),
                "{} runs on the fused or quickened route, but its contract does not admit it",
                word.name
            );
        }
    }
}

#[test]
fn every_admitted_word_has_an_op_or_a_reason() {
    for word in GENERATED_WORDS.iter().filter(|w| admits(w)) {
        let listed = ADMITTED_WITHOUT_OP
            .iter()
            .any(|(name, _)| *name == word.name);
        let has_op = word_op(word.id).is_some();
        assert!(
            has_op != listed,
            "{}: an admitted Word has an op or is listed without one, not {}",
            word.name,
            if has_op { "both" } else { "neither" }
        );
    }
}

#[test]
fn each_kernel_is_its_words_only_route_and_matches_its_arity() {
    for k in kernels() {
        let word = generated_word_by_id(k.word);
        assert!(
            admits(word),
            "{} has a kernel but is not admitted",
            word.name
        );
        assert_eq!(
            word.stack_inputs,
            Arity::Fixed(k.arity as u8),
            "{}: kernel arity",
            word.name
        );
        assert!(
            Kind::of(k.word).is_none(),
            "{}: a quickened Kind and a kernel would be two laws for one Word",
            word.name
        );
        assert!(
            matches!(word_op(k.word), Some(Op::Kernel(found)) if std::ptr::eq(found, kernel(k.word).unwrap())),
            "{}: a hand-written op and a kernel would be two laws for one Word",
            word.name
        );
    }
}

/// The admitted set, written out: a contract edit that moves a Word in or out
/// of the fused route shows up here as a deliberate change.
#[test]
fn the_admitted_words_are_these() {
    let mut admitted: Vec<&str> = GENERATED_WORDS
        .iter()
        .filter(|w| admits(w))
        .map(|w| w.name)
        .collect();
    admitted.sort_unstable();
    assert_eq!(
        admitted,
        [
            "ADD",
            "AND",
            "DEPTH",
            "DIV",
            "EQ",
            "FALSE",
            "FLOOR",
            "GCD",
            "GT",
            "LT",
            "MAX",
            "MIN",
            "MUL",
            "NIL?",
            "NOT",
            "POW",
            "RECONCILE",
            "ROUND",
            "SELECT",
            "SQRT",
            "SUB",
            "TRUE",
        ]
    );
}
