//! Stein's binary gcd (`binary_gcd_u64` and its 128-bit form) against
//! Euclid's, which the normalizer used before it. The gcd is unique, so the
//! two must agree on every pair; Euclid is the oracle, not the property.

use super::fraction::{binary_gcd_u64, compute_gcd_i64, Fraction};
use proptest::prelude::*;

fn euclid(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

#[test]
fn edge_pairs_agree() {
    for (a, b) in [
        (0, 0),
        (0, 7),
        (7, 0),
        (1, u64::MAX),
        (u64::MAX, u64::MAX),
        (1 << 63, 1 << 63),
        (1 << 63, 6),
        (48, 18),
        (17, 17),
    ] {
        assert_eq!(
            u128::from(binary_gcd_u64(a, b)),
            euclid(a.into(), b.into()),
            "gcd({a}, {b})"
        );
    }
    // The one pair whose gcd does not fit i64 keeps wrapping, as it did.
    assert_eq!(compute_gcd_i64(i64::MIN, i64::MIN), i64::MIN);
}

proptest! {
    #[test]
    fn binary_gcd_agrees_with_euclid(a in any::<u64>(), b in any::<u64>(), shift in 0u32..20) {
        let (a, b) = (a >> (shift % 64), b << (shift % 8));
        prop_assert_eq!(u128::from(binary_gcd_u64(a, b)), euclid(a.into(), b.into()));
    }

    /// The `i128` normalizer, through its two paths (both halves in a
    /// machine word, or not), against the `BigInt` one.
    #[test]
    fn create_from_i128_agrees_with_new(
        n in any::<i64>(),
        m in any::<i64>(),
        d in any::<i64>().prop_filter("nonzero", |d| *d != 0),
        e in 1i64..1_000_000,
    ) {
        use num_bigint::BigInt;
        for (num, den) in [
            (i128::from(n), i128::from(d)),
            (i128::from(n) * i128::from(m), i128::from(d) * i128::from(e)),
        ] {
            let fast = Fraction::create_from_i128(num, den);
            let oracle = Fraction::new(BigInt::from(num), BigInt::from(den));
            prop_assert_eq!(fast.to_bigint_pair(), oracle.to_bigint_pair());
            prop_assert_eq!(fast.is_small(), oracle.is_small());
        }
    }
}

/// A reduced pair with a positive denominator, drawn to reach the edges of a
/// machine word as well as everyday values.
fn small_pair() -> impl Strategy<Value = (i64, i64)> {
    let half = prop_oneof![
        4 => -50i64..50,
        1 => Just(i64::MAX),
        1 => Just(i64::MIN),
        1 => any::<i64>(),
        1 => 1i64..1 << 31,
        // Products of these straddle a machine word, which is where the
        // word-sized routes hand over to the widened ones.
        1 => prop_oneof![
            Just(3_037_000_499i64),
            Just(3_037_000_500i64),
            Just(-3_037_000_500i64),
            Just(1i64 << 32),
            Just(-(1i64 << 32)),
            Just(1i64 << 62),
            Just(-(1i64 << 62)),
            Just(i64::MIN + 1),
            Just(-1i64),
        ],
    ];
    (half.clone(), half).prop_filter_map("a denominator", |(n, d)| {
        if d == 0 {
            return None;
        }
        let f = Fraction::new(num_bigint::BigInt::from(n), num_bigint::BigInt::from(d));
        f.extract_i64_pair()
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16384))]

    /// `small_rational` against `Fraction`: where it answers, the same pair;
    /// where it declines, a result `Fraction` could not hold as a pair either
    /// (or a zero divisor).
    #[test]
    fn small_rational_agrees_with_fraction(a in small_pair(), b in small_pair()) {
        use super::small_rational::{add, div, mul, order};
        let fa = Fraction::from_normalized_pair(a.0, a.1);
        let fb = Fraction::from_normalized_pair(b.0, b.1);
        let cases = [
            (add(a, b, false), Some(fa.add(&fb))),
            (add(a, b, true), Some(fa.sub(&fb))),
            (mul(a, b), Some(fa.mul(&fb))),
            (div(a, b), (b.0 != 0).then(|| fa.div(&fb))),
        ];
        for (fast, oracle) in cases {
            let oracle = oracle.and_then(|f| f.extract_i64_pair());
            prop_assert_eq!(fast, oracle, "{:?} and {:?}", a, b);
        }
        let ordering = if fa.lt(&fb) {
            std::cmp::Ordering::Less
        } else if fa.gt(&fb) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        };
        prop_assert_eq!(order(a, b), ordering);
    }
}

