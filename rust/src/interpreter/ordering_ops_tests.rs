//! What `UNIQUE` and `TALLY` answer, and in what representation.
//!
//! Both run one hash-keyed scan. Over a flat pure-integer dense buffer that scan
//! keys the `i64` numerator column instead of a boxed `Value` per lane: every
//! such lane has denominator 1, so numerator equality *is* value equality, and
//! `Fraction`'s cross-multiplying `eq` agrees lane for lane. Everything else
//! keeps the boxed scan, and the tests below pin both halves — the fast one for
//! its answers and its ordering, the slow one for what it must not lose.
//!
//! The pricing half lives in `collection_meter_tests`, whose subject is a charge
//! that must not turn on a representation decision.

use crate::test_support::{equals, top_is_dense};

#[tokio::test]
async fn a_dense_scan_keeps_its_result_dense() {
    for source in ["[ 3 1 3 1 2 ] UNIQUE", "0 7 RANGE UNIQUE"] {
        assert!(
            top_is_dense(source).await,
            "`{source}` must keep its result in columns"
        );
    }
}

/// First-occurrence order, which is `UNIQUE`'s contract and not merely what
/// a `HashMap` happens to produce — so the expectations below are in an
/// order a sorted or hash order would get wrong.
#[tokio::test]
async fn a_dense_scan_reports_distinct_lanes_in_first_occurrence_order() {
    for (source, expected) in [
        ("[ 3 1 3 1 2 ] UNIQUE", "[ 3 1 2 ]"),
        ("[ 9 5 9 1 5 ] UNIQUE", "[ 9 5 1 ]"),
        ("[ -1 2 -1 ] UNIQUE", "[ -1 2 ]"),
        ("[ 7 7 7 ] UNIQUE", "[ 7 ]"),
        ("0 4 RANGE UNIQUE", "[ 0 1 2 3 4 ]"),
    ] {
        assert_eq!(
            equals(source, expected).await,
            Some(true),
            "`{source}` must equal `{expected}`"
        );
    }
}

/// `TALLY` counts in the order `UNIQUE` reports: its `KEYS` are `UNIQUE`'s
/// answer and its `VALUES` the aligned counts (LANG.RECORDS.STRUCTURE).
#[tokio::test]
async fn a_dense_scan_counts_in_the_order_unique_reports() {
    for (source, expected) in [
        ("[ 3 1 3 1 2 ] TALLY", "[ 3 1 2 ] [ 2 2 1 ] RECORD"),
        ("[ 9 5 9 1 5 ] TALLY", "[ 9 5 1 ] [ 2 2 1 ] RECORD"),
        ("[ 7 7 7 ] TALLY", "[ 7 ] [ 3 ] RECORD"),
        ("0 4 RANGE TALLY", "[ 0 1 2 3 4 ] [ 1 1 1 1 1 ] RECORD"),
    ] {
        assert_eq!(
            equals(source, expected).await,
            Some(true),
            "`{source}` must equal `{expected}`"
        );
    }
    // The pairing itself: as many counts as distinct values, and they sum
    // back to the input length.
    assert_eq!(
        equals("[ 3 1 3 1 2 ] TALLY KEYS", "[ 3 1 3 1 2 ] UNIQUE").await,
        Some(true),
        "TALLY's keys must be exactly what UNIQUE reports"
    );
    assert_eq!(
        equals("[ 3 1 3 1 2 ] TALLY VALUES 0 [ ADD ] FOLD", "5").await,
        Some(true),
        "the counts must sum to the input length"
    );
}

/// The two routes agree on the same integers *in the same order*. `MAP`
/// leaves a dense `Tensor`; `CONCAT` with a boxed operand (the empty literal)
/// does not promote, so the right-hand side is the same sequence as a nested
/// `Vector` and takes the boxed scan
/// the dense one declines.
///
/// The order matters to the comparison, not just to the contract: an earlier
/// draft of this test reversed the dense side, and `UNIQUE` rightly answered
/// `[ 2 1 0 6 5 4 3 ]` against `[ 0 1 2 3 4 5 6 ]` — first occurrence is
/// first occurrence *in the input*, so two different sequences are entitled
/// to two different answers and the test was the thing that was wrong.
#[tokio::test]
async fn the_two_routes_answer_alike() {
    for word in ["UNIQUE", "TALLY"] {
        assert_eq!(
            equals(
                &format!("0 99 RANGE [ 1/7 MUL FLOOR ] MAP {word}"),
                &format!(
                    "0 49 RANGE [ 1/7 MUL FLOOR ] MAP \
                     50 99 RANGE [ 1/7 MUL FLOOR ] MAP CONCAT [ ] CONCAT {word}"
                )
            )
            .await,
            Some(true),
            "`{word}` must answer alike on both routes"
        );
    }
}

/// And the order is load-bearing: reversing the input reverses which value
/// is seen first, so a dense `UNIQUE` must report a different order rather
/// than a canonical one.
#[tokio::test]
async fn a_dense_scan_follows_the_inputs_order_not_a_canonical_one() {
    // `(x SUB 5)^2` over 0..9 is 25 16 9 4 1 0 1 4 9 16: not monotone, so
    // neither direction of the input is a sorted order.
    assert!(
        top_is_dense("0 9 RANGE [ 5 SUB 2 POW ] MAP").await,
        "the operand must take the dense route this test is about"
    );
    assert_eq!(
        equals("0 9 RANGE [ 5 SUB 2 POW ] MAP UNIQUE", "[ 25 16 9 4 1 0 ]").await,
        Some(true),
        "first occurrence in the input order"
    );
    assert_eq!(
        equals(
            "0 9 RANGE [ 5 SUB 2 POW ] MAP REVERSE UNIQUE",
            "[ 16 9 4 1 0 25 ]"
        )
        .await,
        Some(true),
        "reversing the input must reorder the distinct values with it"
    );
}

/// Inputs the dense route declines, each for its own reason, must keep the
/// answers the boxed scan gives them: a rational lane is not identified by
/// its numerator, two NILs are the same value only when their reasons agree
/// (and a dense lane's reason lives outside the column), rank above 1
/// reports distinct *rows*, and Text is not numeric at all.
#[tokio::test]
async fn declined_shapes_keep_the_boxed_scans_answers() {
    for (source, expected) in [
        // 1/3 2/3 1 4/3 are four distinct values whose numerators are
        // 1 2 3 4 — and whose *denominators* differ, so a numerator-keyed
        // scan would be answering a different question.
        ("1 4 RANGE [ 3 DIV ] MAP UNIQUE", "1 4 RANGE [ 3 DIV ] MAP"),
        ("[ 1 NIL 1 ] UNIQUE", "[ 1 NIL ]"),
        ("[ 'b' 'a' 'b' ] UNIQUE", "[ 'b' 'a' ]"),
        ("[ [ 1 2 ] [ 1 2 ] [ 3 4 ] ] UNIQUE", "[ [ 1 2 ] [ 3 4 ] ]"),
        ("[ 1 NIL 1 ] TALLY", "[ 1 NIL ] [ 2 1 ] RECORD"),
        ("[ 'b' 'a' 'b' ] TALLY", "[ 'b' 'a' ] [ 2 1 ] RECORD"),
    ] {
        assert_eq!(
            equals(source, expected).await,
            Some(true),
            "`{source}` must equal `{expected}`"
        );
    }
}
