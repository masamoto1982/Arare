//! Tests for `fraction_arithmetic`: the balanced gcd against `Integer::gcd`,
//! and Henrici's `add`/`sub` against the schoolbook form.

use super::fraction::Fraction;
use super::fraction_arithmetic::balanced_bigint_gcd;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::Signed;
use proptest::prelude::*;

/// Hand-picked pairs covering the shapes the doc comment's measurements
/// used: a wide operand against 0, against a small denominator (1, 7),
/// and against another wide operand of comparable size.
#[test]
fn matches_direct_gcd_on_the_measured_shapes() {
    let wide: BigInt = "9".repeat(4096).parse().unwrap();
    let wide_other: BigInt = "7".repeat(4096).parse().unwrap();
    for (a, b) in [
        (wide.clone(), BigInt::from(0)),
        (BigInt::from(0), wide.clone()),
        (wide.clone(), BigInt::from(1)),
        (wide.clone(), BigInt::from(7)),
        (wide.clone(), wide_other.clone()),
        (wide_other, wide),
        (BigInt::from(0), BigInt::from(0)),
        (BigInt::from(-462), BigInt::from(1071)),
        (BigInt::from(17), BigInt::from(17)),
    ] {
        assert_eq!(
            balanced_bigint_gcd(&a, &b),
            a.gcd(&b),
            "balanced_bigint_gcd({a}, {b}) must equal Integer::gcd"
        );
    }
}

proptest! {
    /// Random pairs across a spread of magnitudes, including the
    /// deliberately imbalanced widths (a `u32` against digit strings up
    /// to 512 digits) that are exactly the shape the balancing exists
    /// for. `Integer::gcd` is the oracle; this only has to agree with it
    /// everywhere, not be independently correct.
    #[test]
    fn agrees_with_integer_gcd(
        a in any::<i64>(),
        b_digits in "[0-9]{0,512}",
        b_sign in any::<bool>(),
    ) {
        let a = BigInt::from(a);
        let mut b: BigInt = if b_digits.is_empty() { BigInt::from(0) } else { b_digits.parse().unwrap() };
        if b_sign {
            b = -b;
        }
        prop_assert_eq!(balanced_bigint_gcd(&a, &b), a.gcd(&b));
    }

    /// Both operands within two machine words, the `u128` path: signs,
    /// zero, and the edges of both widths (`u128::MAX` and `i128::MIN`
    /// measure 128 bits).
    #[test]
    fn agrees_with_integer_gcd_within_two_words(
        a in prop_oneof![any::<i128>(), Just(i128::MIN), Just(0i128), (0u32..64).prop_map(|k| 3i128.pow(k))],
        b in prop_oneof![any::<i64>().prop_map(i128::from), Just(i128::MAX), Just(0i128)],
        widen in any::<bool>(),
    ) {
        let a = BigInt::from(a);
        let b = if widen { BigInt::from(u128::MAX) - BigInt::from(b).abs() } else { BigInt::from(b) };
        prop_assert_eq!(balanced_bigint_gcd(&a, &b), a.gcd(&b));
        prop_assert_eq!(balanced_bigint_gcd(&b, &a), a.gcd(&b));
    }

    /// Henrici's `add`/`sub` against the schoolbook oracle
    /// `Fraction::new(a·d ± c·b, b·d)` on wide operands, including a
    /// denominator shared by construction (`shared` multiplies both) so
    /// the `gcd(b, d) != 1` branch and its second reduction both run.
    /// The results must be equal *and* identically represented: both
    /// sides are in lowest terms with a positive denominator.
    #[test]
    fn henrici_add_sub_match_schoolbook(
        an in "-?[1-9][0-9]{0,40}",
        ad in "[1-9][0-9]{18,40}",
        bn in "-?[1-9][0-9]{0,40}",
        bd in "[1-9][0-9]{0,40}",
        shared in 1u64..1_000_000,
    ) {
        let parse = |s: &str| s.parse::<BigInt>().unwrap();
        let shared = BigInt::from(shared);
        let x = Fraction::new(parse(&an), parse(&ad) * &shared);
        let y = Fraction::new(parse(&bn), parse(&bd) * &shared);
        let (xn, xd) = x.to_bigint_pair();
        let (yn, yd) = y.to_bigint_pair();
        let sum = Fraction::new(&xn * &yd + &yn * &xd, &xd * &yd);
        let diff = Fraction::new(&xn * &yd - &yn * &xd, &xd * &yd);
        prop_assert_eq!(x.add(&y).to_bigint_pair(), sum.to_bigint_pair());
        prop_assert_eq!(x.sub(&y).to_bigint_pair(), diff.to_bigint_pair());
    }
}
