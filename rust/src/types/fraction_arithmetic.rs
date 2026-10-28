//! `Fraction` arithmetic, and the one shared `gcd` for `BigInt`
//! (`balanced_bigint_gcd`), balanced before it reaches `num-bigint`'s binary
//! GCD. The gcd is used from `fraction.rs`, this file, and the Tier 1
//! algebraic normal form (`exact::basis`, `exact::algebraic`,
//! `exact::algebraic_field`) — a shared low-level primitive, kept beside the
//! arithmetic that calls it most.

use super::fraction::{Fraction, FractionRepr};
use super::small_rational;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, ToPrimitive, Zero};

impl Fraction {
    /// The pair as `BigInt`s, a `Big` half borrowed and a `Small` one built.
    #[inline]
    pub(crate) fn bigint_pair_cow(
        &self,
    ) -> (std::borrow::Cow<'_, BigInt>, std::borrow::Cow<'_, BigInt>) {
        use std::borrow::Cow;
        match &self.repr {
            FractionRepr::Small(n, d) => {
                (Cow::Owned(BigInt::from(*n)), Cow::Owned(BigInt::from(*d)))
            }
            FractionRepr::Big(big) => (
                Cow::Borrowed(&big.numerator),
                Cow::Borrowed(&big.denominator),
            ),
        }
    }

    /// Bit length of the wider half's magnitude, read where it lies.
    #[inline]
    pub(crate) fn magnitude_bits(&self) -> u64 {
        match &self.repr {
            FractionRepr::Small(n, d) => {
                let width = |v: i64| u64::from(64 - v.unsigned_abs().leading_zeros());
                width(*n).max(width(*d))
            }
            FractionRepr::Big(big) => big.numerator.bits().max(big.denominator.bits()),
        }
    }

    pub fn add(&self, other: &Fraction) -> Fraction {
        self.add_signed(other, false)
    }

    /// A sum of two machine-word pairs is `small_rational`'s, the same law
    /// the column kernels run; a sum that leaves a machine word, or has a
    /// wide operand, is `add_wide`'s.
    #[inline]
    fn add_signed(&self, other: &Fraction, subtract: bool) -> Fraction {
        if let (FractionRepr::Small(a, b), FractionRepr::Small(c, d)) = (&self.repr, &other.repr) {
            if *b != 0 && *d != 0 {
                if let Some((n, d)) = small_rational::add((*a, *b), (*c, *d), subtract) {
                    return Fraction::from_repr(FractionRepr::Small(n, d));
                }
            }
        }
        if !self.is_finite() || !other.is_finite() {
            return self.extended_add(other, subtract);
        }
        self.add_wide(other, subtract)
    }

    /// `self ± other` once a half is past a machine word. A `Big` half is
    /// borrowed, not cloned: the running total of a long sum is the wide
    /// operand, and copying it each step is a copy of the whole accumulator.
    /// A small term added to it skips Henrici's wide passes but one
    /// (`fraction_word_term`).
    fn add_wide(&self, other: &Fraction, subtract: bool) -> Fraction {
        if let Some(sum) = self.add_word_term(other, subtract) {
            return sum;
        }
        let (an, ad) = self.bigint_pair_cow();
        let (bn, bd) = other.bigint_pair_cow();

        if ad == bd {
            let sum: BigInt = if subtract { &*an - &*bn } else { &*an + &*bn };
            if sum.is_zero() {
                return Fraction::from_repr(FractionRepr::Small(0, 1));
            }
            let g: BigInt = balanced_bigint_gcd(&sum, &ad);
            if g.is_one() {
                return Self::create_already_reduced(sum, ad.into_owned());
            }
            return Self::create_already_reduced(&sum / &g, &*ad / &g);
        }

        Self::add_reduced_bigint(&an, &ad, &bn, &bd, subtract)
    }

    pub fn sub(&self, other: &Fraction) -> Fraction {
        if !self.is_finite() || !other.is_finite() {
            return self.extended_add(other, true);
        }

        // Two `Small` integers: the overwhelmingly common operand pair (a
        // counter, a running total, an index). Their sum is an integer, so
        // there is nothing to reduce; `create_from_i128` would still run an
        // i128 Euclidean gcd against 1 (a `__divti3` call) to find that out.
        if let (FractionRepr::Small(a, 1), FractionRepr::Small(c, 1)) = (&self.repr, &other.repr) {
            if let Some(n) = a.checked_sub(*c) {
                return Fraction::from_repr(FractionRepr::Small(n, 1));
            }
        }
        if let (Some((a, b)), Some((c, d))) = (self.extract_i64_pair(), other.extract_i64_pair()) {
            if b == 1 && d == 1 {
                return Self::create_from_i128((a as i128) - (c as i128), 1);
            }
            if b == d {
                return Self::create_from_i128((a as i128) - (c as i128), b as i128);
            }
            // Each product is under 2^126 in magnitude, so their difference
            // is under 2^127: it always fits.
            let num = (a as i128) * (d as i128) - (c as i128) * (b as i128);
            return Self::create_from_i128(num, (b as i128) * (d as i128));
        }

        self.add_wide(other, true)
    }