/// The total forms of `small_rational` against `Fraction` on every pair of
/// a set that holds the two points over zero, zero, signs, a non-integer
/// and the ends of a machine word: the same pair wherever `Fraction`'s
/// answer fits one, and the same order, `None` exactly where `Fraction`
/// orders nothing.
#[test]
fn small_rational_total_forms_agree_with_fraction() {
    use super::small_rational::{add_total, div_total, mul_total, order_total};
    let pairs = [
        (0, 1),
        (1, 1),
        (-1, 1),
        (3, 4),
        (-1, 2),
        (i64::MAX, 1),
        (i64::MIN + 1, 1),
        (1, 0),
        (0, 0),
    ];
    for a in pairs {
        for b in pairs {
            let fa = Fraction::from_normalized_pair(a.0, a.1);
            let fb = Fraction::from_normalized_pair(b.0, b.1);
            let cases = [
                ("ADD", add_total(a, b, false), fa.add(&fb)),
                ("SUB", add_total(a, b, true), fa.sub(&fb)),
                ("MUL", mul_total(a, b), fa.mul(&fb)),
                ("DIV", div_total(a, b), fa.div(&fb)),
            ];
            for (word, fast, oracle) in cases {
                assert_eq!(fast, oracle.extract_i64_pair(), "{a:?} {b:?} {word}");
            }
            assert_eq!(order_total(a, b), fa.order(&fb), "{a:?} {b:?} order");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16384))]

    /// The total forms agree with `Fraction` on random pairs and the two
    /// points over zero.
    #[test]
    fn small_rational_total_forms_agree_on_random_pairs(
        a in prop_oneof![4 => small_pair(), 1 => over_zero()],
        b in prop_oneof![4 => small_pair(), 1 => over_zero()],
    ) {
        use super::small_rational::{add_total, div_total, mul_total, order_total};
        let fa = Fraction::from_normalized_pair(a.0, a.1);
        let fb = Fraction::from_normalized_pair(b.0, b.1);
        prop_assert_eq!(add_total(a, b, false), fa.add(&fb).extract_i64_pair());
        prop_assert_eq!(add_total(a, b, true), fa.sub(&fb).extract_i64_pair());
        prop_assert_eq!(mul_total(a, b), fa.mul(&fb).extract_i64_pair());
        prop_assert_eq!(div_total(a, b), fa.div(&fb).extract_i64_pair());
        prop_assert_eq!(order_total(a, b), fa.order(&fb));
    }
}

fn over_zero() -> impl Strategy<Value = (i64, i64)> {
    prop_oneof![Just((1, 0)), Just((0, 0))]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16384))]

    /// The wasm32 multiply (`small_rational::narrow_first_mul`) answers
    /// exactly what `i64::overflowing_mul` does, at and around the 32-bit
    /// boundary where it switches routes and at the ends of a machine word.
    #[test]
    fn narrow_first_mul_is_overflowing_mul(a in mul_factor(), b in mul_factor()) {
        prop_assert_eq!(
            super::small_rational::narrow_first_mul(a, b),
            a.overflowing_mul(b)
        );
    }
}

fn mul_factor() -> impl Strategy<Value = i64> {
    let edge = prop_oneof![
        Just(i64::from(i32::MAX)),
        Just(i64::from(i32::MAX) + 1),
        Just(i64::from(i32::MIN)),
        Just(i64::from(i32::MIN) - 1),
        Just(i64::MAX),
        Just(i64::MIN),
        Just(0i64),
        Just(-1i64),
        Just(3_037_000_500i64),
    ];
    prop_oneof![
        2 => edge.clone(),
        2 => (edge, -3i64..4).prop_map(|(e, d)| e.wrapping_add(d)),
        1 => any::<i64>(),
        1 => any::<i32>().prop_map(i64::from),
    ]
}
