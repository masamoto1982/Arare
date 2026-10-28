//! Test suite for `crate::interpreter::math_ops` (MATH module MIN/MAX/SQRT).

use crate::interpreter::Interpreter;

async fn top_i64(program: &str) -> i64 {
    let mut interp = Interpreter::new();
    interp
        .execute(program)
        .await
        .expect("program should succeed");
    assert_eq!(interp.stack.len(), 1, "program: {program}");
    interp.stack[0]
        .as_scalar()
        .expect("expected scalar result")
        .to_i64()
        .expect("expected integer result")
}

#[tokio::test]
async fn min_and_max_pick_correctly() {
    assert_eq!(top_i64("3 8 MIN").await, 3);
    assert_eq!(top_i64("3 8 MAX").await, 8);
    assert_eq!(top_i64("-2 -9 MIN").await, -9);
    assert_eq!(top_i64("-2 -9 MAX").await, -2);
}

#[tokio::test]
async fn nil_passes_through_unary() {
    let mut interp = Interpreter::new();
    interp
        .execute("NIL SQRT")
        .await
        .expect("NIL passthrough should not error");
    assert_eq!(interp.stack.len(), 1);
    assert!(interp.stack[0].is_nil(), "SQRT of NIL should be NIL");
}

#[tokio::test]
async fn nil_passes_through_binary() {
    let mut interp = Interpreter::new();
    interp
        .execute("NIL 5 MIN")
        .await
        .expect("NIL passthrough should not error");
    assert_eq!(interp.stack.len(), 1);
    assert!(interp.stack[0].is_nil(), "MIN with NIL should be NIL");
}

#[tokio::test]
async fn non_number_input_errors() {
    let mut interp = Interpreter::new();
    let result = interp.execute("'hello' SQRT").await;
    assert!(
        result.is_err(),
        "SQRT of text should be a malformed-use error"
    );
}
/// LANG.COLLECTIONS.LIFT makes element-wise application the rule for an
/// arithmetic Word given a vector. `MIN`, `MAX` and `SQRT` were the three
/// that did not follow it, so a rectifier (`[ .. ] 0 MAX`), a clipped
/// gradient and a per-feature standard deviation each needed a `MAP` and a
/// block while `[ .. ] 0 ADD` lifted happily.
async fn lanes(program: &str) -> Vec<Option<i64>> {
    let mut interp = crate::interpreter::Interpreter::new();
    interp
        .execute(program)
        .await
        .expect("program should succeed");
    let top = &interp.stack[interp.stack.len() - 1];
    (0..top.len())
        .map(|i| {
            top.child(i)
                .and_then(|c| c.as_scalar().map(|f| f.to_i64().unwrap()))
        })
        .collect()
}

#[tokio::test]
async fn max_lifts_over_a_vector_with_a_scalar() {
    assert_eq!(
        lanes("[ -1 2 -3 ] 0 MAX").await,
        vec![Some(0), Some(2), Some(0)]
    );
}

/// A length-1 vector broadcasts the same way, so the rectifier written
/// `[ 0 ] MAX` and the one written `0 MAX` agree.
#[tokio::test]
async fn max_broadcasts_a_length_one_vector() {
    assert_eq!(
        lanes("[ -1 2 -3 ] [ 0 ] MAX").await,
        lanes("[ -1 2 -3 ] 0 MAX").await
    );
}

#[tokio::test]
async fn min_pairs_two_vectors_element_wise() {
    assert_eq!(
        lanes("[ 1 5 3 ] [ 4 2 6 ] MIN").await,
        vec![Some(1), Some(2), Some(3)]
    );
}

#[tokio::test]
async fn sqrt_lifts_over_a_vector() {
    assert_eq!(lanes("[ 4 9 ] SQRT").await, vec![Some(2), Some(3)]);
}

/// A negative radicand is a per-lane domain miss, not a failure of the
/// whole vector: the lane projects onto NIL and its neighbours survive.
#[tokio::test]
async fn sqrt_projects_one_bad_lane_to_nil() {
    assert_eq!(lanes("[ 4 -1 ] SQRT").await, vec![Some(2), None]);
}

#[tokio::test]
async fn min_passes_a_nil_lane_through() {
    assert_eq!(lanes("[ 1 NIL ] 2 MAX").await, vec![Some(2), None]);
}

/// Two vectors that neither match nor broadcast are a length error, the
/// same answer the arithmetic broadcast gives.
#[tokio::test]
async fn mismatched_lengths_error() {
    let mut interp = crate::interpreter::Interpreter::new();
    let result = interp.execute("[ 1 2 3 ] [ 1 2 ] MIN").await;
    assert!(result.is_err(), "unequal lengths should not silently pair");
}

/// FLOOR and ROUND over a Vector holding an irrational lane answer each lane
/// exactly, as they do for that irrational on its own. The flat rational
/// route used to drop the irrational lane and abort on the short buffer.
#[tokio::test]
async fn floor_and_round_lift_over_irrational_lanes() {
    use crate::test_support::top;
    assert_eq!(top("[ 2 ] SQRT FLOOR").await, "[ 1/1 ]");
    assert_eq!(top("[ 2 8 ] SQRT FLOOR").await, "[ 1/1 2/1 ]");
    assert_eq!(top("[ 2 3 ] SQRT ROUND").await, "[ 1/1 2/1 ]");
    assert_eq!(top("[ 4 2 ] SQRT FLOOR").await, "[ 2/1 1/1 ]");
    assert_eq!(
        top("[ [ 2 ] [ 3 ] ] SQRT FLOOR").await,
        "[ [ 1/1 ] [ 1/1 ] ]"
    );
}

/// MIN and MAX align two shapes at the innermost axis, as ADD does
/// (LANG.COLLECTIONS.LIFT): `[ 0 5 ]` is a row reused down the columns of a
/// matrix, not one value per row. A ragged Vector pairs with no Vector.
#[tokio::test]
async fn min_aligns_shapes_innermost_like_add() {
    assert_eq!(
        crate::test_support::top("[ [ 1 2 ] [ 3 4 ] ] [ 0 5 ] MIN").await,
        "[ [ 0/1 2/1 ] [ 0/1 4/1 ] ]"
    );
    assert_eq!(
        crate::test_support::error_of("[ 1 [ 2 3 ] ] [ 1 1 ] MAX").await,
        "shapeMismatch"
    );
}
