//! Arithmetic on rationals whose numerator and denominator each fit a
//! machine word: the `Small` form `Fraction` stores them in, held as an
//! `(i64, i64)` pair in lowest terms with a positive denominator.
//!
//! `Fraction`'s own small path dispatches on its enum, widens to `i128` and
//! re-normalises through `create_from_i128` on every operation. The kernels
//! that hold many such values unboxed (`dense_kernels`, and through them the
//! column walk) use these instead: Henrici's sum and cross-cancelled
//! product and quotient, with gcds and quotients in machine words and only
//! the final products widened. Each answers `None` when its result no longer
//! fits a pair (or, for `div`, on a zero divisor), for the caller to hand the
//! work to `Fraction`. A rational's lowest-terms form with a positive
//! denominator is unique, so these agree with `Fraction` wherever they
//! answer; `fraction_gcd_tests` holds them to it.
//!
//! The `_total` forms take the two points over zero too — `(1, 0)` and
//! `(0, 0)` — and a zero divisor, which the pair laws above do not (they
//! assume a positive denominator). Such an operation is answered by the
//! `Fraction`'s own total arithmetic (`fraction_extended`), whose answer is
//! always one of the two points or zero, a small pair; every other is the
//! pair law's. `order_total` is the one place the pair kernels decide that
//! neither point has an order.

use crate::types::fraction::{binary_gcd_u128, binary_gcd_u64, compute_gcd_i64, Fraction};
use std::cmp::Ordering;

/// A rational in lowest terms with a positive denominator.
pub(crate) type Pair = (i64, i64);

/// `n/d` rounded to the nearest integer, halves away from zero, for a pair
/// in lowest terms with `d > 0`: `⌊(2|n| + d) / 2d⌋` with the sign put back.
///
/// The answer always fits: for `d ≥ 2` its magnitude is at most `|n|/d + ½`,
/// and for `d = 1` it is `|n|` itself, so `i64::MIN` comes back as it went
/// in. The quickened scalar tier and the column kernels each used to spell
/// this formula out.
#[inline]
pub(crate) fn round_half_away_from_zero(n: i64, d: i64) -> i64 {
    let (wide_n, wide_d) = (i128::from(n), i128::from(d));
    let magnitude = (2 * wide_n.abs() + wide_d) / (2 * wide_d);
    (if n < 0 { -magnitude } else { magnitude }) as i64
}

/// `a * b` with an overflow flag, as `i64::overflowing_mul` answers it.
///
/// On wasm32 the flag is computed through a 128-bit software multiply
/// (`__multi3`), which was a tenth of a rational MAP's time there. Two
/// factors that each fit 32 bits cannot overflow, so that case — nearly
/// every product a program makes — is one plain multiply.
#[inline(always)]
pub(crate) fn overflowing_mul(a: i64, b: i64) -> (i64, bool) {
    #[cfg(target_arch = "wasm32")]
    return narrow_first_mul(a, b);
    #[cfg(not(target_arch = "wasm32"))]
    a.overflowing_mul(b)
}

/// [`overflowing_mul`]'s wasm32 route, built on every target so the tests
/// hold it to `i64::overflowing_mul` natively too.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[inline(always)]
pub(crate) fn narrow_first_mul(a: i64, b: i64) -> (i64, bool) {
    if a == i64::from(a as i32) && b == i64::from(b as i32) {
        return (a.wrapping_mul(b), false);
    }
    a.overflowing_mul(b)
}

/// `a * b`, or `None` on overflow, through [`overflowing_mul`].
#[inline(always)]
pub(crate) fn checked_mul(a: i64, b: i64) -> Option<i64> {
    match overflowing_mul(a, b) {
        (v, false) => Some(v),
        (_, true) => None,
    }
}

#[inline]
fn fit(n: i128, d: i128) -> Option<Pair> {
    Some((i64::try_from(n).ok()?, i64::try_from(d).ok()?))
}

