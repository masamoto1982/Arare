//! The exact-real scalar value behind `ValueData::ExactScalar` (LANG.VALUES.EXACT).
//!
//! An enum over the two numeric cost tiers: Tier 0 rationals (`Fraction`,
//! the two points over zero included) and Tier 1 algebraic numbers. The
//! variant is a cost class, never an observable property
//! (LANG.AUTHORITY.FREEDOM): values demote to the cheapest tier that holds
//! them exactly, so an `Algebraic` payload is always irrational. Sign, floor
//! and order are decidable over both, so every comparison here decides —
//! except one asked of `0/0`, which has no order (`Fraction::order`).
//!
//! Its rational view (`best_rational_approximation`) is derived by
//! continued-fraction convergents, computed here and never stored — the
//! value is the normal form.

use crate::types::exact::algebraic::{Algebraic, AlgebraicResult};
use crate::types::fraction::Fraction;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, Zero};
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq)]
pub enum ExactReal {
    /// Tier 0: an exact rational, or one of the two points over zero
    /// (`fraction_extended`).
    Rational(Fraction),
    /// Tier 1: an algebraic irrational in multiquadratic normal form.
    /// Invariant: never rational (rational results demote eagerly). Boxed:
    /// a basis and a term map are five words, and every `Value` would carry
    /// that width for the rare value that is irrational.
    Algebraic(Box<Algebraic>),
}

/// Hashes the same way the derived `PartialEq` compares: by variant, then by
/// the payload's own `Hash` (each tier's own impl is what stays consistent
/// with that tier's equality — `Fraction`'s reduced pair, `Algebraic`'s
/// representation-independent bucket key).
impl std::hash::Hash for ExactReal {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Rational(f) => {
                state.write_u8(0);
                f.hash(state);
            }
            Self::Algebraic(a) => {
                state.write_u8(1);
                a.hash(state);
            }
        }
    }
}

impl ExactReal {
    #[inline]
    pub fn from_fraction(f: Fraction) -> Self {
        Self::Rational(f)
    }

    #[inline]
    pub fn from_integer(n: i64) -> Self {
        Self::Rational(Fraction::new(BigInt::from(n), BigInt::one()))
    }

    #[inline]
    pub fn from_bigint(n: BigInt) -> Self {
        Self::Rational(Fraction::new(n, BigInt::one()))
    }

    fn from_result(result: AlgebraicResult) -> Self {
        match result {
            AlgebraicResult::Rational(f) => Self::Rational(f),
            AlgebraicResult::Irrational(a) => Self::Algebraic(Box::new(a)),
        }
    }

    /// √`radicand` as an exact real. Returns `None` for nil or negative
    /// input; perfect squares and zero demote to `Rational` (same
    /// normalization as the retired CF constructor).
    pub fn from_sqrt_rational(radicand: Fraction) -> Option<Self> {
        Algebraic::sqrt_of_fraction(&radicand).map(Self::from_result)
    }

    /// [`Self::from_sqrt_rational`] with the square-free factorization of the
    /// radicand charged to `budget`; `Err` when the budget cannot factor it.
    pub fn try_sqrt_rational(
        radicand: Fraction,
        budget: &mut u64,
    ) -> Result<Option<Self>, super::squarefree::FactorBudgetExhausted> {
        Ok(Algebraic::sqrt_of_fraction_within(&radicand, budget)?.map(Self::from_result))
    }

    #[inline]
    pub fn as_rational(&self) -> Option<&Fraction> {
        match self {
            Self::Rational(f) => Some(f),
            Self::Algebraic(_) => None,
        }
    }

    #[inline]
    pub fn to_fraction(&self) -> Option<Fraction> {
        self.as_rational().cloned()
    }

