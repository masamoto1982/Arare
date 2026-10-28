//! Field operations on the Tier 1 normal form — multiplicative inverse and
//! division — and its integer rounding and rational approximation. Split
//! from `algebraic.rs` to respect the file-size budget (the file-size budget
//! in docs/dev/specification-implementation-rules.md).
//!
//! The continued fraction is **derived** here for the rational approximation
//! only — it is not an internal representation, and it is not a display
//! either (an irrational displays as its own source, `display.rs`). Because a
//! Tier 1 value has a decidable floor and exact field arithmetic, the
//! canonical CF terms fall out of the classical floor-and-reciprocate
//! iteration, exactly, to any requested depth.

use crate::types::exact::algebraic::{add_term as merge_term, Algebraic, AlgebraicResult};
use crate::types::fraction::Fraction;
use crate::types::fraction_arithmetic::balanced_bigint_gcd;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Zero};
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// A ceiling on the work one approximation may spend tightening its
/// enclosure, counted as terms × (enclosure bits + coefficient bits): each
/// `bounds(bits)` takes one integer square root and one product per term, at
/// about that width. This approximation is a convenience beside a value in the
/// wire protocol, never a meter-charged operation, so it must not cost
/// seconds: a value whose terms cancel almost exactly (`(√2 − 1)ⁿ`, written
/// out as `a − b√2` with coefficients of hundreds of thousands of bits) needs
/// an enclosure about as fine as its coefficients are wide before even its
/// sign is known. Past the ceiling it answers what it has, or `None` for the
/// caller's termwise fallback — the answer the work-budgeted expansion gave
/// such values before.
const ENCLOSURE_WORK: u64 = 1 << 18;

impl Algebraic {
    /// Multiplicative inverse `1/self` by recursive conjugation.
    ///
    /// Splitting on a basis element b as y = u + v (v = the terms whose
    /// monomial contains b), y·(u − v) = u² − v² has no b in its support,
    /// so each step eliminates one basis element and bottoms out at a
    /// rational. u² − v² = 0 would force y = 0 (a field has no zero
    /// divisors), and an `Algebraic` is never zero by the normal-form
    /// invariant, so the recursion is total — no budget, no failure path.
    pub fn reciprocal(&self) -> AlgebraicResult {
        let value = MqTerms(self.terms().clone());
        let inverse = value.inverse(self);
        AlgebraicResult::from_terms_over(self, inverse.0)
    }

    /// `self / other` for another algebraic (never-zero) value.
    pub fn div(&self, other: &Algebraic) -> AlgebraicResult {
        match other.reciprocal() {
            AlgebraicResult::Rational(f) => self.mul_fraction(&f),
            AlgebraicResult::Irrational(inv) => self.mul(&inv),
        }
    }

    /// `q / self` for a rational numerator.
    pub fn recip_scaled(&self, q: &Fraction) -> AlgebraicResult {
        match self.reciprocal() {
            AlgebraicResult::Rational(f) => AlgebraicResult::Rational(f.mul(q)),
            AlgebraicResult::Irrational(inv) => inv.mul_fraction(q),
        }
    }
}

impl AlgebraicResult {
    /// Package raw terms produced over `source`'s basis (demoting to a
    /// rational when the term shape allows).
    fn from_terms_over(source: &Algebraic, terms: BTreeMap<BigInt, Fraction>) -> AlgebraicResult {
        Algebraic::result_from_parts_of(source, terms)
    }
}

/// A raw coefficient map over an implicit shared basis: the internal
/// working state of the conjugation recursion. Unlike `Algebraic` it may
/// be zero or rational mid-recursion.
struct MqTerms(BTreeMap<BigInt, Fraction>);

impl MqTerms {
    fn zero() -> MqTerms {
        MqTerms(BTreeMap::new())
    }

    fn as_rational(&self) -> Option<Fraction> {
        match self.0.len() {
            0 => Some(Fraction::new(BigInt::zero(), BigInt::one())),
            1 => self
                .0
                .iter()
                .next()
                .filter(|(m, _)| m.is_one())
                .map(|(_, c)| c.clone()),
            _ => None,
        }
    }

    fn mul(&self, other: &MqTerms) -> MqTerms {
        let mut out = MqTerms::zero();
        for (m1, c1) in &self.0 {
            for (m2, c2) in &other.0 {
                let g = balanced_bigint_gcd(m1, m2);
                let monomial = (m1 / &g) * (m2 / &g);
                let coeff = c1.mul(c2).mul(&Fraction::new(g, BigInt::one()));
                merge_term(&mut out.0, monomial, coeff);
            }
        }
        out
    }

    fn sub(&self, other: &MqTerms) -> MqTerms {
        let mut out = MqTerms(self.0.clone());
        for (m, c) in &other.0 {
            merge_term(&mut out.0, m.clone(), c.neg());
        }
        out
    }

