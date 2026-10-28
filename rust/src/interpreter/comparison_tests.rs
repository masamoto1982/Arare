//! Two rational operands compare as the rationals they are.
//!
//! `three_way_compare`, `compare_scalar_pair` and `scalar_pair_eq` each ended in
//! a `Fraction` comparison whenever both operands were rational — their
//! `(Some, Some)` arm — but reached it by building an `ExactReal` from each
//! operand first, and `extract_exact_real_for_comparison` clones the `Fraction`
//! out of the `Value` to do that. Two clones and two constructions per
//! comparison, to arrive at the two `Fraction`s the operands already held.
//!
//! `rational_pair` borrows them instead. That is only sound if the round trip it
//! skips is the identity — `ExactReal::from_fraction(f).as_rational() == Some(f)`
//! — and only *complete* if nothing but a plain rational takes the screen. Both
//! are pinned here, the first directly and the second by leaving every other
//! shape to answer through the route that can: an algebraic pair decides exactly
//! (√8 vs √2+√2, LANG.VALUES.EXACT).

use crate::test_support::answer;
use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;

/// The round trip the screen skips is the identity, over a dense range of
/// rationals including both signs, zero, integers, and the i64 extremes.
#[test]
fn a_rational_survives_the_exact_real_round_trip_it_no_longer_takes() {
    let mut fractions: Vec<Fraction> = Vec::new();
    for numerator in -40..=40 {
        for denominator in 1..=12 {
            fractions.push(Fraction::new(numerator.into(), denominator.into()));
        }
    }
    for n in [i64::MAX, i64::MIN + 1, i64::MIN, 0, 1, -1] {
        fractions.push(Fraction::from(n));
    }

    for fraction in &fractions {
        let exact = ExactReal::from_fraction(fraction.clone());
        let round_tripped = exact.as_rational();
        assert_eq!(
            round_tripped,
            Some(fraction),
            "{fraction} must survive from_fraction/as_rational unchanged"
        );
    }

    // And the order the screen reads is the order the general arm read.
    for a in fractions.iter().take(60) {
        for b in fractions.iter().take(60) {
            let screened = a.order(b);
            let (ea, eb) = (
                ExactReal::from_fraction(a.clone()),
                ExactReal::from_fraction(b.clone()),
            );
            let general = ea.cmp_exact(&eb);
            assert_eq!(screened, general, "comparing {a} with {b}");
        }
    }
}

/// Every ordering word, over the sign and magnitude cases a rational screen
/// could get wrong: equal values, both signs, a negative denominator's worth
/// of sign placement, and unequal denominators.
#[tokio::test]
async fn every_ordering_word_answers_the_same_for_rationals() {
    for (program, expected) in [
        ("2 3 LT", "TRUE"),
        ("3 2 LT", "FALSE"),
        ("2 2 LT", "FALSE"),
        ("2 2 GT NOT", "TRUE"),
        ("3 2 GT", "TRUE"),
        ("2 3 GT", "FALSE"),
        ("2 2 LT NOT", "TRUE"),
        ("2 2 EQ", "TRUE"),
        ("2 3 EQ", "FALSE"),
        ("-2 3 LT", "TRUE"),
        ("3 -2 LT", "FALSE"),
        ("-3 -2 LT", "TRUE"),
        ("-2 -3 LT", "FALSE"),
        ("0 0 EQ", "TRUE"),
        ("0 -0 EQ", "TRUE"),
        ("1/2 1/3 GT", "TRUE"),
        ("1/3 1/2 GT", "FALSE"),
        ("2/4 1/2 EQ", "TRUE"),
        ("-1/2 1/2 LT", "TRUE"),
    ] {
        assert_eq!(answer(program).await, expected, "`{program}`");
    }
}

/// An exact operand must not take the screen. √8 and √2+√2 are the same
/// value by different histories, which only the exact route decides, and π
/// is the case that may honestly not decide at all.
#[tokio::test]
async fn an_exact_operand_still_decides_through_the_exact_route() {
    assert_eq!(answer("8 SQRT 2 SQRT 2 SQRT ADD EQ").await, "TRUE");
    assert_eq!(answer("2 SQRT 1 GT").await, "TRUE");
    assert_eq!(answer("2 SQRT 2 LT").await, "TRUE");
    // A rational compared against an algebraic is still a mixed pair.
    assert_eq!(answer("1 2 SQRT LT").await, "TRUE");
}