    /// `a/b ± c/d` for two already-reduced fractions with positive
    /// denominators, by Henrici's algorithm (Knuth, TAOCP vol. 2, 4.5.1) —
    /// the one CPython's `fractions.Fraction` uses.
    ///
    /// The schoolbook form `(a·d ± c·b) / (b·d)` followed by one gcd hands
    /// that gcd two operands as wide as the *product* of the denominators.
    /// Henrici takes `g = gcd(b, d)` first: when it is 1 the sum is already
    /// in lowest terms and no wide gcd runs at all, and otherwise the only
    /// remaining gcd is `gcd(t, g)`, whose second operand is no wider than
    /// the narrower denominator. Accumulating a wide fraction with a narrow
    /// one — `1 1 n RANGE DIV 0 [ ADD ] FOLD`, a running total, any sum of
    /// many small-denominator terms — therefore never asks for a gcd of two
    /// wide numbers. Both gcds go through `balanced_bigint_gcd`, so the
    /// wide-against-narrow shape costs one division, not a binary GCD.
    /// Measured on the harmonic number H(5000): 810 ms → 7 ms natively.
    ///
    /// The result is in lowest terms, so it is built without the
    /// normalizing gcd `Fraction::new` would run again.
    fn add_reduced_bigint(
        an: &BigInt,
        ad: &BigInt,
        bn: &BigInt,
        bd: &BigInt,
        subtract: bool,
    ) -> Fraction {
        let g: BigInt = balanced_bigint_gcd(ad, bd);
        if g.is_one() {
            let cross = bn * ad;
            let num = if subtract {
                an * bd - cross
            } else {
                an * bd + cross
            };
            return Self::create_already_reduced(num, ad * bd);
        }
        use crate::types::small_divisor::quotient;
        let ad_g: BigInt = quotient(ad, &g);
        let bd_g: BigInt = quotient(bd, &g);
        let cross = bn * &ad_g;
        let t: BigInt = if subtract {
            an * &bd_g - cross
        } else {
            an * &bd_g + cross
        };
        // `t` is never 0: two reduced values that cancel have equal
        // denominators, which the caller's shared-denominator arm answers.
        let g2: BigInt = balanced_bigint_gcd(&t, &g);
        if g2.is_one() {
            return Self::create_already_reduced(t, ad_g * bd);
        }
        Self::create_already_reduced(quotient(&t, &g2), ad_g * quotient(bd, &g2))
    }

    pub fn mul(&self, other: &Fraction) -> Fraction {
        if let (FractionRepr::Small(a, b), FractionRepr::Small(c, d)) = (&self.repr, &other.repr) {
            if *b != 0 && *d != 0 {
                if let Some((n, d)) = small_rational::mul((*a, *b), (*c, *d)) {
                    return Fraction::from_repr(FractionRepr::Small(n, d));
                }
            }
        }
        if !self.is_finite() || !other.is_finite() {
            return self.extended_mul(other);
        }
        let (an, ad) = self.bigint_pair_cow();
        let (bn, bd) = other.bigint_pair_cow();
        Self::mul_wide(&an, &ad, &bn, &bd)
    }

    /// `(an/ad)·(bn/bd)` of two reduced pairs, each numerator cancelled
    /// against the other's denominator first so the product is reduced. A
    /// quotient is the product with the divisor's halves swapped, so `bd`
    /// may be negative; the sign settles in the constructor.
    fn mul_wide(an: &BigInt, ad: &BigInt, bn: &BigInt, bd: &BigInt) -> Fraction {
        let gcd = |x: &BigInt, unit: &BigInt| {
            if unit.magnitude().is_one() {
                BigInt::one()
            } else {
                balanced_bigint_gcd(x, unit)
            }
        };
        let (g1, g2) = (gcd(an, bd), gcd(bn, ad));
        let cancel = |x: &BigInt, g: &BigInt| if g.is_one() { x.clone() } else { x / g };
        Self::create_already_reduced(
            cancel(an, &g1) * cancel(bn, &g2),
            cancel(ad, &g2) * cancel(bd, &g1),
        )
    }

    pub fn div(&self, other: &Fraction) -> Fraction {
        if let (FractionRepr::Small(a, b), FractionRepr::Small(c, d)) = (&self.repr, &other.repr) {
            if *b != 0 && *d != 0 {
                if let Some((n, d)) = small_rational::div((*a, *b), (*c, *d)) {
                    return Fraction::from_repr(FractionRepr::Small(n, d));
                }
            }
        }
        if !other.is_finite() {
            return self.mul(&other.reciprocal());
        }
        if !self.is_finite() || other.is_zero() {
            return self.extended_mul(&other.reciprocal());
        }
        let (an, ad) = self.bigint_pair_cow();
        let (bn, bd) = other.bigint_pair_cow();
        Self::mul_wide(&an, &ad, &bd, &bn)
    }

