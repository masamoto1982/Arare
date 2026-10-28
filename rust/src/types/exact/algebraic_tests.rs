//! Property tests for the Tier 1 algebraic normal form: ring axioms over
//! sampled values, √r·√r = r, eager demotion to Tier 0, semantic
//! normal-form uniqueness across construction histories, decidable
//! comparison, and the derived CF against known expansions.

use crate::test_support::frac;
use crate::types::exact::algebraic::{Algebraic, AlgebraicResult};
use crate::types::fraction::Fraction;
use num_bigint::BigInt;
use std::cmp::Ordering;

fn sqrt_irr(n: i64, d: i64) -> Algebraic {
    match Algebraic::sqrt_of_fraction(&frac(n, d)) {
        Some(AlgebraicResult::Irrational(a)) => a,
        other => panic!("√({n}/{d}) should be irrational, got {other:?}"),
    }
}

/// A small pool of Tier 1 values with varied bases and signs, for the
/// axiom checks below.
fn samples() -> Vec<Algebraic> {
    let sqrt2 = sqrt_irr(2, 1);
    let sqrt3 = sqrt_irr(3, 1);
    let sqrt_half = sqrt_irr(1, 2);
    let sum = match sqrt2.add(&sqrt3) {
        AlgebraicResult::Irrational(a) => a,
        other => panic!("√2+√3 is irrational, got {other:?}"),
    };
    let shifted = match sqrt2.add_fraction(&frac(-7, 3)) {
        AlgebraicResult::Irrational(a) => a,
        other => panic!("√2−7/3 is irrational, got {other:?}"),
    };
    vec![sqrt2, sqrt3, sqrt_half, sum, shifted]
}

fn as_result(a: &Algebraic) -> AlgebraicResult {
    AlgebraicResult::Irrational(a.clone())
}

fn results_equal(a: &AlgebraicResult, b: &AlgebraicResult) -> bool {
    match (a, b) {
        (AlgebraicResult::Rational(x), AlgebraicResult::Rational(y)) => x == y,
        (AlgebraicResult::Irrational(x), AlgebraicResult::Irrational(y)) => x == y,
        // Mixed shapes cannot be equal: demotion is eager, so a rational
        // never hides inside an `Irrational`.
        _ => false,
    }
}

fn add_results(a: &AlgebraicResult, b: &AlgebraicResult) -> AlgebraicResult {
    match (a, b) {
        (AlgebraicResult::Rational(x), AlgebraicResult::Rational(y)) => {
            AlgebraicResult::Rational(x.add(y))
        }
        (AlgebraicResult::Rational(x), AlgebraicResult::Irrational(y))
        | (AlgebraicResult::Irrational(y), AlgebraicResult::Rational(x)) => y.add_fraction(x),
        (AlgebraicResult::Irrational(x), AlgebraicResult::Irrational(y)) => x.add(y),
    }
}

fn mul_results(a: &AlgebraicResult, b: &AlgebraicResult) -> AlgebraicResult {
    match (a, b) {
        (AlgebraicResult::Rational(x), AlgebraicResult::Rational(y)) => {
            AlgebraicResult::Rational(x.mul(y))
        }
        (AlgebraicResult::Rational(x), AlgebraicResult::Irrational(y))
        | (AlgebraicResult::Irrational(y), AlgebraicResult::Rational(x)) => y.mul_fraction(x),
        (AlgebraicResult::Irrational(x), AlgebraicResult::Irrational(y)) => x.mul(y),
    }
}

