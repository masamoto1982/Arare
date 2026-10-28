//! The machine-word rational kernels, proved by bounded model checking
//! (AQ-VER-001-T, docs/quality/TRACEABILITY_MATRIX.md).
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take, so where the property tests
//! in `fraction_gcd_tests` sample, these decide. Two kinds:
//!
//! - **Whole-range** harnesses quantify over every `i64` operand. They cover
//!   the code with no loop in it: the wasm32 multiply, rounding, the order,
//!   and the integer routes of `add` and `mul`, where the overflow edges —
//!   `i64::MIN`, a sum one past `i64::MAX` — are.
//! - **Small-scope** harnesses take every pair in lowest terms whose halves
//!   fit [`SCOPE_BITS`] bits. They reach the gcd loops, which a solver can
//!   only unroll to a bound, and hold each answer to the specification: the
//!   exact value, in lowest terms, over a positive denominator.
//!
//! The specification is written in `i128`, where no product of two words
//! overflows, so it is a second statement of each law rather than a copy of
//! the kernel.

use super::{add, div, mul, narrow_first_mul, order, round_half_away_from_zero, Pair};
use crate::types::fraction::binary_gcd_u64;
use std::cmp::Ordering;

/// The width of every half of a small-scope operand.
const SCOPE_BITS: u32 = 4;

/// The width of every operand of the gcd harness.
const GCD_SCOPE_BITS: u32 = 6;

/// Euclid's gcd, the reference the kernels' Stein gcd is held to. Bounded by
/// the unwinding each harness declares. Every small-scope value fits 16 bits,
/// and a narrow remainder keeps the formula the solver sees small.
fn euclid(mut a: u16, mut b: u16) -> u16 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `n/d` in lowest terms over a positive denominator, for a small pair.
fn is_normalized((n, d): Pair) -> bool {
    let (n, d) = (
        u16::try_from(n.unsigned_abs()).expect("a small-scope half fits 16 bits"),
        u16::try_from(d.unsigned_abs()).expect("a small-scope half fits 16 bits"),
    );
    d > 0 && euclid(n, d) == 1
}

fn wide((n, d): Pair) -> (i128, i128) {
    (i128::from(n), i128::from(d))
}

/// A small pair's halves, narrowed so the specification's products are cheap.
fn narrow((n, d): Pair) -> (i32, i32) {
    (n as i32, d as i32)
}

/// `a` and `b` are the same rational: `an · bd = bn · ad`.
fn same_small_value((an, ad): (i32, i32), (bn, bd): (i32, i32)) -> bool {
    an * bd == bn * ad
}

/// Any rational in lowest terms with a positive denominator.
fn any_pair() -> Pair {
    let pair: Pair = (kani::any(), kani::any());
    kani::assume(pair.1 > 0);
    pair
}

/// A pair in lowest terms whose halves fit [`SCOPE_BITS`] bits.
fn small_pair() -> Pair {
    let bound = 1i64 << SCOPE_BITS;
    let (n, d): Pair = (kani::any(), kani::any());
    kani::assume(-bound < n && n < bound && 0 < d && d < bound);
    kani::assume(is_normalized((n, d)));
    (n, d)
}

// ---- whole range -----------------------------------------------------------

/// The wasm32 multiply answers what `i64::overflowing_mul` answers.
#[kani::proof]
fn narrow_first_mul_is_overflowing_mul() {
    let (a, b): (i64, i64) = (kani::any(), kani::any());
    assert_eq!(narrow_first_mul(a, b), a.overflowing_mul(b));
}

/// `round_half_away_from_zero` is the nearest integer, a tie taking the one
/// farther from zero, and it always fits.
#[kani::proof]
fn rounding_is_nearest_with_ties_away_from_zero() {
    let (n, d) = any_pair();
    let r = round_half_away_from_zero(n, d);
    let (n, d, r) = (i128::from(n), i128::from(d), i128::from(r));
    let error = (n - r * d).abs();
    assert!(2 * error <= d);
    if 2 * error == d {
        assert!((r * d).abs() > n.abs());
    }
}

/// `order` is the order of the exact values, whatever the cross products'
/// width.
#[kani::proof]
fn order_is_the_order_of_the_values() {
    let (a, b) = (any_pair(), any_pair());
    let ((an, ad), (bn, bd)) = (wide(a), wide(b));
    assert_eq!(order(a, b), (an * bd).cmp(&(bn * ad)));
}

/// An integer sum or difference is exact wherever it fits a word, and
/// declined exactly where it does not.
#[kani::proof]
fn integer_add_is_exact_or_declined() {
    let (a, b): (i64, i64) = (kani::any(), kani::any());
    let subtract: bool = kani::any();
    let exact = if subtract {
        i128::from(a) - i128::from(b)
    } else {
        i128::from(a) + i128::from(b)
    };
    let expected = i64::try_from(exact).ok().map(|n| (n, 1));
    assert_eq!(add((a, 1), (b, 1), subtract), expected);
}