    /// Whether the value is a rational or an algebraic irrational, rather
    /// than one of the two points over zero.
    #[inline]
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Rational(f) => f.is_finite(),
            Self::Algebraic(_) => true,
        }
    }

    /// The sign of the value, decidable over the whole domain.
    pub fn signum(&self) -> Ordering {
        match self {
            Self::Rational(f) => f.signum(),
            Self::Algebraic(a) => a.sign(),
        }
    }

    /// The value as the rational an operation over the two points reads:
    /// itself for a rational, and its sign for an irrational, since only the
    /// sign of a finite operand reaches such a result (`fraction_extended`).
    fn extended_view(&self) -> Fraction {
        match self {
            Self::Rational(f) => f.clone(),
            Self::Algebraic(a) => match a.sign() {
                Ordering::Greater => Fraction::from(1),
                Ordering::Less => Fraction::from(-1),
                Ordering::Equal => Fraction::from(0),
            },
        }
    }

    #[inline]
    pub fn is_integer(&self) -> bool {
        match self {
            Self::Rational(f) => f.is_integer(),
            Self::Algebraic(_) => false,
        }
    }

    /// Whether the value is *known* to be exactly zero. Total and never
    /// wrongly `true`: an `Algebraic` is never zero by the normal-form
    /// invariant.
    #[inline]
    pub fn is_structurally_zero(&self) -> bool {
        match self {
            Self::Rational(f) => f.is_zero(),
            Self::Algebraic(_) => false,
        }
    }

    // ---- Arithmetic (the four operations, total) ----

    /// Negation.
    pub fn neg(&self) -> Self {
        match self {
            Self::Rational(f) => Self::Rational(f.neg()),
            Self::Algebraic(a) => Self::Algebraic(Box::new(a.neg())),
        }
    }

    /// Size probe (CS5): the number of algebraic terms this value carries — a
    /// Tier 1 `Algebraic`'s normal-form term count, or `1` for a Tier 0
    /// rational (which cannot explode multiplicatively).
    /// Used to bound the term-pair work of an exact multiply *before* running
    /// it, and to reject a value whose term count crosses `max_algebraic_terms`.
    pub fn algebraic_term_count(&self) -> usize {
        match self {
            Self::Rational(_) => 1,
            Self::Algebraic(a) => a.term_count(),
        }
    }

    /// Size probe (CS5): how many radicals a Tier 1 value's basis holds —
    /// the depth of the conjugation that inverts it — or `0` for a rational.
    /// Used to price a reciprocal before running it.
    pub fn algebraic_basis_len(&self) -> usize {
        match self {
            Self::Rational(_) => 0,
            Self::Algebraic(a) => a.basis().elements().len(),
        }
    }

    /// The multiquadratic normal-form terms `(monomial, coefficient)` of a
    /// Tier 1 value, or `None` for a rational. Used by the lossless state
    /// persistence codec (`crate::types::value_persist`) to capture the
    /// exact algebraic value, and by `DEF` to write such a value back as
    /// source (`crate::interpreter::value_as_code`); both readers reconstruct
    /// it by replaying `∑ cₘ·√m`, which the canonical normal form makes exact.
    pub(crate) fn algebraic_terms(&self) -> Option<Vec<(BigInt, Fraction)>> {
        match self {
            Self::Algebraic(a) => Some(
                a.terms()
                    .iter()
                    .map(|(m, c)| (m.clone(), c.clone()))
                    .collect(),
            ),
            Self::Rational(_) => None,
        }
    }

    /// Size probe (CS5): the largest coefficient bit-length in this value, used
    /// to bound BigInt blow-up against `max_bigint_bits`.
    pub fn max_coefficient_bits(&self) -> u64 {
        match self {
            Self::Rational(f) => f.numerator().bits().max(f.denominator().bits()),
            Self::Algebraic(a) => a.max_coefficient_bits(),
        }
    }

    /// Reciprocal `1/x`, total: the reciprocal of zero is `1/0`
    /// (`Fraction::reciprocal`), and an `Algebraic` is never zero.
    pub fn reciprocal(&self) -> Self {
        match self {
            Self::Rational(f) => Self::Rational(f.reciprocal()),
            Self::Algebraic(a) => Self::from_result(a.reciprocal()),
        }
    }

    /// Addition; demotes to `Rational` whenever the sum is rational
    /// (cheapest-tier-wins). An operand over zero decides the sum by the
    /// pair formulas, to which a finite operand contributes its sign alone.
    pub fn add(&self, other: &Self) -> Self {
        if !self.is_finite() || !other.is_finite() {
            return Self::Rational(self.extended_view().add(&other.extended_view()));
        }
        match (self, other) {
            (Self::Rational(a), Self::Rational(b)) => Self::Rational(a.add(b)),
            (Self::Rational(q), Self::Algebraic(a)) | (Self::Algebraic(a), Self::Rational(q)) => {
                Self::from_result(a.add_fraction(q))
            }
            (Self::Algebraic(a), Self::Algebraic(b)) => Self::from_result(a.add(b)),
        }
    }

    /// Subtraction `self − other`.
    pub fn sub(&self, other: &Self) -> Self {
        if !self.is_finite() || !other.is_finite() {
            return Self::Rational(self.extended_view().sub(&other.extended_view()));
        }
        match (self, other) {
            (Self::Rational(a), Self::Rational(b)) => Self::Rational(a.sub(b)),
            _ => self.add(&other.neg()),
        }
    }

    /// Multiplication.
    pub fn mul(&self, other: &Self) -> Self {
        if !self.is_finite() || !other.is_finite() {
            return Self::Rational(self.extended_view().mul(&other.extended_view()));
        }
        match (self, other) {
            (Self::Rational(a), Self::Rational(b)) => Self::Rational(a.mul(b)),
            (Self::Rational(q), Self::Algebraic(a)) | (Self::Algebraic(a), Self::Rational(q)) => {
                Self::from_result(a.mul_fraction(q))
            }
            (Self::Algebraic(a), Self::Algebraic(b)) => Self::from_result(a.mul(b)),
        }
    }

    /// Division `self / other`, total: multiplication by the reciprocal. A
    /// zero divisor answers `1/0`, or `0/0` for a zero dividend.
    pub fn div(&self, other: &Self) -> Self {
        if !self.is_finite() || !other.is_finite() || other.is_structurally_zero() {
            return Self::Rational(self.extended_view().div(&other.extended_view()));
        }
        match (self, other) {
            (Self::Rational(a), Self::Rational(b)) => Self::Rational(a.div(b)),
            (Self::Rational(q), Self::Algebraic(b)) => Self::from_result(b.recip_scaled(q)),
            (Self::Algebraic(a), Self::Rational(q)) => {
                Self::from_result(a.mul_fraction(&q.reciprocal()))
            }
            (Self::Algebraic(a), Self::Algebraic(b)) => Self::from_result(a.div(b)),
        }
    }

    // ---- Observations ----

    /// Three-way comparison (LANG.VALUES.EXACT). Order in the field is
    /// decidable, so every pair decides — except one holding a point over
    /// zero, which has no order: `None`.
    pub fn cmp_exact(&self, other: &Self) -> Option<Ordering> {
        if !self.is_finite() || !other.is_finite() {
            return self.extended_view().order(&other.extended_view());
        }
        Some(match (self, other) {
            (Self::Rational(a), Self::Rational(b)) => a.cmp_finite(b),
            (Self::Rational(q), Self::Algebraic(b)) => b.cmp_fraction(q).reverse(),
            (Self::Algebraic(a), Self::Rational(q)) => a.cmp_fraction(q),
            (Self::Algebraic(a), Self::Algebraic(b)) => a.cmp(b),
        })
    }

    /// Floor as an exact real. Each of the two points over zero is its
    /// own floor (`Fraction::floor`).
    pub fn floor(&self) -> ExactReal {
        match self {
            Self::Rational(f) => Self::Rational(f.floor()),
            Self::Algebraic(a) => Self::from_bigint(a.floor_int()),
        }
    }

    /// Round to the nearest integer, ties away from zero (matching
    /// `Fraction::round`).
    pub fn round(&self) -> ExactReal {
        match self {
            Self::Rational(f) => Self::Rational(f.round()),
            Self::Algebraic(a) => Self::from_bigint(a.round_int()),
        }
    }

    /// Best rational approximation within a denominator bound: the
    /// deepest principal convergent whose denominator does not exceed
    /// `max_denominator`. `None` for one of the two points over zero,
    /// which no rational approximates, or a bound below 1.
    pub fn best_rational_approximation(&self, max_denominator: &BigInt) -> Option<Fraction> {
        if max_denominator < &BigInt::one() {
            return None;
        }
        match self {
            Self::Rational(f) => {
                if !f.is_finite() {
                    return None;
                }
                if &f.denominator() <= max_denominator {
                    return Some(f.clone());
                }
                convergent_within(
                    &rational_partial_quotients(f.numerator(), f.denominator()),
                    max_denominator,
                )
            }
            Self::Algebraic(a) => a.best_rational_approximation(max_denominator),
        }
    }
}