#[test]
fn ring_axioms_hold_over_samples() {
    let pool = samples();
    for a in &pool {
        for b in &pool {
            // Commutativity.
            assert!(results_equal(&a.add(b), &b.add(a)), "a+b EQ b+a");
            assert!(results_equal(&a.mul(b), &b.mul(a)), "a·b = b·a");
            for c in &pool {
                // Associativity.
                let left = add_results(&a.add(b), &as_result(c));
                let right = add_results(&as_result(a), &b.add(c));
                assert!(results_equal(&left, &right), "(a+b)+c EQ a+(b+c)");
                let left = mul_results(&a.mul(b), &as_result(c));
                let right = mul_results(&as_result(a), &b.mul(c));
                assert!(results_equal(&left, &right), "(a·b)·c = a·(b·c)");
                // Distributivity.
                let left = mul_results(&as_result(a), &b.add(c));
                let right = add_results(&a.mul(b), &a.mul(c));
                assert!(results_equal(&left, &right), "a·(b+c) = a·b + a·c");
            }
        }
    }
}

#[test]
fn additive_and_multiplicative_inverses_cancel() {
    for a in &samples() {
        // a + (−a) = 0, demoted to the Tier 0 zero.
        match a.add(&a.neg()) {
            AlgebraicResult::Rational(f) => assert!(f.is_zero()),
            other => panic!("a + (−a) should demote to 0, got {other:?}"),
        }
        // a · a⁻¹ = 1.
        let product = match a.reciprocal() {
            AlgebraicResult::Rational(f) => a.mul_fraction(&f),
            AlgebraicResult::Irrational(inv) => a.mul(&inv),
        };
        match product {
            AlgebraicResult::Rational(f) => assert_eq!(f, frac(1, 1)),
            other => panic!("a · a⁻¹ should demote to 1, got {other:?}"),
        }
    }
}

#[test]
fn sqrt_times_itself_recovers_radicand() {
    for (n, d) in [(2, 1), (3, 1), (8, 1), (1, 2), (5, 7)] {
        let r = sqrt_irr(n, d);
        match r.mul(&r) {
            AlgebraicResult::Rational(f) => assert_eq!(f, frac(n, d), "√r·√r = r for {n}/{d}"),
            other => panic!("√r·√r should demote to the rational r, got {other:?}"),
        }
    }
}

#[test]
fn sqrt_normalizes_like_the_historical_constructor() {
    // Perfect squares and zero demote to Tier 0.
    assert_eq!(
        Algebraic::sqrt_of_fraction(&frac(9, 4)),
        Some(AlgebraicResult::Rational(frac(3, 2)))
    );
    assert_eq!(
        Algebraic::sqrt_of_fraction(&frac(0, 1)),
        Some(AlgebraicResult::Rational(frac(0, 1)))
    );
    // A negative radicand is rejected; 1/0 and 0/0 are their own roots.
    assert_eq!(Algebraic::sqrt_of_fraction(&frac(-2, 1)), None);
    assert_eq!(
        Algebraic::sqrt_of_fraction(&Fraction::infinity()),
        Some(AlgebraicResult::Rational(Fraction::infinity()))
    );
    assert_eq!(
        Algebraic::sqrt_of_fraction(&Fraction::nullity()),
        Some(AlgebraicResult::Rational(Fraction::nullity()))
    );
}

#[test]
fn demotion_is_eager_famous_identity() {
    // (1+√2)(√2−1) = 1: the Gosper-era value that could not even show
    // its leading CF term now demotes to the Tier 0 rational 1/1.
    let sqrt2 = sqrt_irr(2, 1);
    let one_plus = match sqrt2.add_fraction(&frac(1, 1)) {
        AlgebraicResult::Irrational(a) => a,
        other => panic!("1+√2 is irrational, got {other:?}"),
    };
    let minus_one = match sqrt2.add_fraction(&frac(-1, 1)) {
        AlgebraicResult::Irrational(a) => a,
        other => panic!("√2−1 is irrational, got {other:?}"),
    };
    match one_plus.mul(&minus_one) {
        AlgebraicResult::Rational(f) => assert_eq!(f, frac(1, 1)),
        other => panic!("(1+√2)(√2−1) should demote to 1/1, got {other:?}"),
    }
}

