//! Every `Big` fraction the arithmetic returns is in lowest terms with a
//! positive denominator. The value digest writes a `Big` pair as it is held,
//! without dividing out a gcd, so this is what keeps equal values' digests
//! equal. Operands are wide, carry shared factors (so that the
//! cross-cancellation in `mul`/`div` and the Henrici reduction in `add`/`sub`
//! have something to remove), and include equal denominators and sums that
//! cancel.

use super::fraction::{Fraction, FractionRepr};
use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::{One, Zero};
use proptest::prelude::*;

fn assert_lowest_terms(f: &Fraction, what: &str) {
    if let FractionRepr::Big(big) = &f.repr {
        assert_eq!(
            big.denominator.sign(),
            Sign::Plus,
            "{what}: denominator sign"
        );
        assert!(
            big.numerator.gcd(&big.denominator).is_one(),
            "{what}: {}/{} is not in lowest terms",
            big.numerator,
            big.denominator
        );
    }
}

fn wide() -> impl Strategy<Value = BigInt> {
    (prop::collection::vec(any::<u32>(), 1..10), any::<bool>()).prop_map(|(digits, negative)| {
        BigInt::new(if negative { Sign::Minus } else { Sign::Plus }, digits)
    })
}

/// A factor shared between operands: small primes' powers and a wide one.
fn shared() -> impl Strategy<Value = BigInt> {
    prop_oneof![
        Just(BigInt::one()),
        (0u32..70).prop_map(|k| BigInt::from(2).pow(k)),
        (0u32..40).prop_map(|k| BigInt::from(3).pow(k) * BigInt::from(5).pow(k / 2)),
        wide().prop_map(|w| w.magnitude().clone().into()),
    ]
}

fn fraction() -> impl Strategy<Value = Fraction> {
    (wide(), wide(), shared(), shared()).prop_map(|(n, d, gn, gd)| {
        let d = if d.is_zero() { BigInt::from(7) } else { d };
        let gd = if gd.is_zero() { BigInt::one() } else { gd };
        Fraction::new(n * gn, d * gd)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn arithmetic_keeps_big_pairs_in_lowest_terms(
        a in fraction(),
        b in fraction(),
        same_denominator in any::<bool>(),
        k in shared(),
    ) {
        // `a + 1` keeps `a`'s denominator exactly (gcd(n + d, d) = gcd(n, d)
        // = 1): the equal-denominator branch of add/sub.
        let b = if same_denominator {
            let (an, ad) = a.to_bigint_pair();
            Fraction::new(an + &ad, ad)
        } else {
            b
        };
        let scaled = b.mul(&Fraction::new(k.clone(), BigInt::one()));
        for (what, f) in [
            ("add", a.add(&b)),
            ("sub", a.sub(&b)),
            ("sub self", a.sub(&a)),
            ("mul", a.mul(&b)),
            ("mul scaled", a.mul(&scaled)),
            ("abs", a.abs()),
            ("floor", a.floor()),
            ("round", a.round()),
            ("add scaled", a.add(&scaled)),
        ] {
            assert_lowest_terms(&f, what);
        }
        if !b.is_zero() {
            assert_lowest_terms(&a.div(&b), "div");
        }
        if !scaled.is_zero() {
            assert_lowest_terms(&a.div(&scaled), "div scaled");
        }
    }

    #[test]
    fn a_running_sum_and_product_stay_in_lowest_terms(terms in prop::collection::vec(fraction(), 1..12)) {
        let mut sum = Fraction::new(BigInt::zero(), BigInt::one());
        let mut product = Fraction::new(BigInt::one(), BigInt::one());
        for t in &terms {
            sum = sum.add(t);
            product = product.mul(t);
            assert_lowest_terms(&sum, "running sum");
            assert_lowest_terms(&product, "running product");
        }
    }
}
