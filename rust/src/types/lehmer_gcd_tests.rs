//! Lehmer's gcd against `num-bigint`'s on operands chosen to meet every
//! branch: a shared factor wide enough that both sides stay wide for many
//! rounds, lengths far apart (the remainder step), equal and zero operands,
//! consecutive Fibonacci numbers (Euclid's longest run, every quotient 1),
//! and digits of all ones, of zeros and with the top bit alone.

use super::*;
use num_bigint::Sign;
use proptest::prelude::*;

fn digit() -> impl Strategy<Value = u64> {
    prop_oneof![
        4 => any::<u64>(),
        1 => Just(0u64),
        1 => Just(u64::MAX),
        1 => Just(1u64 << 63),
        1 => Just(1u64),
    ]
}

fn number(max_words: usize) -> impl Strategy<Value = BigInt> {
    (prop::collection::vec(digit(), 0..max_words), any::<bool>()).prop_map(|(digits, negative)| {
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

fn agrees(a: &BigInt, b: &BigInt) {
    assert_eq!(gcd(a, b), a.gcd(b), "gcd({a}, {b})");
    assert_eq!(gcd(b, a), a.gcd(b), "gcd({b}, {a})");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn agrees_with_num_bigint(a in number(12), b in number(12)) {
        agrees(&a, &b);
    }

    #[test]
    fn agrees_on_a_shared_factor(g in number(8), a in number(8), b in number(8)) {
        agrees(&(&g * &a), &(&g * &b));
    }

    #[test]
    fn agrees_on_far_apart_lengths(a in number(40), b in number(4)) {
        agrees(&a, &b);
    }
}

#[test]
fn consecutive_fibonacci_numbers_are_coprime() {
    let (mut a, mut b) = (BigInt::from(0), BigInt::from(1));
    for i in 0..3000 {
        (a, b) = (b.clone(), a + b);
        if i % 97 == 0 {
            agrees(&a, &b);
            agrees(&(&a * 6), &(&b * 6));
        }
    }
}

#[test]
fn edge_operands() {
    let wide = BigInt::from(3).pow(400);
    let zero = BigInt::from(0);
    agrees(&zero, &zero);
    agrees(&wide, &zero);
    agrees(&wide, &wide);
    agrees(&wide, &(-&wide));
    agrees(&(BigInt::from(1) << 1000), &(BigInt::from(1) << 700));
    agrees(
        &((BigInt::from(1) << 1000) - 1),
        &((BigInt::from(1) << 600) - 1),
    );
    agrees(&(&wide * 10), &(BigInt::from(10).pow(200)));
    // Top words of all ones on both sides, equal lengths: the widest window
    // the certainty test reads, where its bound once overflowed `i128`.
    let ones = (BigInt::from(1) << 1000) - 1;
    agrees(&ones, &(&ones - (BigInt::from(1) << 500)));
    agrees(&ones, &((BigInt::from(1) << 999) - 1));
    // The minimal input CI's debug build found it with.
    let a: BigInt = "6277101735386680763835789423207666416083908700390324961280"
        .parse()
        .unwrap();
    agrees(&a, &(BigInt::from(1) << 128));
}