#[test]
fn normal_form_identity_survives_construction_history() {
    // √8 built directly (coarse basis {8}) equals 2·√2 (basis {2}) —
    // uniqueness is semantic, guaranteed by rebasing, not structural.
    let sqrt8 = sqrt_irr(8, 1);
    let two_sqrt2 = match sqrt_irr(2, 1).mul_fraction(&frac(2, 1)) {
        AlgebraicResult::Irrational(a) => a,
        other => panic!("2√2 is irrational, got {other:?}"),
    };
    assert_eq!(sqrt8, two_sqrt2);
    assert_eq!(sqrt8.cmp(&two_sqrt2), Ordering::Equal);
    // And √12 vs 2·√3 through an additive detour.
    let sqrt12 = sqrt_irr(12, 1);
    let detour = match sqrt_irr(3, 1).add(&sqrt_irr(3, 1)) {
        AlgebraicResult::Irrational(a) => a,
        other => panic!("√3+√3 is irrational, got {other:?}"),
    };
    assert_eq!(sqrt12, detour);
}

#[test]
fn comparison_is_total_and_budget_free() {
    let sqrt2 = sqrt_irr(2, 1);
    let sqrt3 = sqrt_irr(3, 1);
    assert_eq!(sqrt2.cmp(&sqrt3), Ordering::Less);
    assert_eq!(sqrt3.cmp(&sqrt2), Ordering::Greater);
    assert_eq!(sqrt2.cmp(&sqrt2), Ordering::Equal);
    // Against rationals, including the √2 < 2 acceptance criterion.
    assert_eq!(sqrt2.cmp_fraction(&frac(2, 1)), Ordering::Less);
    assert_eq!(sqrt2.cmp_fraction(&frac(1, 1)), Ordering::Greater);
    // A pair whose CF streams agree for many terms still decides:
    // √8 vs √2+√2 (equal values through different histories).
    let sqrt8 = sqrt_irr(8, 1);
    let doubled = match sqrt_irr(2, 1).add(&sqrt_irr(2, 1)) {
        AlgebraicResult::Irrational(a) => a,
        other => panic!("√2+√2 is irrational, got {other:?}"),
    };
    assert_eq!(sqrt8.cmp(&doubled), Ordering::Equal);
    // Sign of a multi-term difference: √2+√3 − 3 < 0 < √2+√3 − 3.14…?
    let sum = match sqrt2.add(&sqrt3) {
        AlgebraicResult::Irrational(a) => a,
        other => panic!("√2+√3 is irrational, got {other:?}"),
    };
    assert_eq!(sum.cmp_fraction(&frac(3, 1)), Ordering::Greater);
    assert_eq!(sum.cmp_fraction(&frac(315, 100)), Ordering::Less);
}

#[test]
fn floor_and_round_are_exact() {
    let sqrt2 = sqrt_irr(2, 1);
    assert_eq!(sqrt2.floor_int(), BigInt::from(1));
    assert_eq!(sqrt2.round_int(), BigInt::from(1));
    let neg = sqrt2.neg();
    assert_eq!(neg.floor_int(), BigInt::from(-2));
    assert_eq!(neg.round_int(), BigInt::from(-1));
    // √3 ≈ 1.732 rounds up.
    assert_eq!(sqrt_irr(3, 1).round_int(), BigInt::from(2));
}

#[test]
fn best_rational_approximation_returns_principal_convergents() {
    let sqrt2 = sqrt_irr(2, 1);
    // Convergents of √2: 1, 3/2, 7/5, 17/12, 41/29, 99/70, …
    assert_eq!(
        sqrt2.best_rational_approximation(&BigInt::from(1)),
        Some(frac(1, 1))
    );
    assert_eq!(
        sqrt2.best_rational_approximation(&BigInt::from(12)),
        Some(frac(17, 12))
    );
    assert_eq!(
        sqrt2.best_rational_approximation(&BigInt::from(70)),
        Some(frac(99, 70))
    );
    assert_eq!(sqrt2.best_rational_approximation(&BigInt::from(0)), None);
}

