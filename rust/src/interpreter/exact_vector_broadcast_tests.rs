//! Behavioral coverage for element-wise arithmetic over vectors/structures
//! that carry irrational `ExactScalar` lanes (LANG.COLLECTIONS.LIFT vector ops + LANG.VALUES.EXACT
//! exact-real scalars).
//!
//! Before this path existed, any vector containing an irrational continued
//! fraction fell through to the rational `FlatTensor` broadcast, which
//! hard-errors on `ExactScalar` (`FlatTensor::from_value`). These tests pin
//! that such vectors now compute lane-by-lane as exact reals, that the
//! all-rational route is unchanged, and that the broadcast shape rules and the
//! per-lane division-by-zero NIL Projection Rule are preserved.

use crate::error::NilReason;
use crate::interpreter::Interpreter;
use crate::test_support::{error_of, run_ok, top};
use crate::types::fraction::Fraction;
use crate::types::{Value, ValueData};

fn vector_children(value: &Value) -> &[Value] {
    match &value.data {
        ValueData::Vector(items) => items.as_slice(),
        other => panic!("expected a vector, got {other:?}"),
    }
}

fn is_exact_real_lane(value: &Value) -> bool {
    matches!(value.data, ValueData::ExactScalar(_) | ValueData::Scalar(_))
}

/// Two equal-length vectors of irrationals add lane-by-lane, staying exact and
/// never collapsing to a tensor-conversion error.
#[tokio::test]
async fn irrational_vector_plus_irrational_vector_is_exact() {
    let stack = run_ok("[ 2 3 ] [ SQRT ] MAP [ 2 3 ] [ SQRT ] MAP ADD").await;
    assert_eq!(stack.len(), 1);
    let children = vector_children(&stack[0]);
    assert_eq!(children.len(), 2, "result must keep both lanes");
    assert!(
        children
            .iter()
            .all(|c| !c.is_nil() && is_exact_real_lane(c)),
        "each lane of √n + √n must stay an exact real, got {children:?}"
    );
}
/// A bare irrational scalar broadcasts across a rational vector, producing one
/// exact lane per element instead of erroring.
#[tokio::test]
async fn irrational_scalar_broadcasts_across_rational_vector() {
    let stack = run_ok("[ 1 2 3 ] 2 SQRT MUL").await;
    assert_eq!(stack.len(), 1);
    let children = vector_children(&stack[0]);
    assert_eq!(
        children.len(),
        3,
        "√2 must broadcast across all three lanes"
    );
    assert!(
        children
            .iter()
            .all(|c| !c.is_nil() && is_exact_real_lane(c)),
        "each lane of [1 2 3] * √2 must stay an exact real, got {children:?}"
    );
}

/// Unequal-length vectors raise the same `VectorLengthMismatch` the rational
/// broadcast does — exactness does not relax the shape contract.
#[tokio::test]
async fn irrational_vector_length_mismatch_errors() {
    let mut interp = Interpreter::new();
    let result = interp
        .execute("[ 2 3 ] [ SQRT ] MAP [ 2 3 4 ] [ SQRT ] MAP ADD")
        .await;
    assert!(
        result.is_err(),
        "mismatched irrational vector lengths must error, got {:?}",
        interp.get_stack()
    );
}

/// A per-lane division by zero is the point over zero that lane's sign
/// names, matching the scalar `√x 0 DIV` → `1/0` (LANG.VALUES.EXACT) rather
/// than aborting the vector.
#[tokio::test]
async fn irrational_vector_div_by_zero_lane_is_the_point_over_zero() {
    let stack = run_ok("[ 2 3 ] [ SQRT ] MAP [ 1 0 ] DIV").await;
    assert_eq!(stack.len(), 1);
    let children = vector_children(&stack[0]);
    assert_eq!(children.len(), 2);
    assert!(
        is_exact_real_lane(&children[0]) && !children[0].is_nil(),
        "first lane √2 / 1 must stay exact, got {:?}",
        children[0]
    );
    assert_eq!(
        children[1],
        Value::from_fraction(Fraction::infinity()),
        "√3 / 0 is the sign of √3 over zero"
    );
}

