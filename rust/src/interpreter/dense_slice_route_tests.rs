//! The dense routes of `TAKE`, `DROP`, `CONCAT`, and
//! `BSEARCH` answer exactly what the materializing route answers, at the same
//! price.
//!
//! Each of these Words used to box every lane of a dense Tensor into a `Value`
//! before reading any of them. They now cut, join or read the columns
//! directly. That is a representation decision, and LANG.AUTHORITY.FREEDOM
//! requires it to be unobservable: the same value (absent lanes' reasons
//! included), the same display, the same collection work. Every case here runs
//! one Word on the same data held two ways — a dense Tensor, and the nested
//! `Vector` that `[ ] CONCAT` leaves — and requires the two runs to agree on
//! all three.

use crate::interpreter::Interpreter;
use crate::types::{Value, ValueData};

/// What `word` leaves and charges, run on what `setup` leaves.
async fn observe(setup: &str, word: &str) -> (Value, String, u64) {
    let mut interp = Interpreter::new();
    interp
        .execute(setup)
        .await
        .unwrap_or_else(|e| panic!("setup `{setup}` must compute, got: {e:?}"));
    interp
        .execute(word)
        .await
        .unwrap_or_else(|e| panic!("`{setup} {word}` must compute, got: {e:?}"));
    let top = interp.get_stack().last().cloned().expect("an answer");
    let shown = interp
        .get_stack()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    (top, shown, interp.collection_work_used())
}

fn is_dense(value: &Value) -> bool {
    matches!(value.data, ValueData::Tensor { .. })
}

/// `dense` leaves a Tensor below the operand `word` takes; the same source with
/// `[ ] CONCAT` after the Tensor leaves it nested. Both must answer alike.
async fn routes_agree(dense: &str, operand: &str, word: &str) {
    let mut interp = Interpreter::new();
    interp.execute(dense).await.expect("dense setup computes");
    assert!(
        is_dense(interp.get_stack().last().expect("a value")),
        "`{dense}` must leave a dense Tensor for this case to test anything"
    );
    let nested = format!("{dense} [ ] CONCAT");
    let mut interp = Interpreter::new();
    interp
        .execute(&nested)
        .await
        .expect("nested setup computes");
    assert!(
        !is_dense(interp.get_stack().last().expect("a value")),
        "`{nested}` must leave a nested Vector"
    );

    let (dense_top, dense_shown, dense_work) = observe(&format!("{dense} {operand}"), word).await;
    let (nested_top, nested_shown, nested_work) =
        observe(&format!("{nested} {operand}"), word).await;
    let case = format!("`{dense} {operand} {word}`");
    assert_eq!(dense_top, nested_top, "{case}: the value differs by route");
    assert_eq!(
        dense_shown, nested_shown,
        "{case}: the display differs by route"
    );
    assert_eq!(
        dense_top.nil_reason(),
        nested_top.nil_reason(),
        "{case}: the absence differs by route"
    );
    assert_eq!(
        dense_work, nested_work,
        "{case}: the charge differs by route"
    );
}

/// A flat range; absent lanes — quotients by zero, the absence a dense
/// Tensor holds as the dividend over zero, with its reason — among present
/// ones and on their own; and a rank-2 tensor.
const OPERANDS: [&str; 4] = [
    "0 9 RANGE",
    "[ 1 2 3 4 5 ] [ 1 0 1 0 1 ] DIV",
    "0 4 RANGE [ 0 DIV ] MAP",
    "0 11 RANGE [ 4 3 ] RESHAPE",
];

#[tokio::test]
async fn take_and_drop_answer_alike_on_both_routes() {
    for operand in OPERANDS {
        for count in ["0", "2", "-2", "3", "99", "-99"] {
            routes_agree(operand, count, "TAKE").await;
            routes_agree(operand, count, "DROP").await;
        }
    }
}

#[tokio::test]
async fn a_dense_slice_stays_dense() {
    let mut interp = Interpreter::new();
    interp.execute("0 999 RANGE 10 TAKE").await.unwrap();
    assert!(is_dense(interp.get_stack().last().unwrap()));
    let mut interp = Interpreter::new();
    interp
        .execute("0 999 RANGE 0 999 RANGE CONCAT")
        .await
        .unwrap();
    assert!(is_dense(interp.get_stack().last().unwrap()));
}

#[tokio::test]
async fn a_slice_keeps_the_reasons_of_its_absent_lanes() {
    let (top, _, _) = observe(
        "0 4 RANGE [ 'X' BIND 'x' NUM ] MAP",
        "1 DROP 0 GET NIL-REASON",
    )
    .await;
    assert_eq!(top.to_string(), "'invalidEncoding'");
}

#[tokio::test]
async fn concat_answers_alike_on_both_routes() {
    for left in OPERANDS {
        for right in OPERANDS {
            // Both nested on one side, both dense on the other; rows of
            // different shapes take the nested route on both.
            let dense = format!("{left} {right} CONCAT");
            let nested = format!("{left} [ ] CONCAT {right} [ ] CONCAT CONCAT");
            let (dense_top, dense_shown, _) = observe(&dense, "").await;
            let (nested_top, nested_shown, _) = observe(&nested, "").await;
            assert_eq!(dense_top, nested_top, "`{dense}`");
            assert_eq!(dense_shown, nested_shown, "`{dense}`");
            let (_, _, dense_work) = observe(&format!("{left} {right}"), "CONCAT").await;
            let (_, _, nested_work) =
                observe(&format!("{left} [ ] CONCAT {right} [ ] CONCAT"), "CONCAT").await;
            assert_eq!(dense_work, nested_work, "`{dense}`: the charge differs");
        }
    }
    // The absent lanes of the right operand keep their reasons at their new
    // positions.
    let (top, _, _) = observe(
        "0 2 RANGE 0 1 RANGE [ 'X' BIND 'x' NUM ] MAP CONCAT",
        "4 GET NIL-REASON",
    )
    .await;
    assert_eq!(top.to_string(), "'invalidEncoding'");
}

#[tokio::test]
async fn the_searches_answer_alike_on_both_routes() {
    for operand in ["0 9 RANGE", "0 9 RANGE 1/3 MUL"] {
        for keys in ["3", "1/3", "[ 0 9 10 ]", "[ 2/3 5 ]", "-1"] {
            routes_agree(operand, keys, "BSEARCH").await;
        }
    }
}

#[tokio::test]
async fn bsearch_refuses_an_unsorted_dense_operand() {
    let mut interp = Interpreter::new();
    let err = interp
        .execute("0 9 RANGE REVERSE 3 BSEARCH")
        .await
        .expect_err("an unsorted operand is refused");
    assert!(err.to_string().contains("ascending"), "{err}");
    // The operands are back where the program left them.
    assert_eq!(interp.get_stack().len(), 2);
}