/// `a ± b` by Henrici's algorithm, as `Fraction::add` reduces a wide sum:
/// `g = gcd(b, d)` first, and when it is 1 the sum is already reduced.
/// The gcds and quotients stay in machine words where the operands do.
pub(crate) fn add((an, ad): Pair, (bn, bd): Pair, subtract: bool) -> Option<Pair> {
    if let Some(sum) = add_in_words((an, ad), (bn, bd), subtract) {
        return sum;
    }
    let bn = if subtract {
        -i128::from(bn)
    } else {
        i128::from(bn)
    };
    if ad == 1 && bd == 1 {
        return fit(i128::from(an) + bn, 1);
    }
    let g = gcd64(ad, bd);
    if g == 1 {
        let n = i128::from(an) * i128::from(bd) + bn * i128::from(ad);
        if n == 0 {
            return Some((0, 1));
        }
        return fit(n, i128::from(ad) * i128::from(bd));
    }
    let (s, bd_g) = (ad / g, bd / g);
    let t = i128::from(an) * i128::from(bd_g) + bn * i128::from(s);
    if t == 0 {
        return Some((0, 1));
    }
    let (n, g2) = match i64::try_from(t) {
        Ok(t) => {
            let g2 = gcd64(t, g);
            (i128::from(t / g2), g2)
        }
        Err(_) => {
            let g2 = binary_gcd_u128(t.unsigned_abs(), g.unsigned_abs().into()) as i64;
            (t / i128::from(g2), g2)
        }
    };
    fit(n, i128::from(s) * i128::from(bd / g2))
}

/// `add` with every product and sum in machine words: the answer, or
/// `None` when an intermediate overflows and `add` must widen. A value that
/// fits is the same value however it was computed, so this answers exactly
/// what the widened route does wherever it answers at all — and it matters
/// on WebAssembly, where an `i128` product is a call into a software routine.
#[inline]
fn add_in_words((an, ad): Pair, (bn, bd): Pair, subtract: bool) -> Option<Option<Pair>> {
    let bn = if subtract { bn.checked_neg()? } else { bn };
    if ad == 1 && bd == 1 {
        return Some(Some((an.checked_add(bn)?, 1)));
    }
    // An integer beside a fraction: `g` is 1 and the sum `(a·d + c) / d` is
    // already reduced, as `c` shares no factor with `d`; and it is not zero,
    // as `d > 1` does not divide `c`. So no gcd is needed to know either.
    if ad == 1 {
        return Some(Some((checked_mul(an, bd)?.checked_add(bn)?, bd)));
    }
    if bd == 1 {
        return Some(Some((checked_mul(bn, ad)?.checked_add(an)?, ad)));
    }
    let g = gcd64(ad, bd);
    if g == 1 {
        let n = checked_mul(an, bd)?.checked_add(checked_mul(bn, ad)?)?;
        if n == 0 {
            return Some(Some((0, 1)));
        }
        return Some(Some((n, checked_mul(ad, bd)?)));
    }
    let (s, bd_g) = (ad / g, bd / g);
    let t = checked_mul(an, bd_g)?.checked_add(checked_mul(bn, s)?)?;
    if t == 0 {
        return Some(Some((0, 1)));
    }
    let g2 = gcd64(t, g);
    Some(Some((t / g2, checked_mul(s, bd / g2)?)))
}

/// `gcd(|a|, b)` for `b > 0`, in machine words: `b` is a positive
/// denominator, so the gcd divides it and fits.
#[inline]
fn gcd64(a: i64, b: i64) -> i64 {
    compute_gcd_i64(a, b)
}

/// `a × b` with the cross gcds divided out first, so the product is already
/// in lowest terms. The gcds and quotients stay in machine words — an `i128`
/// division is a software routine — and only the products widen.
pub(crate) fn mul((an, ad): Pair, (bn, bd): Pair) -> Option<Pair> {
    // An integer half has nothing to cancel against: gcd(x, 1) is 1.
    let g1 = if bd == 1 { 1 } else { gcd64(an, bd) };
    let g2 = if ad == 1 { 1 } else { gcd64(bn, ad) };
    // A gcd of 1, the usual case, divides nothing: skip the division, which
    // costs more than the rest of the product.
    let (an, bd) = if g1 == 1 {
        (an, bd)
    } else {
        (an / g1, bd / g1)
    };
    let (bn, ad) = if g2 == 1 {
        (bn, ad)
    } else {
        (bn / g2, ad / g2)
    };
    // Each product fits a word exactly when it fits the answer, so the
    // checked products are the whole test.
    Some((checked_mul(an, bn)?, checked_mul(ad, bd)?))
}

