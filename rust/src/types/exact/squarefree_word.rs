//! The word routes of the factoring steps: Miller–Rabin and Pollard–Brent in
//! Montgomery form for a cofactor that fits 64 or 128 bits, step for step and
//! charge for charge with the `BigInt` routes in the parent module.

use super::{FactorBudgetExhausted, Meter, MR_BASES};
use num_bigint::BigInt;
use num_traits::{CheckedAdd, One, PrimInt, ToPrimitive, Zero};

/// Arithmetic modulo an odd word `n` in Montgomery form (`x` held as
/// `x·R mod n`, `R` the word's modulus), where a product is reduced by
/// multiplications and a shift rather than a division (Montgomery, "Modular
/// multiplication without trial division", 1985).
pub(super) trait Montgomery: Sized {
    type Word: PrimInt + From<u64>;

    fn new(n: Self::Word) -> Self;
    fn n(&self) -> Self::Word;
    fn mul(&self, a: Self::Word, b: Self::Word) -> Self::Word;
    /// `x` brought into the form.
    fn to_form(&self, x: Self::Word) -> Self::Word;

    #[inline]
    fn add(&self, a: Self::Word, b: Self::Word) -> Self::Word {
        let n = self.n();
        match a.checked_add(&b) {
            Some(sum) if sum < n => sum,
            Some(sum) => sum - n,
            // `a + b − n` without the carry: `a − (n − b)`, as `b < n`.
            None => a - (n - b),
        }
    }

    fn pow(&self, base: Self::Word, mut exponent: Self::Word) -> Self::Word {
        let one = Self::Word::one();
        let mut result = self.to_form(one);
        let mut base = base;
        while exponent > Self::Word::zero() {
            if exponent & one == one {
                result = self.mul(result, base);
            }
            base = self.mul(base, base);
            exponent = exponent >> 1;
        }
        result
    }
}

pub(super) struct Montgomery64 {
    n: u64,
    /// `n⁻¹ mod 2⁶⁴`.
    inverse: u64,
    /// `2¹²⁸ mod n`.
    r2: u64,
}

impl Montgomery for Montgomery64 {
    type Word = u64;

    fn new(n: u64) -> Self {
        debug_assert!(n % 2 == 1);
        let r = (1u128 << 64) % u128::from(n);
        let r2 = (r * r % u128::from(n)) as u64;
        // Newton's iteration doubles the correct low bits each step: 3 → 96.
        let mut inverse = n;
        for _ in 0..5 {
            inverse = inverse.wrapping_mul(2u64.wrapping_sub(n.wrapping_mul(inverse)));
        }
        Montgomery64 { n, inverse, r2 }
    }

    #[inline]
    fn n(&self) -> u64 {
        self.n
    }

    /// `a·b·2⁻⁶⁴ mod n`, in `[0, n)`: `t − m·n` for the `m` that clears the
    /// low word of `t = a·b` leaves the high words' difference.
    #[inline]
    fn mul(&self, a: u64, b: u64) -> u64 {
        let t = u128::from(a) * u128::from(b);
        let m = (t as u64).wrapping_mul(self.inverse);
        let mn = ((u128::from(m) * u128::from(self.n)) >> 64) as u64;
        let (difference, borrow) = ((t >> 64) as u64).overflowing_sub(mn);
        if borrow {
            difference.wrapping_add(self.n)
        } else {
            difference
        }
    }

    fn to_form(&self, x: u64) -> u64 {
        self.mul(x % self.n, self.r2)
    }
}

pub(super) struct Montgomery128 {
    n: u128,
    /// `n⁻¹ mod 2¹²⁸`.
    inverse: u128,
    /// `2²⁵⁶ mod n`.
    r2: u128,
}

/// The 256-bit product `a·b` as `(high, low)`, from four 64-bit products.
#[inline]
fn widening_mul(a: u128, b: u128) -> (u128, u128) {
    const LOW: u128 = u64::MAX as u128;
    let (a1, a0) = (a >> 64, a & LOW);
    let (b1, b0) = (b >> 64, b & LOW);
    let (p00, p01, p10, p11) = (a0 * b0, a0 * b1, a1 * b0, a1 * b1);
    // At most three words below 2⁶⁴ each: no carry out of 128 bits.
    let middle = (p00 >> 64) + (p01 & LOW) + (p10 & LOW);
    let low = (p00 & LOW) | (middle << 64);
    let high = p11 + (p01 >> 64) + (p10 >> 64) + (middle >> 64);
    (high, low)
}

impl Montgomery for Montgomery128 {
    type Word = u128;

    fn new(n: u128) -> Self {
        debug_assert!(n % 2 == 1);
        // Newton's iteration doubles the correct low bits each step: 3 → 192.
        let mut inverse = n;
        for _ in 0..6 {
            inverse = inverse.wrapping_mul(2u128.wrapping_sub(n.wrapping_mul(inverse)));
        }
        let mut form = Montgomery128 { n, inverse, r2: 0 };
        // `2¹²⁸ mod n`, then doubled 128 times: `2²⁵⁶ mod n`.
        let mut r2 = (u128::MAX % n + 1) % n;
        for _ in 0..128 {
            r2 = form.add(r2, r2);
        }
        form.r2 = r2;
        form
    }

    #[inline]
    fn n(&self) -> u128 {
        self.n
    }