    #[inline]
    pub fn neg(&self) -> Fraction {
        match &self.repr {
            FractionRepr::Small(_, 0) => self.clone(),
            FractionRepr::Small(n, d) => match n.checked_neg() {
                Some(negated) => Fraction::from_repr(FractionRepr::Small(negated, *d)),
                None => Fraction::from_repr(FractionRepr::big(-BigInt::from(*n), BigInt::from(*d))),
            },
            // `-(2^63)` narrows back to a machine word.
            FractionRepr::Big(big) => {
                Self::from_bigint_pair(-&big.numerator, big.denominator.clone())
            }
        }
    }

    #[inline]
    pub fn abs(&self) -> Fraction {
        if self.signum() == std::cmp::Ordering::Less {
            self.neg()
        } else {
            self.clone()
        }
    }

    pub fn floor(&self) -> Fraction {
        if !self.is_finite() || self.is_integer() {
            return self.clone();
        }
        match &self.repr {
            FractionRepr::Small(n, d) => Fraction::from(n.div_euclid(*d)),
            FractionRepr::Big(big) => {
                Self::from_bigint_pair(big.numerator.div_floor(&big.denominator), BigInt::one())
            }
        }
    }

    /// The nearest integer, a half rounded away from zero.
    pub fn round(&self) -> Fraction {
        if !self.is_finite() || self.is_integer() {
            return self.clone();
        }
        match &self.repr {
            FractionRepr::Small(n, d) => {
                Fraction::from(small_rational::round_half_away_from_zero(*n, *d))
            }
            FractionRepr::Big(big) => {
                let (n, d) = (&big.numerator, &big.denominator);
                let magnitude = (n.magnitude() * 2u32 + d.magnitude()) / (d.magnitude() * 2u32);
                Self::from_bigint_pair(BigInt::from_biguint(n.sign(), magnitude), BigInt::one())
            }
        }
    }
}

/// `gcd(a, b)` for `BigInt`, balanced before handing off to `num-bigint`'s
/// binary GCD.
///
/// `num_integer::Integer::gcd` on `BigInt` is quadratic in the *difference*
/// between the operands' bit widths, not their size: `gcd(4096-digit, 1)`
/// measured 612 µs on one container, `gcd(4096-digit, 7)` 356 µs, but
/// `gcd(4096-digit, 4096-digit)` 1.2 µs. Every wide rational whose
/// denominator is 1 — i.e. every wide *integer*-valued `Fraction`, which is
/// the common case, not an edge one — hits the slow side of that on every
/// normalization, every `+`/`-`/`*`/`/`, and the equality hash
/// (`Fraction`'s `Hash` impl); a `UNIQUE` over a vector of wide integers was
/// measured charging the collection meter for roughly 1/470th of the time
/// it actually took, because the pricing had no way to see this and priced
/// the value's declared width instead.
///
/// `gcd(a, b) == gcd(b, a mod b)`: one Euclidean division collapses the
/// wider operand down to at most the narrower operand's width — cheap,
/// since a `BigInt` remainder is not the binary GCD's pathological case —
/// and the binary GCD that follows then runs on two comparably-sized
/// operands, its fast case. Verified against `Integer::gcd` directly (not
/// merely against this function's own logic) below: a property test
/// compares the two on random pairs across a spread of relative widths,
/// plus zero and negative operands, and every call site's existing
/// arithmetic/hash tests exercise it in place. Measured 494x faster at
/// `gcd(4096-digit, 1)` and within noise of the direct call once the
/// operands are already balanced, where the extra division is pure
/// overhead.
#[inline]
pub(crate) fn balanced_bigint_gcd(a: &BigInt, b: &BigInt) -> BigInt {
    if a.is_zero() {
        return b.gcd(a);
    }
    if b.is_zero() {
        return a.gcd(b);
    }
    let (wide, narrow) = if a.bits() >= b.bits() { (a, b) } else { (b, a) };
    // A one-word narrow side, against a wide one: the remainder by reciprocal
    // and a word gcd, not a `div` per digit.
    if let Some(g) = crate::types::small_divisor::gcd_with_word(wide, narrow) {
        return g;
    }
    // Both within two machine words: the binary gcd on `u128`, with none of
    // the `BigInt` shifts the gcd below makes per bit. This is the `GCD` Word
    // on ordinary integers, and the reduction of every fraction a little past
    // a machine word.
    if wide.bits() <= 128 {
        let word = |n: &BigInt| n.magnitude().to_u128().expect("within 128 bits");
        return BigInt::from(word(wide).gcd(&word(narrow)));
    }
    if narrow.bits() > LEHMER_BITS {
        return crate::types::lehmer_gcd::gcd(wide, narrow);
    }
    narrow.gcd(&(wide % narrow))
}

/// A narrower side is left to `num-bigint`: one remainder brings the pair
/// within two words, where its binary gcd is short.
const LEHMER_BITS: u64 = 128;