#[test]
fn enclosures_narrow_monotonically_around_the_value() {
    let sqrt2 = sqrt_irr(2, 1);
    let (lo1, hi1) = sqrt2.bounds(8);
    assert!(lo1.lt(&hi1), "irrational enclosure is not a point");
    let (lo2, hi2) = sqrt2.bounds(32);
    assert!(lo1 <= lo2 && hi2 <= hi1, "deeper bounds nest");
    assert!(hi2.sub(&lo2).lt(&hi1.sub(&lo1)), "deeper bounds narrow");
    // The enclosure straddles the true value: lo < √2 < hi ⇔ lo² < 2 < hi².
    assert!(lo2.mul(&lo2).lt(&frac(2, 1)));
    assert!(hi2.mul(&hi2).gt(&frac(2, 1)));
}

/// The normal form is the value, and `normal_form_terms` hands it out in the
/// shape a host can draw in one line. This is what lets a Stack area show `√3`
/// instead of choosing between the source-form display and a best
/// rational approximation that looks exactly like an exact rational.
#[test]
fn normal_form_terms_expose_the_stored_representation() {
    let sqrt3 = sqrt_irr(3, 1);
    assert_eq!(
        sqrt3.normal_form_terms(),
        vec![(frac(1, 1), BigInt::from(3))],
        "√3 is one term with coefficient 1"
    );

    // 2√2 — the coefficient rides on the term, not on a separate value.
    let two_root_two = match sqrt_irr(2, 1).mul_fraction(&frac(2, 1)) {
        AlgebraicResult::Irrational(a) => a,
        other => panic!("2·√2 is irrational, got {other:?}"),
    };
    assert_eq!(
        two_root_two.normal_form_terms(),
        vec![(frac(2, 1), BigInt::from(2))]
    );

    // A rational part keys on radicand 1, and terms come back in ascending
    // radicand order so a host renders them in a stable sequence.
    let mixed = match sqrt_irr(5, 1).mul_fraction(&frac(1, 3)) {
        AlgebraicResult::Irrational(a) => match a.add_fraction(&frac(1, 2)) {
            AlgebraicResult::Irrational(b) => b,
            other => panic!("1/2 + 1/3·√5 is irrational, got {other:?}"),
        },
        other => panic!("1/3·√5 is irrational, got {other:?}"),
    };
    assert_eq!(
        mixed.normal_form_terms(),
        vec![(frac(1, 2), BigInt::from(1)), (frac(1, 3), BigInt::from(5))]
    );
}

/// The convergents the floor-and-reciprocate iteration gives, to any depth:
/// the reference the enclosure-based approximation must agree with.
fn convergent_by_reciprocal(value: &Algebraic, max_denominator: &BigInt) -> Fraction {
    let (mut h2, mut h1) = (BigInt::from(0), BigInt::from(1));
    let (mut k2, mut k1) = (BigInt::from(1), BigInt::from(0));
    let mut best = None;
    let mut state = Some(value.clone());
    while let Some(x) = state {
        let a = x.floor_int();
        let h = &a * &h1 + &h2;
        let k = &a * &k1 + &k2;
        if &k > max_denominator {
            break;
        }
        h2 = std::mem::replace(&mut h1, h.clone());
        k2 = std::mem::replace(&mut k1, k.clone());
        best = Some(Fraction::new(h, k));
        state = match x.add_fraction(&Fraction::new(-a, BigInt::from(1))) {
            AlgebraicResult::Irrational(f) => match f.reciprocal() {
                AlgebraicResult::Irrational(next) => Some(next),
                AlgebraicResult::Rational(_) => None,
            },
            AlgebraicResult::Rational(_) => None,
        };
    }
    best.expect("a denominator bound of 1 or more admits the integer part")
}

fn sum_of_roots(radicands: &[i64]) -> Algebraic {
    let mut sum = sqrt_irr(radicands[0], 1);
    for &m in &radicands[1..] {
        sum = match sum.add(&sqrt_irr(m, 1)) {
            AlgebraicResult::Irrational(a) => a,
            other => panic!("a sum of distinct roots is irrational, got {other:?}"),
        };
    }
    sum
}

