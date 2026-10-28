//! The playground's bounded stack view (`stack_view.rs`): a value that fits is
//! the protocol node every host reads, and a value that does not says, about
//! the whole value, what the parts it left out would have decided.

use super::report::protocol_node_json;
use super::stack_view::{stack_view_json, value_view_json, VIEW_ELEMENTS_PER_COLLECTION};
use crate::test_support::run_ok;
use crate::types::value_protocol::value_to_protocol;
use serde_json::Value as Json;

async fn view_of(code: &str) -> Json {
    let stack = run_ok(code).await;
    value_view_json(stack.last().expect("the program leaves a value"))
}

#[tokio::test]
async fn a_value_that_fits_is_its_protocol_node() {
    for code in [
        "1 3 DIV",
        "2 SQRT",
        "'text'",
        "TRUE",
        "-1 SQRT",
        "[ ]",
        "1 100 RANGE",
        "[ [ 1 2 ] [ 3 4 ] ]",
        "[ 1 'a' TRUE [ 2/3 ] ]",
        "[ 1 0 DIV 0 0 DIV ]",
        "[ 'a' 'b' ] [ 1 [ 2 3 ] ] RECORD",
        "[ [ 'k' ] [ 1 ] RECORD 5 ]",
        "1 6 RANGE [ 2 3 ] RESHAPE",
        "[ 1 2 ] SQRT",
    ] {
        let stack = run_ok(code).await;
        let value = stack.last().expect("the program leaves a value");
        assert_eq!(
            value_view_json(value),
            protocol_node_json(&value_to_protocol(value)),
            "`{code}` fits the view and must be its protocol node"
        );
    }
}

#[tokio::test]
async fn a_long_vector_keeps_its_head_and_states_its_length() {
    let view = view_of("0 999 RANGE").await;
    let children = view["value"].as_array().expect("a vector's children");
    assert_eq!(children.len(), VIEW_ELEMENTS_PER_COLLECTION);
    assert_eq!(children[0]["value"]["numerator"], "0");
    assert_eq!(children[99]["value"]["numerator"], "99");
    assert_eq!(view["truncated"]["length"], 1000);
    assert_eq!(view["truncated"]["holdsNil"], false);
    assert_eq!(view["truncated"]["holdsRecord"], false);
}

#[tokio::test]
async fn a_vector_at_the_limit_is_not_cut() {
    let view = view_of("1 100 RANGE").await;
    assert_eq!(view["value"].as_array().unwrap().len(), 100);
    assert!(view.get("truncated").is_none());
}

#[tokio::test]
async fn nested_vectors_are_cut_at_every_depth() {
    let view = view_of("1 40000 RANGE [ 200 200 ] RESHAPE").await;
    let rows = view["value"].as_array().unwrap();
    assert_eq!(rows.len(), VIEW_ELEMENTS_PER_COLLECTION);
    assert_eq!(view["truncated"]["length"], 200);
    assert_eq!(
        rows[0]["value"].as_array().unwrap().len(),
        VIEW_ELEMENTS_PER_COLLECTION
    );
    assert_eq!(rows[0]["truncated"]["length"], 200);
}

#[tokio::test]
async fn a_change_past_the_cut_changes_the_view() {
    let before = view_of("0 999 RANGE").await;
    let after = view_of("0 999 RANGE 500 -1 PUT").await;
    assert_eq!(before["value"], after["value"], "the kept head is the same");
    assert_ne!(
        before, after,
        "a change in the left-out tail must still show"
    );
    assert_eq!(
        before,
        view_of("0 999 RANGE").await,
        "the view is deterministic"
    );
}

#[tokio::test]
async fn a_dense_and_a_boxed_vector_of_one_value_view_alike() {
    // `PUT` answers a boxed Vector; `RANGE` a dense Tensor. They are one value.
    assert_eq!(
        view_of("0 999 RANGE").await,
        view_of("0 999 RANGE 0 0 PUT").await
    );
}

#[tokio::test]
async fn what_the_tail_holds_is_stated() {
    let nil = view_of("0 199 RANGE 150 -1 SQRT PUT").await;
    assert_eq!(nil["truncated"]["holdsNil"], true);
    let record = view_of("0 199 RANGE 150 [ 'k' ] [ 1 ] RECORD PUT").await;
    assert_eq!(record["truncated"]["holdsRecord"], true);
    assert_eq!(record["truncated"]["holdsNil"], false);
}

#[tokio::test]
async fn the_stack_view_is_one_view_per_slot() {
    let stack = run_ok("1 0 999 RANGE").await;
    let view = stack_view_json(&stack);
    let slots = view.as_array().unwrap();
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0]["value"]["numerator"], "1");
    assert_eq!(slots[1]["truncated"]["length"], 1000);
}