/// Canonical (regular) CF of a rational, with the standard uniqueness
/// normalization: no trailing `1` term (`[…, a, 1]` folds to `[…, a+1]`).
pub(crate) fn rational_partial_quotients(mut num: BigInt, mut den: BigInt) -> Vec<BigInt> {
    debug_assert!(!den.is_zero());
    if den.is_negative() {
        num = -num;
        den = -den;
    }
    let mut terms: Vec<BigInt> = Vec::new();
    loop {
        let (q, r) = num.div_mod_floor(&den);
        terms.push(q);
        if r.is_zero() {
            break;
        }
        num = den;
        den = r;
    }
    if terms.len() >= 2 && terms.last().expect("non-empty").is_one() {
        let popped = terms.pop().expect("just checked length >= 2");
        *terms.last_mut().expect("length >= 1 after pop") += popped;
    }
    terms
}

/// The deepest principal convergent of `terms` whose denominator stays
/// within `max_denominator`.
fn convergent_within(terms: &[BigInt], max_denominator: &BigInt) -> Option<Fraction> {
    let mut h_prev2 = BigInt::zero();
    let mut h_prev1 = BigInt::one();
    let mut k_prev2 = BigInt::one();
    let mut k_prev1 = BigInt::zero();
    let mut best: Option<(BigInt, BigInt)> = None;
    for a in terms {
        let h = a * &h_prev1 + &h_prev2;
        let k = a * &k_prev1 + &k_prev2;
        if &k > max_denominator {
            break;
        }
        h_prev2 = std::mem::replace(&mut h_prev1, h.clone());
        k_prev2 = std::mem::replace(&mut k_prev1, k.clone());
        best = Some((h, k));
    }
    best.map(|(h, k)| Fraction::new(h, k))
}
