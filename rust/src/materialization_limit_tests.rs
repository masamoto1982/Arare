//! Regression tests for generative-word materialization limits
//! (`RuntimeLimits::max_materialized_elements`).
//!
//! `RANGE`, `FILL`, and `RESHAPE` each loop internally to build a vector or
//! tensor, so they count as a single execution step and bypass the
//! step-count backstop. Before these guards, hostile sizes drove the process
//! into an OOM abort (an unrecoverable trap inside the WASM playground) or, for
//! shapes whose element-count product overflows `usize`, into a
//! `multiply with overflow` panic.
//!
//! A request past the ceiling is refused by the ceiling's name —
//! `resourceLimitExceeded`, resource `materializedElements` — before anything
//! is built, with the operands left on the stack as every refusal leaves
//! them (LANG.MACHINE.LIMITS). It is never a value: a ceiling is the host's,
//! and a NIL that flowed on from it let one program answer `ok` with
//! different values on two hosts. `RESHAPE` refuses the same way; its probe
//! lives in `shape_words_tests`.
//!
//! Two neighbouring probes live here as well: the extreme-index regressions
//! (`i64::MIN` counts and indices past `u32`), and the shared stack rendering
//! every observation surface goes through.

use crate::error::{AjisaiError, ResourceLimit};
use crate::interpreter::Interpreter;
use crate::test_support::top_nil_reason;
use crate::types::display::render_stack;

fn is_materialization_refusal(err: &AjisaiError) -> bool {
    matches!(
        err,
        AjisaiError::ResourceLimitExceeded {
            resource: ResourceLimit::MaterializedElements,
            ..
        }
    )
}

#[tokio::test]
async fn range_refuses_an_unbounded_count_by_the_ceilings_name() {
    let mut interp = Interpreter::new();
    let err = interp
        .execute("0 9999999999999 RANGE")
        .await
        .expect_err("an over-budget RANGE must be refused, never answered");
    assert!(is_materialization_refusal(&err), "{err:?}");
    assert_eq!(
        render_stack(interp.get_stack()),
        vec!["0/1".to_string(), "9999999999999/1".to_string()],
        "the bounds are put back, as every refusal leaves its operands"
    );
}

#[tokio::test]
async fn a_ceiling_is_never_recovered_as_a_value() {
    // The whole point of an ERROR over a projected NIL: a fallback written
    // for an absence must not be chosen by the host's ceiling. The program
    // below answered `[ 42 ]` under a small ceiling and the sequence under a
    // large one, both as `ok`, when the ceiling projected a NIL.
    let mut interp = Interpreter::new();
    let result = interp
        .execute("0 9999999999999 RANGE 'S' BIND [ 42 ] S S NIL? SELECT")
        .await;
    assert!(
        result.is_err(),
        "the ceiling must stop the run, not choose a branch: {result:?}"
    );
}

#[tokio::test]
async fn range_accepts_ordinary_size() {
    let mut interp = Interpreter::new();
    let result = interp.execute("0 5 RANGE").await;
    assert!(result.is_ok(), "small RANGE should still succeed");
    assert_eq!(
        top_nil_reason(&interp),
        None,
        "a small RANGE does not project onto NIL"
    );
}

#[tokio::test]
async fn range_handles_extreme_bounds_without_overflow() {
    // start/end at the i64 extremes: the span arithmetic must not overflow
    // while computing the over-budget element count, and the request is
    // refused by name.
    let mut interp = Interpreter::new();
    let program = format!("{} {} RANGE", i64::MIN, i64::MAX);
    let err = interp
        .execute(&program)
        .await
        .expect_err("full-i64-span RANGE must be refused, not panic");
    assert!(is_materialization_refusal(&err), "{err:?}");
}

#[tokio::test]
async fn range_infinite_direction_is_still_an_error() {
    // A malformed range (a bound that is not an integer) is not a budget
    // miss; it remains an ordinary channel error, under a different name.
    let mut interp = Interpreter::new();
    let err = interp
        .execute("0 1/2 RANGE")
        .await
        .expect_err("a non-integer RANGE bound stays a malformed-use error");
    assert!(!is_materialization_refusal(&err), "{err:?}");
}

#[tokio::test]
async fn fill_refuses_an_oversized_product_by_the_ceilings_name() {
    let mut interp = Interpreter::new();
    let err = interp
        .execute("[ 1000000 1000000 ] 7 FILL")
        .await
        .expect_err("a billion-element FILL must be refused, never answered");
    assert!(is_materialization_refusal(&err), "{err:?}");
    assert_eq!(
        render_stack(interp.get_stack()),
        vec!["[ 1000000/1 1000000/1 ]".to_string(), "7/1".to_string()],
        "the shape and the value are put back"
    );
}

#[tokio::test]
async fn fill_refuses_a_shape_product_that_overflows() {
    // The product of these dimensions overflows usize; the old
    // `shape.iter().product()` panicked here. The count has no number to
    // report, but the ceiling does.
    let mut interp = Interpreter::new();
    let err = interp
        .execute("[ 99999999 99999999 99999999 ] 1 FILL")
        .await
        .expect_err("an overflowing FILL shape must be refused, not panic");
    assert!(
        matches!(
            err,
            AjisaiError::ResourceLimitExceeded {
                resource: ResourceLimit::MaterializedElements,
                observed: None,
                ..
            }
        ),
        "{err:?}"
    );
}