/// An integer product is exact wherever it fits a word, and declined exactly
/// where it does not.
#[kani::proof]
fn integer_mul_is_exact_or_declined() {
    let (a, b): (i64, i64) = (kani::any(), kani::any());
    let expected = a.checked_mul(b).map(|n| (n, 1));
    assert_eq!(mul((a, 1), (b, 1)), expected);
}

/// A power-of-two operand takes the gcd's shortcut, which has no loop: the
/// answer is the twos both operands share, or the other operand when one is
/// zero. The unwinding bound of 1 makes Kani prove the loop past the shortcut
/// is never entered.
#[kani::proof]
#[kani::unwind(1)]
fn gcd_with_a_power_of_two_is_the_shared_twos() {
    let (a, k): (u64, u32) = (kani::any(), kani::any());
    kani::assume(k < 64);
    let b = 1u64 << k;
    let expected = if a == 0 {
        b
    } else {
        1 << a.trailing_zeros().min(k)
    };
    assert_eq!(binary_gcd_u64(a, b), expected);
    assert_eq!(binary_gcd_u64(b, a), expected);
}

// ---- small scope -----------------------------------------------------------

/// Stein's gcd, with its remainder shortcut, is Euclid's.
#[kani::proof]
#[kani::unwind(14)]
fn binary_gcd_is_euclid() {
    let bound = 1u64 << GCD_SCOPE_BITS;
    let (a, b): (u64, u64) = (kani::any(), kani::any());
    kani::assume(a < bound && b < bound);
    assert_eq!(binary_gcd_u64(a, b), u64::from(euclid(a as u16, b as u16)));
}

/// The remainder shortcut, which [`binary_gcd_is_euclid`]'s operands are too
/// narrow to take: for a larger operand past `b · 2¹⁶`, built as `q · b + r`
/// so the specification needs no division, the gcd is `gcd(b, r)`.
#[kani::proof]
#[kani::unwind(14)]
fn binary_gcd_takes_the_remainder_of_a_lopsided_pair() {
    let bound = 1u64 << GCD_SCOPE_BITS;
    let (q, b, r): (u64, u64, u64) = (kani::any(), kani::any(), kani::any());
    kani::assume(b < bound && r < b && q < 1 << 17);
    let a = q * b + r;
    kani::assume(a >> 16 > b);
    let expected = u64::from(euclid(b as u16, r as u16));
    assert_eq!(binary_gcd_u64(a, b), expected);
    assert_eq!(binary_gcd_u64(b, a), expected);
}

/// `a ± b` answers, and answers the exact value in lowest terms.
#[kani::proof]
#[kani::unwind(14)]
fn add_is_the_normalized_sum() {
    let (a, b) = (small_pair(), small_pair());
    let subtract: bool = kani::any();
    let sum = add(a, b, subtract).expect("a small sum fits a pair");
    let ((an, ad), (bn, bd)) = (narrow(a), narrow(b));
    let bn = if subtract { -bn } else { bn };
    assert!(is_normalized(sum));
    assert!(same_small_value(narrow(sum), (an * bd + bn * ad, ad * bd)));
}

/// `a × b` answers, and answers the exact value in lowest terms.
#[kani::proof]
#[kani::unwind(14)]
fn mul_is_the_normalized_product() {
    let (a, b) = (small_pair(), small_pair());
    let product = mul(a, b).expect("a small product fits a pair");
    let ((an, ad), (bn, bd)) = (narrow(a), narrow(b));
    assert!(is_normalized(product));
    assert!(same_small_value(narrow(product), (an * bn, ad * bd)));
}

/// `a ÷ b` declines a zero divisor and otherwise answers the exact value in
/// lowest terms.
#[kani::proof]
#[kani::unwind(14)]
fn div_is_the_normalized_quotient() {
    let (a, b) = (small_pair(), small_pair());
    let quotient = div(a, b);
    if b.0 == 0 {
        assert!(quotient.is_none());
        return;
    }
    let quotient = quotient.expect("a small quotient fits a pair");
    let ((an, ad), (bn, bd)) = (narrow(a), narrow(b));
    assert!(is_normalized(quotient));
    assert!(same_small_value(narrow(quotient), (an * bd, ad * bn)));
}

/// The order of small pairs is a total order consistent with equality of
/// the normalized forms: equal exactly when the pairs are the same.
#[kani::proof]
#[kani::unwind(14)]
fn order_equal_means_same_pair() {
    let (a, b) = (small_pair(), small_pair());
    assert_eq!(order(a, b) == Ordering::Equal, a == b);
    assert_eq!(order(a, b), order(b, a).reverse());
}