/// Regression: an all-rational vector op never touches the exact path and
/// keeps its plain rational result (still the dense-tensor representation, so
/// compare against the literal value rather than assuming a `Vector`).
#[tokio::test]
async fn rational_vector_addition_unchanged() {
    let stack = run_ok("[ 1 2 3 ] [ 10 20 30 ] ADD").await;
    assert_eq!(stack.len(), 1);
    let expected = run_ok("[ 11 22 33 ]").await;
    assert_eq!(
        stack[0], expected[0],
        "rational vector add must be unchanged"
    );
}

/// **An empty axis stays empty when a one-element axis broadcasts over it.**
///
/// `broadcast_shape` stretched a length-1 axis to `a_dim.max(b_dim)`, which is
/// right for every length but zero: `[ ] 1 ADD` aligns shapes `[0]` and `[]`,
/// so the scalar's implicit `1` won the `max` and the broadcast then read lane
/// 0 of a zero-lane tensor. That was a panic — no value, no NIL, no ERROR, so
/// no outcome under LANG.FAILURE.TRICHOTOMY at all, and a program whose
/// predicted outcome set is vacuously wrong whatever it contains. Found by the
/// composition sweep behind `scripts/check-outcome-prediction.mjs`.
#[tokio::test]
async fn a_one_element_axis_broadcast_over_an_empty_one_stays_empty() {
    let empty = run_ok("[ ]").await;
    for code in [
        "[ ] 1 ADD",
        "[ 1 ] [ ] ADD",
        "[ ] [ ] ADD",
        "[ ] 1 DIV",
        "[ ] 0 DIV",
    ] {
        let stack = run_ok(code).await;
        assert_eq!(stack.len(), 1, "`{code}` must leave one value");
        assert_eq!(
            stack[0], empty[0],
            "`{code}` must answer the empty vector, got {:?}",
            stack[0]
        );
    }
}

/// The neighbouring shape rules are untouched: an empty axis against a
/// *longer* one still mismatches, and ordinary broadcasts still stretch.
#[tokio::test]
async fn an_empty_axis_against_a_longer_one_still_mismatches() {
    let mut interp = Interpreter::new();
    let error = interp
        .execute("[ 1 2 ] [ ] ADD")
        .await
        .expect_err("shapes [2] and [0] do not broadcast");
    assert_eq!(
        crate::error::ErrorCategory::from_error(&error)
            .expect("a program ERROR has a category")
            .as_protocol_str(),
        "shapeMismatch",
        "got {error}"
    );

    let stack = run_ok("[ 1 ] [ 2 3 ] ADD").await;
    assert_eq!(stack[0], run_ok("[ 3 4 ]").await[0]);
}

/// **An absent lane stays absent through the exact-real broadcast.**
///
/// The exact lift read a lane through `ExactReal::from_fraction(Fraction::
/// nil())` — a *number* whose denominator happens to be zero — so the law
/// computed with it and answered an observable `0/0` scalar. The lane stopped
/// being an absence at all: `NIL-REASON` on it projected, the answer for a
/// value that is not a NIL, because by then it was a value.
/// Losing the reason is one bug; losing the NIL is a value escaping
/// LANG.FAILURE.TRICHOTOMY.
#[tokio::test]
async fn an_absent_lane_survives_the_exact_real_broadcast() {
    for code in [
        "[ 2 3 ] [ SQRT ] MAP [ -1 -4 ] SQRT ADD",
        "[ 2 3 ] [ SQRT ] MAP [ -1 -4 ] SQRT MUL",
        "[ 2 3 ] [ SQRT ] MAP [ -1 -4 ] SQRT [ 1 1 ] ADD ADD",
    ] {
        let stack = run_ok(code).await;
        let lane = stack[0]
            .child(1)
            .unwrap_or_else(|| panic!("`{code}` must leave a two-lane vector"));
        assert!(lane.is_nil(), "`{code}` lane 1 must stay NIL, got {lane:?}");
        assert_eq!(
            lane.nil_reason().cloned(),
            Some(NilReason::DomainMiss),
            "`{code}` lane 1 must keep the reason it was created with"
        );
        // The exact lane beside it is NIL too: the leftmost absent operand
        // decides, and here it is the right operand's lane.
        assert!(stack[0].child(0).unwrap().is_nil());
    }
}

