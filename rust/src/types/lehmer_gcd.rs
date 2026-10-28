//! The gcd of two wide integers by Lehmer's method, in place of
//! `num-bigint`'s binary (Stein) gcd.
//!
//! Stein's algorithm shifts and subtracts the whole number for every few bits
//! it removes, so two n-word operands cost about 64·n passes over n words. A
//! running product of rationals (`X 1 X SUB MUL 7/2 MUL` folded, the logistic
//! map) reduces numbers of tens of thousands of bits at every step, and spent
//! 94% of its time there. Lehmer's method runs Euclid's quotients on the top
//! two words alone, as long as they are provably the full numbers' quotients
//! (Knuth, TAOCP vol. 2, 4.5.2, Algorithm L), then applies the steps they
//! stand for to the full numbers in one pass: about one pass per word of
//! progress, not per bit.
//!
//! The answer is the gcd, a function of the operands alone, so which
//! algorithm finds it is unobservable. `lehmer_gcd_tests` holds it equal to
//! `num-bigint`'s.

use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use std::cmp::Ordering;

/// `gcd(a, b)`, non-negative, as `num_integer::Integer::gcd` answers it.
pub(crate) fn gcd(a: &BigInt, b: &BigInt) -> BigInt {
    BigInt::from(gcd_magnitude(a.magnitude(), b.magnitude()))
}

/// `gcd(a, b)` of two magnitudes.
fn gcd_magnitude(a: &BigUint, b: &BigUint) -> BigUint {
    let mut x = a.to_u64_digits();
    let mut y = b.to_u64_digits();
    if compare(&x, &y) == Ordering::Less {
        std::mem::swap(&mut x, &mut y);
    }
    // Invariant: x ≥ y, both without leading zero words.
    loop {
        if y.len() <= 2 {
            return finish(&x, &y);
        }
        if x.len() > y.len() + 1 {
            remainder_step(&mut x, &mut y);
            continue;
        }
        let matrix = top_steps(&x, &y);
        if matrix.1 == 0 {
            // Not even one quotient was certain from the top bits: one
            // division of the full numbers instead.
            remainder_step(&mut x, &mut y);
            continue;
        }
        (x, y) = apply(matrix, &x, &y);
        if compare(&x, &y) == Ordering::Less {
            std::mem::swap(&mut x, &mut y);
        }
    }
}

/// `(x, y) ← (y, x mod y)`.
fn remainder_step(x: &mut Vec<u64>, y: &mut Vec<u64>) {
    let r = BigUint::from_slice(&to_u32_digits(x)) % BigUint::from_slice(&to_u32_digits(y));
    *x = std::mem::replace(y, r.to_u64_digits());
}

/// The gcd once `y` fits in two words: one remainder of `x` by it, then
/// Euclid on machine words.
fn finish(x: &[u64], y: &[u64]) -> BigUint {
    if y.is_empty() {
        return BigUint::from_slice(&to_u32_digits(x));
    }
    let y = to_u128(y);
    let r = BigUint::from_slice(&to_u32_digits(x)) % y;
    let r = to_u128(&r.to_u64_digits());
    BigUint::from(y.gcd(&r))
}

/// The matrix `[[a, b], [c, d]]` of Euclid steps whose quotients the top
/// 126 bits of `x` and `y` determine, so that `(a·x + b·y, c·x + d·y)` is `x`
/// and `y` after those steps. `b == 0` when no step was certain. Every entry's
/// magnitude stays below 2^63 (the steps stop before one would not), so one
/// round takes about 63 bits off each number; `a`, `b` (likewise `c`, `d`)
/// never share a sign, the results being non-negative.
fn top_steps(x: &[u64], y: &[u64]) -> (i128, i128, i128, i128) {
    const CAP: i128 = 1 << 63;
    let n = x.len();
    let shift = x[n - 1].leading_zeros();
    // The top 126 bits at the position of `x`'s leading bit, for both: a
    // truncation of each at one scale, which is all Knuth's test needs. Two
    // bits short of `i128`, so `xh + a` (|a| < 2^63) cannot overflow.
    let top = |v: &[u64]| -> i128 {
        let word = |i: usize| v.get(i).copied().unwrap_or(0);
        let high = (u128::from(word(n - 1)) << 64) | u128::from(word(n - 2));
        let bits = if shift == 0 {
            high
        } else {
            (high << shift) | u128::from(word(n - 3) >> (64 - shift))
        };
        (bits >> 2) as i128
    };
    let (mut xh, mut yh) = (top(x), top(y));
    let (mut a, mut b, mut c, mut d) = (1i128, 0i128, 0i128, 1i128);
    // Knuth's test: the quotient is certain when the two extreme values the
    // full numbers' ratio can take give the same one. Floor division, as the
    // test is stated: a bound can go negative, where `/` rounds toward zero.
    while yh + c != 0 && yh + d != 0 {
        let q = floor_div(xh + a, yh + c);
        if q != floor_div(xh + b, yh + d) {
            break;
        }
        let (Some(next_c), Some(next_d)) = (
            q.checked_mul(c).and_then(|qc| a.checked_sub(qc)),
            q.checked_mul(d).and_then(|qd| b.checked_sub(qd)),
        ) else {
            break;
        };
        if next_c.abs() >= CAP || next_d.abs() >= CAP {
            break;
        }
        (a, c) = (c, next_c);
        (b, d) = (d, next_d);
        (xh, yh) = (yh, xh - q * yh);
    }
    (a, b, c, d)
}