/// LANG.COLLECTIONS.BUDGET names JSON-DECODE beside RANGE and FILL: the
/// members of every container in the text count together, nested ones
/// included.
#[tokio::test]
async fn json_decode_refuses_too_many_members_by_the_ceilings_name() {
    for (ceiling, refused) in [(4, true), (5, false)] {
        let mut interp = Interpreter::new();
        let mut limits = *interp.runtime_limits();
        limits.max_materialized_elements = ceiling;
        interp.set_runtime_limits(limits);
        let result = interp.execute("'[1, 2, [3, 4]]' JSON-DECODE").await;
        match result {
            Err(err) => {
                assert!(
                    refused && is_materialization_refusal(&err),
                    "ceiling {ceiling}: {err:?}"
                );
                assert_eq!(
                    render_stack(interp.get_stack()),
                    vec!["'[1, 2, [3, 4]]'".to_string()],
                    "ceiling {ceiling}: the text is put back"
                );
            }
            Ok(()) => assert!(!refused, "ceiling {ceiling}: must be refused"),
        }
    }
}

#[tokio::test]
async fn fill_accepts_ordinary_shape() {
    let mut interp = Interpreter::new();
    let result = interp.execute("[ 2 2 ] 7 FILL").await;
    assert!(result.is_ok(), "small FILL should still succeed");
    assert_eq!(
        top_nil_reason(&interp),
        None,
        "a small FILL does not project onto NIL"
    );
}

// Regression tests for extreme integer indices and counts
// (`compute_take_bounds`, `normalize_index`).
//
// `i64::MIN` has no positive i64 counterpart, so the old `(-count) as usize`
// in `TAKE` panicked the moment a `-9223372036854775808` count reached it.
// Index normalization additionally narrowed positive indices with a bare
// `index as usize`, which truncates out-of-range values on 32-bit wasm. Every
// case below must resolve to a clean error or `NIL` rather than crash or
// silently alias an in-range slot. A well-formed count or index that simply
// names a position past the end is `NIL`, whatever its magnitude: `TAKE`
// projects it the way `GET` always has (LANG.FAILURE.PROJECT).
const I64_MIN: &str = "-9223372036854775808";

#[tokio::test]
async fn take_projects_i64_min_count_without_panicking() {
    let mut interp = Interpreter::new();
    interp
        .execute(&format!("[ 1 2 3 ] {} TAKE", I64_MIN))
        .await
        .expect("i64::MIN TAKE count must project to NIL, not panic or error");
    let stack = interp.get_stack();
    assert_eq!(stack.len(), 1);
    assert!(stack[0].is_nil(), "a count past the end projects to NIL");
}

#[tokio::test]
async fn take_still_handles_ordinary_negative_count() {
    let mut interp = Interpreter::new();
    interp
        .execute("[ 1 2 3 ] -2 TAKE")
        .await
        .expect("negative TAKE within range should succeed");
    // Tail two elements remain.
    assert_eq!(interp.get_stack().len(), 1);
}

#[tokio::test]
async fn get_with_i64_min_index_yields_nil_not_panic() {
    let mut interp = Interpreter::new();
    let result = interp.execute(&format!("[ 1 2 3 ] {} GET", I64_MIN)).await;
    assert!(
        result.is_ok(),
        "out-of-range GET should produce NIL, not error"
    );
}

#[tokio::test]
async fn get_with_huge_positive_index_is_out_of_bounds() {
    // 2^40 is far beyond the vector but also exceeds u32: on 32-bit wasm a
    // truncating `as usize` could have aliased a valid index. It must read
    // as out-of-bounds (NIL) on every target.
    let mut interp = Interpreter::new();
    interp
        .execute("[ 1 2 3 ] 1099511627776 GET")
        .await
        .expect("huge index GET should resolve to NIL without error");
    let stack = interp.get_stack();
    assert!(
        stack.last().map(|v| v.is_nil()).unwrap_or(false),
        "huge out-of-range index must yield NIL"
    );
}

// CS3 (observation): the shared stack rendering.
//
// Every observation surface (CLI stack display, REPL, in-process conformance
// runner, JSON report) renders through one function —
// `crate::types::display::render_stack` — and it renders each slot from its
// value alone (LANG.VALUES.DENOTATION).
async fn render(code: &str) -> Vec<String> {
    let mut interp = Interpreter::new();
    interp
        .execute(code)
        .await
        .unwrap_or_else(|e| panic!("`{code}` unexpectedly errored: {e}"));
    render_stack(interp.get_stack())
}
#[tokio::test]
async fn arithmetic_result_renders_as_a_number() {
    assert_eq!(render("1 2 ADD").await, vec!["3/1".to_string()]);
}

#[tokio::test]
async fn truth_and_absence_render_canonically() {
    assert_eq!(render("TRUE").await, vec!["TRUE".to_string()]);
    assert_eq!(render("FALSE").await, vec!["FALSE".to_string()]);
    assert_eq!(render("NIL").await, vec!["NIL".to_string()]);
}

/// The same value renders the same way however it was produced: a Boolean
/// from a comparison and one from a `FOLD` of `AND`, a NIL from a literal and
/// one that passed through `MUL`.
#[tokio::test]
async fn rendering_does_not_depend_on_the_producing_word() {
    assert_eq!(
        render("3 2 GT").await,
        render("[ 3 4 ] [ 2 GT ] MAP TRUE [ AND ] FOLD").await
    );
    assert_eq!(render("NIL -1 MUL").await, render("NIL").await);
}
