//! `a/b ± c/d` for a reduced fraction with a wide denominator `b` and one
//! whose denominator `d` fits half a machine word: a running sum of small
//! terms (`1 1 n RANGE DIV 0 [ ADD ] FOLD`), where the accumulator is the
//! wide side and every term a narrow one.
//!
//! Henrici's algorithm (`Fraction::add_reduced_bigint`) asks three questions
//! of wide numbers, each a pass of word divisions over every digit:
//! `b mod d` for `g = gcd(b, d)`, the quotient `b / g`, and `t mod g` for
//! `g2 = gcd(t, g)`, where `t = a·(d/g) ± c·(b/g)` is the new numerator. A
//! word division waits on the remainder before it, so each pass runs at the
//! latency of one division per digit however wide the machine is.
//!
//! One pass answers all three. Because `g` divides `d`, `g²` divides `d²`,
//! and `d² < 2⁶⁴`, so dividing `b` once by `d²`, as `q·d² + r`, gives
//! `b mod d = r mod d`, the quotient `b / g = q·(d²/g) + r/g` (a product and
//! a sum, which do not wait on one another digit to digit), and
//! `(b/g) mod g = (r mod g²) / g`. With `a mod d`, taken in the same walk as
//! a second chain the processor overlaps with the first, `t mod g` is then
//! arithmetic on words. The answer is the reduced pair Henrici's algorithm
//! reaches, so it is the same `Fraction`; `fraction_word_term_tests` holds
//! the two equal.

use super::fraction::{Fraction, FractionRepr};
use super::small_divisor::{div_two_by_one, div_word, gcd_word, reciprocal};
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::Zero;

/// A denominator this narrow is left to Henrici's route: the walk's setup
/// would cost more than the passes it saves.
const WIDE_BITS: u64 = 128;

/// A one-word divisor prepared for division most significant digit first:
/// normalized (top bit set), with its reciprocal.
#[derive(Clone, Copy)]
struct Divisor {
    normalized: u64,
    reciprocal: u64,
    shift: u32,
}

impl Divisor {
    fn new(d: u64) -> Self {
        debug_assert!(d != 0);
        let shift = d.leading_zeros();
        let normalized = d << shift;
        Self {
            normalized,
            reciprocal: reciprocal(normalized),
            shift,
        }
    }

    /// Digit `j` of the shifted number in `digits` (`padded`'s layout): a
    /// digit's low bits over the next one's high bits, by one double-word
    /// shift, which leaves the digit itself when there is no shift.
    #[inline(always)]
    fn word(&self, digits: &[u64], j: usize) -> u64 {
        let pair = (u128::from(digits[j + 1]) << 64) | u128::from(digits[j]);
        (pair >> (64 - self.shift)) as u64
    }

    #[inline(always)]
    fn step(&self, remainder: u64, word: u64) -> (u64, u64) {
        div_two_by_one(remainder, word, self.normalized, self.reciprocal)
    }
}

/// A magnitude's digits, least significant first, written into `words`
/// (`len + 2` of them, zeroed) after a zero below the lowest digit, with
/// zeros above the top: every digit of the number shifted left by under a
/// word, `len + 1` of them, then reads two neighbours (`Divisor::word`).
fn pad_into(words: &mut [u64], magnitude: &BigUint) {
    for (slot, digit) in words[1..].iter_mut().zip(magnitude.iter_u64_digits()) {
        *slot = digit;
    }
}

