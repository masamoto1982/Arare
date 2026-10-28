//! The reciprocal route against the `BigInt` operators, digit patterns and
//! divisors chosen to meet every branch: a divisor with and without a
//! normalizing shift, a dividend of one digit and of many, digits of all ones
//! and of zeros, a negative dividend, a quotient of zero.

use super::*;
use num_bigint::Sign;
use num_integer::Integer;
use num_traits::Signed;
use proptest::prelude::*;

fn digit() -> impl Strategy<Value = u64> {
    prop_oneof![
        3 => any::<u64>(),
        1 => Just(0u64),
        1 => Just(u64::MAX),
        1 => Just(1u64 << 63),
        1 => Just(1u64),
    ]
}

fn wide() -> impl Strategy<Value = BigInt> {
    (prop::collection::vec(digit(), 0..9), any::<bool>()).prop_map(|(digits, negative)| {
        let halves: Vec<u32> = digits
            .iter()
            .flat_map(|d| [*d as u32, (*d >> 32) as u32])
            .collect();
        let magnitude = BigInt::new(Sign::Plus, halves);
        if negative {
            -magnitude
        } else {
            magnitude
        }
    })
}

fn divisor() -> impl Strategy<Value = u64> {
    prop_oneof![
        3 => 1u64..100_000,
        2 => any::<u64>().prop_filter("nonzero", |d| *d != 0),
        1 => Just(1u64),
        1 => Just(2u64),
        1 => Just(u64::MAX),
        1 => Just(1u64 << 63),
        1 => Just((1u64 << 63) + 1),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20000))]

    #[test]
    fn remainder_and_quotient_are_the_operators(a in wide(), d in divisor()) {
        let divisor = BigInt::from(d);
        prop_assert_eq!(BigInt::from(rem_word(&a, d)), a.abs() % &divisor);
        prop_assert_eq!(div_word(&a, d), &a / &divisor);
        prop_assert_eq!(quotient(&a, &divisor), &a / &divisor);
    }

    #[test]
    fn the_word_gcd_route_is_the_gcd(a in wide(), d in divisor()) {
        let divisor = BigInt::from(d);
        match gcd_with_word(&a, &divisor) {
            Some(g) => prop_assert_eq!(g, a.gcd(&divisor)),
            None => prop_assert!(a.bits() <= WIDE_BITS),
        }
    }

    #[test]
    fn the_binary_word_gcd_is_the_gcd(a in any::<u64>(), b in any::<u64>()) {
        prop_assert_eq!(BigInt::from(gcd_word(a, b)), BigInt::from(a).gcd(&BigInt::from(b)));
    }
}

#[test]
fn a_divisor_of_more_than_one_word_or_zero_is_not_a_word() {
    assert_eq!(single_word(&BigInt::from(0)), None);
    assert_eq!(single_word(&(BigInt::from(1) << 64)), None);
    assert_eq!(single_word(&((BigInt::from(1) << 64) - 1)), Some(u64::MAX));
    assert_eq!(single_word(&BigInt::from(-5)), Some(5));
}

/// A long sum of unit fractions, whose accumulator is thousands of bits wide
/// and whose terms are one word: every step takes the reciprocal route. The
/// sum must be what the schoolbook form (cross-multiply, one normalizing gcd)
/// gives at each step, for a sum, a difference and a sum with a common factor.
#[test]
fn a_long_running_sum_is_the_schoolbook_sum() {
    use crate::types::fraction::Fraction;
    let schoolbook = |a: &Fraction, b: &Fraction, subtract: bool| {
        let (an, ad) = a.to_bigint_pair();
        let (bn, bd) = b.to_bigint_pair();
        let cross = &bn * &ad;
        let num = if subtract {
            &an * &bd - cross
        } else {
            &an * &bd + cross
        };
        Fraction::new(num, &ad * &bd)
    };
    for (numerator, step) in [(1i64, 1i64), (6, 1), (1, 7)] {
        let (mut fast_sum, mut slow_sum) = (Fraction::from(0), Fraction::from(0));
        let (mut fast_diff, mut slow_diff) = (Fraction::from(0), Fraction::from(0));
        for i in (1..=400i64).map(|i| i * step) {
            let term = Fraction::new(BigInt::from(numerator), BigInt::from(i));
            fast_sum = fast_sum.add(&term);
            slow_sum = schoolbook(&slow_sum, &term, false);
            fast_diff = fast_diff.sub(&term);
            slow_diff = schoolbook(&slow_diff, &term, true);
            assert_eq!(fast_sum, slow_sum, "sum at {i}");
            assert_eq!(fast_diff, slow_diff, "difference at {i}");
        }
        assert!(
            slow_sum.denominator().bits() > 400,
            "the accumulator is wide"
        );
    }
}