    /// `a·b·2⁻¹²⁸ mod n`, in `[0, n)`, as `Montgomery64::mul` one size up.
    #[inline]
    fn mul(&self, a: u128, b: u128) -> u128 {
        let (high, low) = widening_mul(a, b);
        let m = low.wrapping_mul(self.inverse);
        let (mn_high, _) = widening_mul(m, self.n);
        let (difference, borrow) = high.overflowing_sub(mn_high);
        if borrow {
            difference.wrapping_add(self.n)
        } else {
            difference
        }
    }

    fn to_form(&self, x: u128) -> u128 {
        self.mul(x % self.n, self.r2)
    }
}

/// `gcd(a, b)` of two words: `fraction`'s Stein gcd (the 128-bit form
/// hands a pair that fits 64 bits to the 64-bit one), which this module
/// used to carry a generic copy of. A word converts to `u128` and the gcd
/// of two words back to a word without loss.
fn gcd<W: PrimInt>(a: W, b: W) -> W {
    let wide = crate::types::fraction::binary_gcd_u128(
        a.to_u128().expect("a word fits u128"),
        b.to_u128().expect("a word fits u128"),
    );
    <W as num_traits::NumCast>::from(wide).expect("the gcd of two words fits a word")
}

fn abs_diff<W: PrimInt>(a: W, b: W) -> W {
    if a > b {
        a - b
    } else {
        b - a
    }
}

/// `is_probable_prime` on a word `n` in Montgomery form instead of on
/// `BigInt`s: the same bases, the same steps, the same charges against `wide`
/// (which is `n`), so the answer and the work metered are the wide route's.
pub(super) fn miller_rabin<F: Montgomery>(
    n: F::Word,
    wide: &BigInt,
    meter: &mut Meter,
) -> Result<bool, FactorBudgetExhausted> {
    let one = F::Word::one();
    let n_minus_1 = n - one;
    let r = n_minus_1.trailing_zeros();
    let d = n_minus_1 >> r as usize;
    let form = F::new(n);
    let (one_form, minus_one) = (form.to_form(one), form.to_form(n_minus_1));
    'bases: for &a in &MR_BASES {
        let a = F::Word::from(a);
        if (a % n).is_zero() {
            continue;
        }
        for _ in 0..wide.bits() {
            meter.charge(wide)?;
        }
        let mut x = form.pow(form.to_form(a), d);
        if x == one_form || x == minus_one {
            continue;
        }
        for _ in 1..r {
            meter.charge(wide)?;
            x = form.mul(x, x);
            if x == minus_one {
                continue 'bases;
            }
        }
        return Ok(false);
    }
    Ok(true)
}

/// `pollard_brent` on a word `n`, step for step and charge for charge, in
/// Montgomery form instead of on `BigInt`s.
pub(super) fn pollard_brent<F: Montgomery>(
    n: F::Word,
    wide: &BigInt,
    meter: &mut Meter,
) -> Result<F::Word, FactorBudgetExhausted> {
    let one = F::Word::one();
    // Every value is held in Montgomery form, a unit multiple of itself, so
    // each gcd with `n` — and with it every branch — is the wide route's.
    let form = F::new(n);
    for c in 1u64.. {
        let c_form = form.to_form(F::Word::from(c));
        let f = |x: F::Word| form.add(form.mul(x, x), c_form);
        let mut y = form.to_form(F::Word::from(2));
        let mut r: u64 = 1;
        let mut q = form.to_form(one);
        let mut g = one;
        let mut x = y;
        let mut ys = y;
        const M: u64 = 128;
        while g == one {
            x = y;
            for _ in 0..r {
                meter.charge(wide)?;
                y = f(y);
            }
            let mut k = 0;
            while k < r && g == one {
                ys = y;
                for _ in 0..M.min(r - k) {
                    meter.charge(wide)?;
                    y = f(y);
                    q = form.mul(q, abs_diff(x, y));
                }
                g = gcd(q, n);
                k += M;
            }
            r *= 2;
        }
        if g == n {
            loop {
                meter.charge(wide)?;
                ys = f(ys);
                g = gcd(abs_diff(x, ys), n);
                if g != one {
                    break;
                }
            }
        }
        if g != n && g != one {
            return Ok(g);
        }
        // This constant cycled without splitting; try the next one.
        if c > 64 {
            return Err(FactorBudgetExhausted);
        }
    }
    unreachable!("the constant loop only exits by returning")
}

/// `is_probable_prime` on the narrowest word `n` fits, or `None` past 128
/// bits.
pub(super) fn is_probable_prime_word(
    n: &BigInt,
    meter: &mut Meter,
) -> Option<Result<bool, FactorBudgetExhausted>> {
    if let Some(word) = n.to_u64() {
        return Some(miller_rabin::<Montgomery64>(word, n, meter));
    }
    n.to_u128()
        .map(|word| miller_rabin::<Montgomery128>(word, n, meter))
}

/// `pollard_brent` on the narrowest word `n` fits, or `None` past 128 bits.
pub(super) fn pollard_brent_word(
    n: &BigInt,
    meter: &mut Meter,
) -> Option<Result<BigInt, FactorBudgetExhausted>> {
    if let Some(word) = n.to_u64() {
        return Some(pollard_brent::<Montgomery64>(word, n, meter).map(BigInt::from));
    }
    n.to_u128()
        .map(|word| pollard_brent::<Montgomery128>(word, n, meter).map(BigInt::from))
}