#[test]
fn the_enclosure_approximation_matches_floor_and_reciprocate() {
    let values = [
        sum_of_roots(&[2]),
        sum_of_roots(&[2, 3]),
        sum_of_roots(&[2, 3, 5]),
        sum_of_roots(&[2, 3, 5, 7]),
        sum_of_roots(&[2, 3, 5, 6, 7]),
        sqrt_irr(2, 1).neg(),
        sqrt_irr(1, 7),
    ];
    for value in &values {
        for bound in [1u64, 2, 12, 70, 1_000, 1_000_000, 1_000_000_000] {
            let bound = BigInt::from(bound);
            assert_eq!(
                value.best_rational_approximation(&bound),
                Some(convergent_by_reciprocal(value, &bound)),
                "{value:?} within {bound}"
            );
        }
    }
}

/// √1 + … + √30 is nineteen terms; its first reciprocal step alone took
/// twenty-one seconds. The enclosure reads it in well under one.
#[test]
fn a_wide_sum_of_roots_is_approximated_in_linear_time() {
    let radicands = [
        2, 3, 5, 6, 7, 10, 11, 13, 14, 15, 17, 19, 21, 22, 23, 26, 29, 30,
    ];
    let sum = sum_of_roots(&radicands);
    let started = std::time::Instant::now();
    let approx = sum
        .best_rational_approximation(&BigInt::from(1_000_000_000u64))
        .expect("a bound of 1e9 admits a convergent");
    assert!(
        started.elapsed().as_millis() < 500,
        "took {:?}",
        started.elapsed()
    );
    let (lo, hi) = sum.bounds(256);
    assert!(
        approx.sub(&lo).abs().lt(&frac(1, 1_000_000))
            && approx.sub(&hi).abs().lt(&frac(1, 1_000_000))
    );
    assert!(approx.denominator() <= BigInt::from(1_000_000_000u64));
}

/// `(√2 − 1)ⁿ` written out as `a − b√2` is tiny, and its coefficients are as
/// wide as `n` is long: the two ends of an enclosure agree on even its sign
/// only once the enclosure is about as fine as the coefficients are wide. The
/// approximation is a convenience beside the value and no meter pays for it,
/// so it answers within its work ceiling — the exact convergent `0/1` while the
/// coefficients are narrow enough, `None` (the caller's termwise fallback)
/// once they are not — and never spends seconds getting there.
#[test]
fn a_value_whose_terms_cancel_is_answered_within_the_work_ceiling() {
    let mut x = match sqrt_irr(2, 1).add_fraction(&frac(-1, 1)) {
        AlgebraicResult::Irrational(a) => a,
        other => panic!("√2 − 1 is irrational, got {other:?}"),
    };
    let bound = BigInt::from(1_000_000_000u64);
    for squarings in 1..=19 {
        x = match x.mul(&x) {
            AlgebraicResult::Irrational(a) => a,
            other => panic!("a power of √2 − 1 is irrational, got {other:?}"),
        };
        let started = std::time::Instant::now();
        let answer = x.best_rational_approximation(&bound);
        assert!(
            started.elapsed().as_secs_f64() < 2.0,
            "(√2 − 1)^(2^{squarings}) took {:?}",
            started.elapsed()
        );
        if squarings <= 4 {
            assert_eq!(
                answer,
                Some(convergent_by_reciprocal(&x, &bound)),
                "(√2 − 1)^(2^{squarings})"
            );
        } else if squarings <= 12 {
            // Below 1e-9 from here, so no convergent past 0/1 fits the bound.
            assert_eq!(answer, Some(frac(0, 1)), "(√2 − 1)^(2^{squarings})");
        } else {
            assert!(answer.is_none() || answer == Some(frac(0, 1)));
        }
    }
}