/// `a ÷ b`: the cross gcds divided out as in `mul`, with `b`'s halves
/// exchanged and the sign moved to the numerator. `None` for a zero divisor.
pub(crate) fn div((an, ad): Pair, (bn, bd): Pair) -> Option<Pair> {
    if bn == 0 {
        return None;
    }
    // Two integers where the divisor divides: the quotient is an integer
    // (`checked_*` declines i64::MIN / -1, which the general route widens).
    if ad == 1 && bd == 1 && an.checked_rem(bn) == Some(0) {
        if let Some(q) = an.checked_div(bn) {
            return Some((q, 1));
        }
    }
    let g2 = if ad == 1 || bd == 1 { 1 } else { gcd64(bd, ad) };
    // gcd(|a|, |b|) is 2^63 only for halves drawn from {0, i64::MIN}; that
    // one case divides in `i128`.
    let g1 = binary_gcd_u64(an.unsigned_abs(), bn.unsigned_abs());
    // In machine words when every product and the sign change fit; else
    // widened below, which also covers a product of 2^63 that the sign
    // change brings back into range.
    if let Ok(g1) = i64::try_from(g1) {
        // As in `mul`, a gcd of 1 divides nothing.
        let (an1, bn1) = if g1 == 1 {
            (an, bn)
        } else {
            (an / g1, bn / g1)
        };
        let (ad2, bd2) = if g2 == 1 {
            (ad, bd)
        } else {
            (ad / g2, bd / g2)
        };
        let n = checked_mul(an1, bd2);
        let d = checked_mul(ad2, bn1);
        if let (Some(n), Some(d)) = (n, d) {
            if d > 0 {
                return Some((n, d));
            }
            if let (Some(n), Some(d)) = (n.checked_neg(), d.checked_neg()) {
                return Some((n, d));
            }
        }
    }
    let (an_g, bn_g) = match i64::try_from(g1) {
        Ok(g1) => (i128::from(an / g1), i128::from(bn / g1)),
        Err(_) => (
            i128::from(an) / i128::from(g1),
            i128::from(bn) / i128::from(g1),
        ),
    };
    let n = an_g * i128::from(bd / g2);
    let d = i128::from(ad / g2) * bn_g;
    if d < 0 {
        fit(-n, -d)
    } else {
        fit(n, d)
    }
}

#[inline]
pub(crate) fn order((an, ad): Pair, (bn, bd): Pair) -> Ordering {
    if let (Some(x), Some(y)) = (checked_mul(an, bd), checked_mul(bn, ad)) {
        return x.cmp(&y);
    }
    (i128::from(an) * i128::from(bd)).cmp(&(i128::from(bn) * i128::from(ad)))
}

/// A lane over zero, or a zero divisor: the operation the pair laws above
/// leave to the `Fraction`'s own total arithmetic.
#[cold]
#[inline(never)]
fn over_zero(a: Pair, b: Pair, law: fn(&Fraction, &Fraction) -> Fraction) -> Option<Pair> {
    law(
        &Fraction::from_normalized_pair(a.0, a.1),
        &Fraction::from_normalized_pair(b.0, b.1),
    )
    .extract_i64_pair()
}

/// [`add`] on every pair, a point over zero included.
#[inline]
pub(crate) fn add_total(a: Pair, b: Pair, subtract: bool) -> Option<Pair> {
    if a.1 == 0 || b.1 == 0 {
        return over_zero(
            a,
            b,
            if subtract {
                Fraction::sub
            } else {
                Fraction::add
            },
        );
    }
    add(a, b, subtract)
}

/// [`mul`] on every pair, a point over zero included.
#[inline]
pub(crate) fn mul_total(a: Pair, b: Pair) -> Option<Pair> {
    if a.1 == 0 || b.1 == 0 {
        return over_zero(a, b, Fraction::mul);
    }
    mul(a, b)
}

/// [`div`] on every pair, a point over zero and a zero divisor included.
#[inline]
pub(crate) fn div_total(a: Pair, b: Pair) -> Option<Pair> {
    if a.1 == 0 || b.1 == 0 || b.0 == 0 {
        return over_zero(a, b, Fraction::div);
    }
    div(a, b)
}

/// The order of two pairs, as `Fraction::order` decides it: `None` when
/// either lies over zero, since neither point is ordered against anything
/// (LANG.VALUES.EXACT).
#[inline]
pub(crate) fn order_total(a: Pair, b: Pair) -> Option<Ordering> {
    (a.1 != 0 && b.1 != 0).then(|| order(a, b))
}

#[cfg(kani)]
#[path = "small_rational_proofs.rs"]
mod proofs;
