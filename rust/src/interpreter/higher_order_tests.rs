//! Test suite for `MAP`'s result contract: the block's one result *is* the
//! mapped element, whatever shape it has.
//!
//! `MAP` used to unwrap a one-element Vector result into its element, a
//! leftover from the time a scalar and a one-element Vector were the same
//! thing. With the domains disjoint (LANG.VALUES.DISJOINT) that unwrapping
//! silently changed the answer and left no way to map onto singletons at all,
//! so these cases pin the shape rather than only the numbers.

use crate::interpreter::Interpreter;
use crate::types::display::render_stack;

async fn run(source: &str) -> String {
    let mut interp = Interpreter::new();
    interp
        .execute(source)
        .await
        .unwrap_or_else(|e| panic!("{} should run: {:?}", source, e));
    render_stack(interp.get_stack()).join(" ")
}

/// The block owes exactly one value (LANG.COLLECTIONS.HIGHER). A surplus
/// used to be discarded and the top taken — `[ 1 2 3 ] [ 2 MUL 7 ] MAP`
/// answered `[ 7 7 7 ]` and said nothing about the `2 MUL` it threw away
/// — and is now the same violation a block that leaves nothing has
/// always been, on every higher-order Word and on the fused route too
/// (`[ 1 2 ]` is a block the fused lowering would otherwise admit).
#[tokio::test]
async fn a_block_that_leaves_a_surplus_is_a_contract_violation() {
    for (source, message) in [
        (
            "[ 1 2 3 ] [ 1 2 ] MAP",
            "MAP: expected the block to leave one value, and it left 3",
        ),
        (
            "[ 1 2 3 ] [ 2 MUL 7 ] MAP",
            "MAP: expected the block to leave one value, and it left 2",
        ),
        (
            "[ 1 2 3 ] [ 1 GT TRUE ] FILTER",
            "FILTER: expected the predicate block to leave one truth value, and it left 2",
        ),
        (
            "[ 1 2 3 ] 0 [ ADD 5 ] FOLD",
            "FOLD: expected the block to leave one value, and it left 2",
        ),
        (
            "[ 1 2 3 ] 0 [ ADD 5 ] SCAN",
            "SCAN: expected the block to leave one value, and it left 2",
        ),
        (
            "[ 1 2 ] [ PRINT ] MAP",
            "MAP: expected the block to leave one value, and it left none",
        ),
    ] {
        let mut interp = Interpreter::new();
        let err = interp
            .execute(source)
            .await
            .expect_err("a block leaving other than one value must be refused");
        assert_eq!(err.to_string(), message, "{source}");
        assert_eq!(
            crate::error::ErrorCategory::from_error(&err).map(|c| c.as_protocol_str().to_string()),
            Some("blockContractViolation".to_string()),
            "{source}"
        );
    }
    // One value, however it was computed, is the block's result.
    assert_eq!(
        run("[ 1 2 3 ] [ 'X' BIND X X MUL ] MAP").await,
        "[ 1/1 4/1 9/1 ]"
    );
}

#[tokio::test]
async fn map_keeps_a_one_element_vector_result_whole() {
    assert_eq!(
        run("[ 1 2 ] [ 1 COLLECT ] MAP").await,
        "[ [ 1/1 ] [ 2/1 ] ]"
    );
}

#[tokio::test]
async fn map_keeps_a_nested_one_element_vector_whole() {
    assert_eq!(
        run("[ [ 1 ] [ 2 3 ] ] [ REVERSE ] MAP").await,
        "[ [ 1/1 ] [ 3/1 2/1 ] ]"
    );
}

#[tokio::test]
async fn a_mapped_singleton_stays_a_singleton_downstream() {
    // The failure this rules out was silent rather than loud: the element
    // read back as a scalar, so `5 ADD` answered `6/1` where `[ 6/1 ]` is
    // the answer and `[ 6 ] 6 EQ` is FALSE.
    assert_eq!(
        run("[ [ 1 ] [ 2 3 ] ] [ REVERSE ] MAP 0 GET 5 ADD").await,
        "[ 6/1 ]"
    );
}

#[tokio::test]
async fn map_still_collects_scalar_results_as_scalars() {
    assert_eq!(run("[ 1 2 3 ] [ 2 MUL ] MAP").await, "[ 2/1 4/1 6/1 ]");
}

#[tokio::test]
async fn map_by_word_name_follows_the_same_rule() {
    assert_eq!(
        run("[ 1 COLLECT ] 'WRAP' DEF [ 1 2 ] [ WRAP ] MAP").await,
        "[ [ 1/1 ] [ 2/1 ] ]"
    );
}