/// `x` divided by `dx`, as the quotient's halves (`BigUint::new`'s digits)
/// and the remainder, and `y mod dy`, in one walk: the two chains share the
/// loop and nothing else, so each division overlaps the other's. The
/// shorter magnitude reads as zeros above its top, which leave a remainder
/// of zero where it is and add only zero digits to a quotient.
fn divide_beside(x: &BigUint, dx: u64, y: &BigUint, dy: u64) -> (Vec<u32>, u64, u64) {
    let (dx, dy) = (Divisor::new(dx), Divisor::new(dy));
    let len = x.iter_u64_digits().len().max(y.iter_u64_digits().len());
    let mut padded: smallvec::SmallVec<[u64; 128]> = smallvec::smallvec![0; 2 * (len + 2)];
    let (xs, ys) = padded.split_at_mut(len + 2);
    pad_into(xs, x);
    pad_into(ys, y);
    let (xs, ys) = (&*xs, &*ys);
    let mut halves: Vec<u32> = vec![0; 2 * (len + 1)];
    let (mut rx, mut ry) = (0u64, 0u64);
    for j in (0..=len).rev() {
        let (q, r) = dx.step(rx, dx.word(xs, j));
        halves[2 * j] = q as u32;
        halves[2 * j + 1] = (q >> 32) as u32;
        rx = r;
        ry = dy.step(ry, dy.word(ys, j)).1;
    }
    (halves, rx >> dx.shift, ry >> dy.shift)
}

impl Fraction {
    /// `self ± other` by the walk above when one side has a wide
    /// denominator and the other a half-word one, or `None` for Henrici's
    /// route. A sum commutes, so the wide side may be either; a difference
    /// is taken only with the wide side first.
    pub(crate) fn add_word_term(&self, other: &Fraction, subtract: bool) -> Option<Fraction> {
        match (&self.repr, &other.repr) {
            (FractionRepr::Big(big), FractionRepr::Small(c, d)) => {
                add_wide_and_word(&big.numerator, &big.denominator, *c, *d, subtract)
            }
            (FractionRepr::Small(c, d), FractionRepr::Big(big)) if !subtract => {
                add_wide_and_word(&big.numerator, &big.denominator, *c, *d, false)
            }
            _ => None,
        }
    }
}

/// `a/b ± c/d` for reduced pairs with `b` wide and positive and
/// `0 < d < 2³²`, or `None` when the operands are not that shape.
fn add_wide_and_word(a: &BigInt, b: &BigInt, c: i64, d: i64, subtract: bool) -> Option<Fraction> {
    if !(1..=i64::from(u32::MAX)).contains(&d) || b.sign() != Sign::Plus || b.bits() <= WIDE_BITS {
        return None;
    }
    let s = if subtract { c.checked_neg()? } else { c };
    let d = d as u64;
    let dd = d * d;
    let (q, rb, ra) = divide_beside(b.magnitude(), dd, a.magnitude(), d);
    let g = gcd_word(rb % d, d);
    if g == 1 {
        // Coprime denominators: `(a·d ± c·b) / (b·d)` is in lowest terms.
        return Some(Fraction::create_already_reduced(a * d + b * s, b * d));
    }
    let d_g = d / g;
    // `g` divides both `b` and `d²`, so it divides `rb` too.
    let b_g = BigInt::from_biguint(Sign::Plus, BigUint::new(q)) * (dd / g) + rb / g;
    // `t mod g`, from words: `a mod g` with `a`'s sign, `(b/g) mod g` from
    // `b mod g²`, and the term's numerator; every product fits an `i128`.
    let g_wide = i128::from(g);
    let a_mod = i128::from(ra % g) * if a.sign() == Sign::Minus { -1 } else { 1 };
    let b_g_mod = i128::from((rb % (g * g)) / g);
    let t_mod = (a_mod * i128::from(d_g % g) + i128::from(s) % g_wide * b_g_mod).rem_euclid(g_wide);
    let t = a * d_g + &b_g * s;
    if t.is_zero() {
        return Some(Fraction::from_repr(FractionRepr::Small(0, 1)));
    }
    let g2 = gcd_word(t_mod as u64, g);
    let numerator = if g2 == 1 { t } else { div_word(&t, g2) };
    Some(Fraction::create_already_reduced(numerator, b_g * (d / g2)))
}

#[cfg(test)]
#[path = "fraction_word_term_tests.rs"]
mod fraction_word_term_tests;
