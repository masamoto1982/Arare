//! The one-walk sum of a wide fraction and a half-word one against the
//! schoolbook sum reduced by `Fraction::new`, pair for pair and form for
//! form (`Small` or `Big`): denominators that share every factor of the
//! term's, some of them, or none, numerators of both signs, and the terms at
//! the edges of the half-word range and past it (which the walk declines).

use super::super::fraction::{Fraction, FractionRepr};
use num_bigint::{BigInt, Sign};
use proptest::prelude::*;

fn form(f: &Fraction) -> (BigInt, BigInt, bool) {
    let (n, d) = f.to_bigint_pair();
    (n, d, matches!(f.repr, FractionRepr::Small(..)))
}

fn reference(a: &BigInt, b: &BigInt, c: i64, d: i64, subtract: bool) -> Fraction {
    let c = if subtract {
        -BigInt::from(c)
    } else {
        BigInt::from(c)
    };
    Fraction::new(a * d + c * b, b * d)
}

/// A reduced `a/b` whose denominator is past `WIDE_BITS`, built from the
/// digits and the factors given, so it shares chosen factors with a term.
fn wide(digits: Vec<u64>, factors: Vec<u64>, numerator: Vec<u64>, negative: bool) -> Fraction {
    let from_digits = |digits: &[u64]| -> BigInt {
        let halves: Vec<u32> = digits
            .iter()
            .flat_map(|d| [*d as u32, (*d >> 32) as u32])
            .collect();
        BigInt::new(Sign::Plus, halves)
    };
    let mut b = from_digits(&digits) + 1;
    for f in factors {
        b *= f;
    }
    b <<= 130;
    let mut a: BigInt = from_digits(&numerator) + 1;
    if negative {
        a = -a;
    }
    Fraction::new(a, b)
}

/// Checks every sum and difference the walk takes; answers how many it took.
fn check(x: &Fraction, c: i64, d: i64) -> usize {
    let FractionRepr::Big(big) = &x.repr else {
        return 0;
    };
    let mut taken = 0;
    let (a, b) = (&big.numerator, &big.denominator);
    let term = Fraction::new(BigInt::from(c), BigInt::from(d));
    let FractionRepr::Small(c, d) = term.repr else {
        unreachable!("a machine-word pair stays small")
    };
    for subtract in [false, true] {
        if let Some(got) = x.add_word_term(&term, subtract) {
            assert_eq!(form(&got), form(&reference(a, b, c, d, subtract)));
            taken += 1;
        }
    }
    if let Some(got) = term.add_word_term(x, false) {
        assert_eq!(form(&got), form(&reference(a, b, c, d, false)));
        taken += 1;
    }
    assert!(term.add_word_term(x, true).is_none());
    taken
}

#[test]
fn edge_terms_agree() {
    let x = wide(vec![12345, 678], vec![2, 3, 5, 7, 11, 13], vec![99], false);
    let y = wide(
        vec![u64::MAX, u64::MAX, 1],
        vec![65_521, 4_294_967_291],
        vec![7],
        true,
    );
    let mut taken = 0;
    for f in [&x, &y] {
        for (c, d) in [
            (1, 1),
            (0, 1),
            (1, 2),
            (-1, 6),
            (i64::MAX, 1),
            (i64::MIN, 1),
            (i64::MIN + 1, 4_294_967_295),
            (1, 4_294_967_295),
            (1, 4_294_967_296),
            (7, 30_030),
            (-65_521, 1),
        ] {
            taken += check(f, c, d);
        }
    }
    // Every term but the one past half a word, and `i64::MIN`'s difference,
    // which has no negation in a word: 2 · (10 · 3 - 1).
    assert_eq!(taken, 58);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    #[test]
    fn the_walk_is_henrici(
        digits in prop::collection::vec(any::<u64>(), 0..6),
        factors in prop::collection::vec(prop_oneof![2u64..40, Just(4_294_967_291u64), 1u64..70_000], 0..8),
        numerator in prop::collection::vec(any::<u64>(), 0..8),
        negative in any::<bool>(),
        c in prop_oneof![-50i64..50, any::<i64>()],
        d in prop_oneof![1i64..2000, 1i64..=4_294_967_295, Just(4_294_967_296i64), 1i64..i64::MAX],
    ) {
        check(&wide(digits, factors, numerator, negative), c, d);
    }
}