    /// Inverse by conjugation over `host`'s basis. Total for a non-zero
    /// value (see `reciprocal`); the recursion depth is bounded by the
    /// basis size.
    fn inverse(&self, host: &Algebraic) -> MqTerms {
        if let Some(q) = self.as_rational() {
            debug_assert!(!q.is_zero(), "inverse of zero is excluded by the invariant");
            let (n, d) = q.to_bigint_pair();
            let mut out = MqTerms::zero();
            merge_term(&mut out.0, BigInt::one(), Fraction::new(d, n));
            return out;
        }
        let split_on = host
            .basis()
            .elements()
            .iter()
            .find(|b| self.0.keys().any(|m| (m % *b).is_zero()))
            .expect("a non-rational term map uses at least one basis element")
            .clone();
        let mut with_b = MqTerms::zero();
        let mut without_b = MqTerms::zero();
        for (m, c) in &self.0 {
            if (m % &split_on).is_zero() {
                merge_term(&mut with_b.0, m.clone(), c.clone());
            } else {
                merge_term(&mut without_b.0, m.clone(), c.clone());
            }
        }
        let conjugate = without_b.sub(&with_b);
        let product = MqTerms(self.0.clone()).mul(&conjugate);
        let product_inverse = product.inverse(host);
        conjugate.mul(&product_inverse)
    }
}

fn fraction_floor(f: &Fraction) -> BigInt {
    f.numerator().div_floor(&f.denominator())
}

impl Algebraic {
    /// ⌊self⌋. An `Algebraic` is irrational (rationals demote), so it is
    /// never an integer and nested enclosures eventually separate it from
    /// every integer: the doubling loop is a decidable computation, not a
    /// budgeted search.
    pub fn floor_int(&self) -> BigInt {
        let mut bits = 8u64;
        loop {
            let (lo, hi) = self.bounds(bits);
            let fl = fraction_floor(&lo);
            let fh = fraction_floor(&hi);
            if fl == fh {
                return fl;
            }
            bits *= 2;
        }
    }

    /// Round to the nearest integer. An irrational is never a half-integer,
    /// so the tie rule (away from zero, matching `Fraction::round`) can
    /// never fire; the order against ⌊self⌋ + 1/2 decides exactly.
    pub fn round_int(&self) -> BigInt {
        let floor = self.floor_int();
        let half_up = Fraction::new(&floor * BigInt::from(2) + BigInt::one(), BigInt::from(2));
        match self.cmp_fraction(&half_up) {
            Ordering::Less => floor,
            _ => floor + BigInt::one(),
        }
    }

    /// Best rational approximation within a denominator bound: the
    /// deepest principal convergent whose denominator does not exceed
    /// `max_denominator`. Same contract as the historical
    /// `ExactReal::best_rational_approximation`; `None` when
    /// `max_denominator LT 1`.
    ///
    /// The convergents come from an enclosure, not from floor-and-reciprocate
    /// on the value: each step of that rewrites the whole sum, whose cost
    /// grows exponentially with the term count (a sum of 19 square roots took
    /// 21 s to show). `bounds(bits)` is linear in the terms, and the
    /// continued-fraction terms that `lo` and `hi` share are the terms of
    /// every number between them — the numbers sharing a prefix form an
    /// interval — so the shared prefix is the value's own expansion as far as
    /// it goes. When it ends before a convergent passes the bound, the
    /// enclosure is tightened and the prefix read again.
    pub fn best_rational_approximation(&self, max_denominator: &BigInt) -> Option<Fraction> {
        if max_denominator < &BigInt::one() {
            return None;
        }
        let mut bits = (2 * max_denominator.bits() + 32).max(64);
        let terms = (self.term_count() as u64).max(1);
        let coefficient_bits = self.max_coefficient_bits();
        let mut best: Option<Fraction> = None;
        while terms.saturating_mul(bits.saturating_add(coefficient_bits)) <= ENCLOSURE_WORK {
            let (lo, hi) = self.bounds(bits);
            let (mut a_num, mut a_den) = lo.to_bigint_pair();
            let (mut b_num, mut b_den) = hi.to_bigint_pair();
            let mut h_prev2 = BigInt::zero();
            let mut h_prev1 = BigInt::one();
            let mut k_prev2 = BigInt::one();
            let mut k_prev1 = BigInt::zero();
            let mut shared_ran_out = true;
            loop {
                let (qa, ra) = a_num.div_mod_floor(&a_den);
                let (qb, rb) = b_num.div_mod_floor(&b_den);
                // A term is the value's only when both ends agree on it and
                // neither ends here: a terminated end is a convergent itself,
                // and the value lies beyond it.
                if qa != qb || ra.is_zero() || rb.is_zero() {
                    // The value's own term lies between the two ends' (the
                    // map from a shared prefix to the next term is monotone
                    // on the interval), one lower when an end terminates
                    // here, since `[…, q]` is also `[…, q − 1, 1]`. When even
                    // the least of those carries the denominator past the
                    // bound, the convergent found so far is the answer and
                    // no finer enclosure can change it.
                    let terminated = ra.is_zero() || rb.is_zero();
                    let least = qa.min(qb) - BigInt::from(u8::from(terminated));
                    if !k_prev1.is_zero() && &(&least * &k_prev1 + &k_prev2) > max_denominator {
                        shared_ran_out = false;
                    }
                    break;
                }
                let h = &qa * &h_prev1 + &h_prev2;
                let k = &qa * &k_prev1 + &k_prev2;
                if &k > max_denominator {
                    shared_ran_out = false;
                    break;
                }
                h_prev2 = std::mem::replace(&mut h_prev1, h.clone());
                k_prev2 = std::mem::replace(&mut k_prev1, k.clone());
                best = Some(Fraction::new(h, k));
                (a_num, a_den) = (a_den, ra);
                (b_num, b_den) = (b_den, rb);
            }
            if !shared_ran_out {
                return best;
            }
            bits *= 2;
        }
        best
    }
}
