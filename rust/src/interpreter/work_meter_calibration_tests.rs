//! The work meter's price has to track what an operation actually costs.
//!
//! `max_numeric_work` is a stand-in for "too long", so a unit only means
//! anything if it buys roughly the same amount of time on every path. When it
//! does not, the ceiling silently becomes a different control depending on
//! which path a program takes — which is how a 4096-digit *addition* came to
//! be charged 213 units while the same-width multiplication was charged 45,369
//! and ran in half the time.
//!
//! These are ratio assertions over charged units, never wall clocks: a timing
//! test in CI is flaky, and the absolute rates belong to one machine and one
//! `num-bigint`. `examples/work_meter_calibration` is where the rates are
//! measured; this is where the shape of the pricing is pinned so a re-measure
//! is a deliberate act rather than a surprise.

use crate::test_support::charged_by;

/// 4096 digits — the widest single literal `numericLiteralDigits` admits,
/// so the widest operand a source can state outright.
fn wide() -> String {
    "9".repeat(4096)
}

#[tokio::test]
async fn a_wide_addition_costs_what_a_wide_multiplication_costs() {
    // `a/b ADD c/d` is `(ad ADD cb)/(bd)` followed by a gcd: three
    // multiplications and a Euclid, none of them linear in the wider
    // operand. Priced as linear it charged 213 against multiplication's
    // 45,369 — a 200x gap on an operation measurably *dearer* than the one
    // it was being undercut by (745 µs against 366 µs on the reference
    // container).
    let wide = wide();
    let addition = charged_by(&format!("{wide} {wide} ADD")).await;
    let multiplication = charged_by(&format!("{wide} {wide} MUL")).await;
    assert_eq!(
        addition, multiplication,
        "the same operand widths must cost the same whichever schema runs"
    );
}

#[tokio::test]
async fn subtraction_and_division_are_priced_with_their_partners() {
    let wide = wide();
    let addition = charged_by(&format!("{wide} {wide} ADD")).await;
    for source in [format!("{wide} {wide} SUB"), format!("{wide} {wide} DIV")] {
        assert_eq!(
            charged_by(&source).await,
            addition,
            "every schema at one width is one price"
        );
    }
}

#[tokio::test]
async fn width_dominates_the_price_of_an_addition() {
    // The property the linear price destroyed: a wide addition has to be
    // recognisably dearer than a narrow one, or a chain of them is bounded
    // only by the step budget — which counts words, not the size of the
    // numbers in them.
    let narrow = charged_by("2 3 ADD").await;
    let wide = charged_by(&format!("{} {} ADD", wide(), wide())).await;
    assert!(
        wide > narrow.saturating_mul(10_000),
        "a 4096-digit addition must dwarf a one-digit one, got {wide} against {narrow}"
    );
}

#[tokio::test]
async fn shape_does_not_change_the_price() {
    // The same guarantee `arithmetic_meter` exists for, restated at the
    // widths this module cares about: a broadcast is `lanes` of the scalar
    // price and nothing else.
    let wide = wide();
    let scalar = charged_by(&format!("{wide} {wide} ADD")).await;
    let one_lane = charged_by(&format!("[ {wide} ] {wide} ADD")).await;
    let three_lanes = charged_by(&format!("[ {wide} {wide} {wide} ] {wide} ADD")).await;
    assert_eq!(
        scalar, one_lane,
        "a one-lane vector is one scalar operation"
    );
    assert_eq!(
        three_lanes,
        scalar * 3,
        "three lanes are three of them, exactly"
    );
}

#[tokio::test]
async fn ordinary_arithmetic_is_still_far_below_the_ceiling() {
    // Repricing addition must not turn everyday work into a refusal. The
    // agent profile's ceiling is 10,000,000.
    for source in [
        "1 3 DIV",
        "0.1 0.2 ADD",
        "0 999 RANGE 0 [ ADD ] FOLD",
        "[ 1 2 3 ] [ 4 5 6 ] MUL",
        "2 SQRT 3 SQRT ADD 5 SQRT 7 SQRT ADD MUL",
    ] {
        let charged = charged_by(source).await;
        assert!(
            charged < 100_000,
            "`{source}` charged {charged} units, which is not ordinary"
        );
    }
}

/// An algebraic `FORMAT` refines every term to the scaled width, so its price
/// follows the term count and the square of the width, as an algebraic
/// product's does. At one unit a digit, six square roots at 65,536 places
/// ran for 7.3 s on 0.86% of the agent profile's budget.
#[tokio::test]
async fn an_algebraic_format_is_priced_by_its_terms_and_width() {
    const SIX: &str = "2 SQRT 3 SQRT ADD 5 SQRT ADD 7 SQRT ADD 11 SQRT ADD 13 SQRT ADD";
    // What building the operand charged is not the FORMAT's.
    let building = charged_by(SIX).await;
    let narrow = charged_by(&format!("{SIX} 1024 FORMAT")).await - building;
    let wide = charged_by(&format!("{SIX} 4096 FORMAT")).await - building;
    assert!(
        wide >= 12 * narrow,
        "four times the places is about sixteen times the work: {narrow} -> {wide}"
    );
    let one_term = charged_by("2 SQRT 4096 FORMAT").await - charged_by("2 SQRT").await;
    assert!(
        wide >= 3 * one_term,
        "six terms cost more than one: {one_term} -> {wide}"
    );
    // A rational still scales and rounds once.
    assert!(charged_by("1/3 4096 FORMAT").await <= 4097);
}
