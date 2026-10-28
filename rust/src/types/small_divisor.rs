//! Division of a wide integer by a divisor of one machine word, by a
//! precomputed reciprocal instead of the hardware `div`.
//!
//! `num-bigint` divides by one digit with a 128-by-64-bit `div` per digit of
//! the dividend, which costs tens of cycles each. A running sum of rationals
//! (`1 1 n RANGE DIV 0 [ ADD ] FOLD`) does this at every step: the accumulator's
//! denominator is thousands of bits, and what it is divided by — the next
//! term's denominator, or the gcd of the two — is a few. Here the divisor's
//! reciprocal is worked out once and each digit then costs a multiplication
//! (Möller and Granlund, "Improved division by invariant integers", 2011,
//! algorithm 4). Measured on a 223-digit dividend: 8.4 µs → 1.3 µs.
//!
//! Everything here answers what the `BigInt` operators answer: the remainder
//! and quotient of the magnitudes, the quotient keeping the dividend's sign.
//! `small_divisor_tests` holds the two equal.

use num_bigint::{BigInt, BigUint};
use num_traits::Zero;

/// A dividend this wide or narrower is left to the operators: the setup of a
/// reciprocal would cost more than it saves.
const WIDE_BITS: u64 = 128;

/// The one-word divisor `d` is, when it is nonzero and fits.
#[inline]
pub(crate) fn single_word(d: &BigInt) -> Option<u64> {
    if d.is_zero() || d.bits() > 64 {
        return None;
    }
    d.magnitude().iter_u64_digits().next()
}

/// The `(quotient, remainder)` of `(u1, u0) / d` for a normalized `d` (top bit
/// set), its reciprocal `v` and `u1 < d`.
#[inline(always)]
pub(crate) fn div_two_by_one(u1: u64, u0: u64, d: u64, v: u64) -> (u64, u64) {
    let product = u128::from(v) * u128::from(u1);
    let sum = product.wrapping_add(((u128::from(u1) + 1) << 64) | u128::from(u0));
    let (mut quotient, low) = ((sum >> 64) as u64, sum as u64);
    let mut remainder = u0.wrapping_sub(quotient.wrapping_mul(d));
    if remainder > low {
        quotient = quotient.wrapping_sub(1);
        remainder = remainder.wrapping_add(d);
    }
    if remainder >= d {
        quotient = quotient.wrapping_add(1);
        remainder -= d;
    }
    (quotient, remainder)
}

/// The reciprocal `⌊(2¹²⁸ − 1) / d⌋ − 2⁶⁴` of a normalized `d` (top bit set),
/// which [`div_two_by_one`] takes.
#[inline]
pub(crate) fn reciprocal(normalized: u64) -> u64 {
    ((u128::MAX - (u128::from(normalized) << 64)) / u128::from(normalized)) as u64
}

/// Walk `magnitude` shifted left by the divisor's normalizing shift, most
/// significant digit first, handing each digit to `step` with the running
/// remainder; answers the final remainder, shifted back.
#[inline(always)]
fn walk(magnitude: &BigUint, d: u64, mut step: impl FnMut(u64, u64) -> u64) -> u64 {
    debug_assert!(d != 0);
    let shift = d.leading_zeros();
    let normalized = d << shift;
    let reciprocal = reciprocal(normalized);
    let mut digits = magnitude.iter_u64_digits().rev();
    let mut remainder = 0u64;
    let mut take = |x: u64, remainder: u64| -> u64 {
        let (quotient, next) = div_two_by_one(remainder, x, normalized, reciprocal);
        step(quotient, next)
    };
    if shift == 0 {
        for digit in digits {
            remainder = take(digit, remainder);
        }
        return remainder;
    }
    // Each shifted digit is a digit's low bits over the next one's high
    // bits: the bits the top digit carries above itself first, then one per
    // digit, the last over nothing.
    let Some(top) = digits.next() else {
        return 0;
    };
    remainder = take(top >> (64 - shift), remainder);
    let mut high = top;
    for digit in digits {
        remainder = take((high << shift) | (digit >> (64 - shift)), remainder);
        high = digit;
    }
    remainder = take(high << shift, remainder);
    remainder >> shift
}

/// `|a| mod d`.
pub(crate) fn rem_word(a: &BigInt, d: u64) -> u64 {
    walk(a.magnitude(), d, |_, remainder| remainder)
}

/// `a / d`, truncated toward zero, as the `/` operator answers it.
///
/// The quotient's digits come most significant first, so each is written
/// straight into its place among the halves `BigUint::new` takes: the walk
/// makes one per digit of `a`, and one more for the top bits a normalizing
/// shift carries above them.
pub(crate) fn div_word(a: &BigInt, d: u64) -> BigInt {
    let digits = a.magnitude().iter_u64_digits().len();
    let words = digits + usize::from(d.leading_zeros() > 0 && digits > 0);
    let mut halves: Vec<u32> = vec![0; 2 * words];
    let mut at = words;
    walk(a.magnitude(), d, |q, remainder| {
        at -= 1;
        halves[2 * at] = q as u32;
        halves[2 * at + 1] = (q >> 32) as u32;
        remainder
    });
    BigInt::from_biguint(a.sign(), BigUint::new(halves))
}

/// `gcd(a, b)` of two machine words: `fraction`'s Stein gcd, which this
/// module used to carry its own copy of.
#[inline]
pub(crate) fn gcd_word(a: u64, b: u64) -> u64 {
    crate::types::fraction::binary_gcd_u64(a, b)
}

/// `gcd(wide, d)` for a one-word `d`, when `wide` is wide enough to be worth
/// it: one remainder by reciprocal and a word gcd. `None` leaves the pair to
/// the caller's own route.
pub(crate) fn gcd_with_word(wide: &BigInt, d: &BigInt) -> Option<BigInt> {
    let word = single_word(d)?;
    (wide.bits() > WIDE_BITS).then(|| BigInt::from(gcd_word(word, rem_word(wide, word))))
}

/// `a / d`, by reciprocal when `d` is one word and `a` wide.
#[inline]
pub(crate) fn quotient(a: &BigInt, d: &BigInt) -> BigInt {
    match single_word(d) {
        // Zero has no bits, so a wide `a` is never zero.
        Some(word) if a.bits() > WIDE_BITS => div_word(a, word),
        _ => a / d,
    }
}

#[cfg(test)]
#[path = "small_divisor_tests.rs"]
mod small_divisor_tests;

#[cfg(kani)]
#[path = "small_divisor_proofs.rs"]
mod proofs;