/// `⌊n / d⌋`. Euclid's quotients are mostly 1, 2 or 3 (Gauss–Kuzmin: 1 in
/// 41%, 2 in 17%, 3 in 9%), which subtraction finds without a 128-bit
/// division.
#[inline]
fn floor_div(n: i128, d: i128) -> i128 {
    if n >= 0 && d > 0 {
        let mut rest = n;
        for q in 0..3 {
            if rest < d {
                return q;
            }
            rest -= d;
        }
        return 3 + rest / d;
    }
    Integer::div_floor(&n, &d)
}

/// One row `p·x + q·y` of the matrix, as the product to add and the one to
/// subtract: `p` and `q` never share a sign, and the result is non-negative.
#[derive(Clone, Copy)]
struct Row {
    add: u64,
    add_from_y: bool,
    sub: u64,
}

impl Row {
    fn of(p: i128, q: i128) -> Row {
        if q <= 0 {
            Row {
                add: p as u64,
                add_from_y: false,
                sub: (-q) as u64,
            }
        } else {
            Row {
                add: q as u64,
                add_from_y: true,
                sub: (-p) as u64,
            }
        }
    }
}

/// `(a·x + b·y, c·x + d·y)` in one pass over the digits.
fn apply(m: (i128, i128, i128, i128), x: &[u64], y: &[u64]) -> (Vec<u64>, Vec<u64>) {
    let rows = [Row::of(m.0, m.1), Row::of(m.2, m.3)];
    let n = x.len();
    let mut out = [Vec::with_capacity(n + 1), Vec::with_capacity(n + 1)];
    // Per row: the carries of the two products and the running borrow.
    let mut carry = [(0u64, 0u64, 0u64); 2];
    for (i, &xi) in x.iter().enumerate() {
        let yi = y.get(i).copied().unwrap_or(0);
        for (r, row) in rows.iter().enumerate() {
            let (plus, minus) = if row.add_from_y { (yi, xi) } else { (xi, yi) };
            let (carry_add, carry_sub, borrow) = carry[r];
            let added = u128::from(row.add) * u128::from(plus) + u128::from(carry_add);
            let taken = u128::from(row.sub) * u128::from(minus) + u128::from(carry_sub);
            let (diff, under1) = (added as u64).overflowing_sub(taken as u64);
            let (diff, under2) = diff.overflowing_sub(borrow);
            carry[r] = (
                (added >> 64) as u64,
                (taken >> 64) as u64,
                u64::from(under1) + u64::from(under2),
            );
            out[r].push(diff);
        }
    }
    for (r, digits) in out.iter_mut().enumerate() {
        let (carry_add, carry_sub, borrow) = carry[r];
        digits.push(carry_add.wrapping_sub(carry_sub).wrapping_sub(borrow));
        while digits.last() == Some(&0) {
            digits.pop();
        }
    }
    let [next_x, next_y] = out;
    (next_x, next_y)
}

fn compare(x: &[u64], y: &[u64]) -> Ordering {
    x.len()
        .cmp(&y.len())
        .then_with(|| x.iter().rev().cmp(y.iter().rev()))
}

fn to_u128(v: &[u64]) -> u128 {
    let word = |i: usize| u128::from(v.get(i).copied().unwrap_or(0));
    word(0) | (word(1) << 64)
}

fn to_u32_digits(v: &[u64]) -> Vec<u32> {
    v.iter()
        .flat_map(|&w| [w as u32, (w >> 32) as u32])
        .collect()
}

#[cfg(test)]
#[path = "lehmer_gcd_tests.rs"]
mod lehmer_gcd_tests;