/// **A one-element Vector broadcasts across irrational lanes, in either
/// position, as it does across rational ones.**
///
/// LANG.COLLECTIONS.LIFT: a length-1 axis's single lane is reused across the
/// other's length, and `[ 1 2 3 ] [ 10 ] MUL` is the spec's own example. The
/// exact-real route walked the operands requiring equal lengths, so the same
/// program with an irrational lane raised `shapeMismatch`.
#[tokio::test]
async fn a_one_element_vector_broadcasts_across_irrational_lanes() {
    let irrational = "[ 2 3 ] [ SQRT ] MAP";
    for word in ["ADD", "SUB", "MUL", "DIV"] {
        assert_eq!(
            top(&format!("{irrational} [ 2 ] {word}")).await,
            top(&format!("{irrational} 2 {word}")).await,
            "{word}: the one-element Vector on the right"
        );
        assert_eq!(
            top(&format!("[ 2 ] {irrational} {word}")).await,
            top(&format!("2 {irrational} {word}")).await,
            "{word}: the one-element Vector on the left"
        );
    }
}

/// **The exact-real route aligns shapes at the innermost axis, as the
/// rational route does.** `[ [ 1 2 ] [ 3 4 ] ] [ 10 20 ] ADD` pairs `10` with
/// the first column; the exact route used to pair it with the first row, so
/// one program meant two things depending on whether a lane was rational.
#[tokio::test]
async fn the_exact_real_route_aligns_shapes_innermost() {
    assert_eq!(
        top("[ [ 1 2 ] [ 3 4 ] ] [ 10 20 ] ADD").await,
        "[ [ 11/1 22/1 ] [ 13/1 24/1 ] ]"
    );
    assert_eq!(
        top("[ [ 1 2 ] [ 3 4 ] ] [ 2 3 ] [ SQRT ] MAP ADD").await,
        top("1 2 SQRT ADD 2 3 SQRT ADD 3 2 SQRT ADD 4 3 SQRT ADD 4 COLLECT [ 2 2 ] RESHAPE").await
    );
}

/// **Two Vectors where either is ragged do not pair** (LANG.COLLECTIONS.LIFT:
/// "so is any pairing of two vectors where either one is ragged"), on both
/// routes and whatever the lengths. Equal top-level lengths used to be zipped.
/// A scalar still combines with every element of a ragged Vector.
#[tokio::test]
async fn a_ragged_vector_does_not_pair_with_a_vector() {
    for code in [
        "[ 1 [ 2 3 ] ] [ 1 1 ] MUL",
        "[ 1 [ 2 3 ] ] [ 1 1 ] ADD",
        "[ 1 1 ] [ 1 [ 2 3 ] ] SUB",
        "[ 1 [ 2 3 ] ] [ 1 0 ] DIV",
        "[ 1 [ 2 3 ] ] [ 2 ] MUL",
        "[ 1 [ 2 3 ] ] [ 2 3 ] [ SQRT ] MAP MUL",
        "[ 2 3 ] [ SQRT ] MAP [ 1 [ 2 3 ] ] ADD",
    ] {
        assert_eq!(error_of(code).await, "shapeMismatch", "`{code}`");
    }
    assert_eq!(top("[ 1 [ 2 3 ] ] 2 MUL").await, "[ 2/1 [ 4/1 6/1 ] ]");
    assert_eq!(
        top("[ 1 [ 2 3 ] ] 2 SQRT MUL").await,
        top("2 SQRT [ 1 [ 2 3 ] ] MUL").await
    );
}
