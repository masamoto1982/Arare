//! Tier 1: algebraic numbers as a first-class multiquadratic normal form
//! (LANG.VALUES.EXACT).
//!
//! An [`Algebraic`] is an element of \(\mathbb{Q}(\sqrt{d_1}, \sqrt{d_2},
//! \dots)\) in normal form \(\sum_m c_m \sqrt{m}\), where the monomials
//! `m` are distinct subset products of the value's GCD-free radicand
//! basis and the coefficients are non-zero rationals. Square roots of
//! distinct subset products of a GCD-free basis are linearly independent
//! over ℚ, so a non-empty term map is never zero — the fact that makes
//! sign, order, and equality **decidable without any budget**: zero tests
//! are algebraic on the normal form, and interval refinement is used only
//! to speed up sign resolution, never to gate its correctness.
//!
//! Everything the current vocabulary can construct (√ of rationals closed
//! under field operations) lives in this form. Values that become
//! rational demote eagerly (cheapest-tier-wins): arithmetic never returns
//! an `Algebraic` that merely wraps a rational — callers receive the
//! `Fraction` itself and route it back to Tier 0.

use crate::types::exact::basis::Basis;
use crate::types::exact::squarefree::{squarefree_split, FactorBudgetExhausted};
use crate::types::fraction::Fraction;
use crate::types::fraction_arithmetic::balanced_bigint_gcd;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, Zero};
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// An irrational algebraic number in multiquadratic normal form. The
/// public constructors uphold two invariants: the term map always has a
/// term with a non-`1` monomial (rational values demote to `Fraction`
/// instead), and every coefficient is a non-zero, non-nil rational.
#[derive(Debug, Clone)]
pub struct Algebraic {
    basis: Basis,
    /// Monomial (subset product of `basis`; `1` keys the rational part)
    /// → non-zero rational coefficient.
    terms: BTreeMap<BigInt, Fraction>,
}

/// Result of an arithmetic step: the exact value, already demoted to
/// Tier 0 when it is rational (cheapest-tier-wins).
#[derive(Debug, Clone, PartialEq)]
pub enum AlgebraicResult {
    Rational(Fraction),
    Irrational(Algebraic),
}

impl AlgebraicResult {
    pub(crate) fn from_parts(basis: Basis, terms: BTreeMap<BigInt, Fraction>) -> AlgebraicResult {
        let rational = match terms.len() {
            0 => Some(Fraction::new(BigInt::zero(), BigInt::one())),
            1 => terms
                .iter()
                .next()
                .filter(|(m, _)| m.is_one())
                .map(|(_, c)| c.clone()),
            _ => None,
        };
        match rational {
            Some(f) => AlgebraicResult::Rational(f),
            None => {
                let basis = basis.pruned_to(terms.keys());
                AlgebraicResult::Irrational(Algebraic { basis, terms })
            }
        }
    }

    /// The irrational payload, if any (test/observation convenience).
    /// The rational payload, if the value demoted (test convenience).
    pub fn as_rational(&self) -> Option<&Fraction> {
        match self {
            AlgebraicResult::Rational(f) => Some(f),
            AlgebraicResult::Irrational(_) => None,
        }
    }
}

impl Algebraic {
    // ---- Construction ----

    /// √`radicand`: `None` for a negative radicand,
    /// `Rational` for zero, a perfect square, `1/0` and `0/0`, `Irrational`
    /// otherwise.
    pub fn sqrt_of_fraction(radicand: &Fraction) -> Option<AlgebraicResult> {
        Self::sqrt_of_fraction_within(radicand, &mut u64::MAX.clone())
            .expect("an unbounded budget always factors")
    }

    /// √`radicand` over its square-free part (`squarefree.rs`), charging the
    /// factorization to `budget`: `Err` when the budget cannot factor it.
    pub fn sqrt_of_fraction_within(
        radicand: &Fraction,
        budget: &mut u64,
    ) -> Result<Option<AlgebraicResult>, FactorBudgetExhausted> {
        if !radicand.is_finite() {
            // √(1/0) is 1/0 and √(0/0) is 0/0: a square root over zero is
            // the pair formula's own answer, √(a/0) = √a/√0 = √a/0, reduced
            // to its point.
            return Ok(match radicand.signum() {
                Ordering::Less => None,
                _ => Some(AlgebraicResult::Rational(radicand.clone())),
            });
        }
        let num = radicand.numerator();
        let den = radicand.denominator();
        if num.is_negative() {
            return Ok(None);
        }
        if num.is_zero() {
            return Ok(Some(AlgebraicResult::Rational(Fraction::new(
                BigInt::zero(),
                BigInt::one(),
            ))));
        }
        // √(p/q) = √(p·q)/q, and p·q = s²·m with m square-free, so the value
        // is (s/q)·√m — one normal form whatever built the radicand.
        let (outside, monomial) = squarefree_split(&(&num * &den), budget)?;
        if monomial.is_one() {
            return Ok(Some(AlgebraicResult::Rational(Fraction::new(outside, den))));
        }
        let basis = Basis::build(vec![monomial.clone()]);
        let mut terms = BTreeMap::new();
        terms.insert(monomial, Fraction::new(outside, den));
        Ok(Some(AlgebraicResult::from_parts(basis, terms)))
    }

    /// Number of terms in the normal form (test/diagnostic surface).
    pub fn term_count(&self) -> usize {
        self.terms.len()
    }

    /// The largest coefficient bit-length across the normal form: the max over
    /// every term of `max(numerator bits, denominator bits)`. A cheap size
    /// probe (CS5) used to bound BigInt blow-up — an algebraic value whose
    /// coefficients cross `max_bigint_bits` is a resource-limit failure.
    pub fn max_coefficient_bits(&self) -> u64 {
        self.terms
            .values()
            .map(|c| c.numerator().bits().max(c.denominator().bits()))
            .max()
            .unwrap_or(0)
    }

    /// The largest radicand bit-length across the normal form.
    ///
    /// The companion to `max_coefficient_bits`, and the half that matters most
    /// for pricing: multiplying two-radical sums leaves every coefficient at
    /// ±1 while the *radicands* multiply, so the work of a term pair is driven
    /// by monomial width even when the coefficients say the value is tiny.
    /// Iterates the term keys directly rather than going through
    /// `normal_form_terms`, which allocates — this is charged per operation.
    pub fn max_radicand_bits(&self) -> u64 {
        self.terms.keys().map(|m| m.bits()).max().unwrap_or(0)
    }

    /// Package raw terms produced over `source`'s basis into a result
    /// (demoting to Tier 0 when the shape allows). Internal helper for
    /// the field-operation module.
    pub(crate) fn result_from_parts_of(
        source: &Algebraic,
        terms: BTreeMap<BigInt, Fraction>,
    ) -> AlgebraicResult {
        AlgebraicResult::from_parts(source.basis.clone(), terms)
    }

    /// An irrational from terms already in normal form over `basis`, every
    /// element of which divides one of them, so `from_parts` would neither
    /// prune nor demote; the sum module builds these (`algebraic_sum.rs`).
    pub(crate) fn from_normal_parts(basis: Basis, terms: BTreeMap<BigInt, Fraction>) -> Algebraic {
        Algebraic { basis, terms }
    }

    pub(crate) fn basis(&self) -> &Basis {
        &self.basis
    }

    pub(crate) fn terms(&self) -> &BTreeMap<BigInt, Fraction> {
        &self.terms
    }

    /// The normal form as `(coefficient, radicand)` pairs in ascending
    /// radicand order, with `1` as the radicand of the rational term.
    ///
    /// This is the value itself, not a rendering of it: \(\sum_m c_m\sqrt m\)
    /// *is* the representation, so a reader given these pairs has the exact
    /// number. It exists because every surface an irrational reached was
    /// either long (a thirty-line continued fraction) or lossy (a best rational
    /// approximation drawn in the same `n/d` shape an exact rational has). A
    /// host that can say `√3` or `1/2 + 1/3√5` needs neither.
    pub fn normal_form_terms(&self) -> Vec<(Fraction, BigInt)> {
        self.terms
            .iter()
            .map(|(monomial, coefficient)| (coefficient.clone(), monomial.clone()))
            .collect()
    }

    /// Rebuild `self`'s terms over `target`, which must cover every
    /// monomial (callers merge bases before rebasing).
    pub(crate) fn rebased_terms(&self, target: &Basis) -> BTreeMap<BigInt, Fraction> {
        let mut out: BTreeMap<BigInt, Fraction> = BTreeMap::new();
        for (m, c) in &self.terms {
            let (outside, monomial) = target
                .decompose_sqrt(m)
                .expect("merged basis covers both operands' monomials");
            let scaled = c.mul(&Fraction::new(outside, BigInt::one()));
            add_term(&mut out, monomial, scaled);
        }
        out
    }

    // ---- Ring operations ----

    /// `-self`. Negation never changes rationality, so the result stays
    /// irrational and keeps the basis.
    pub fn neg(&self) -> Algebraic {
        let terms = self
            .terms
            .iter()
            .map(|(m, c)| (m.clone(), c.neg()))
            .collect();
        Algebraic {
            basis: self.basis.clone(),
            terms,
        }
    }

    /// `self + q` for a rational `q`.
    pub fn add_fraction(&self, q: &Fraction) -> AlgebraicResult {
        // Only the rational term changes, and it is keyed by `1`, which no
        // basis element divides: the basis needs no pruning, and the
        // irrational terms that keep `self` irrational are all still there.
        let mut terms = self.terms.clone();
        add_term(&mut terms, BigInt::one(), q.clone());
        AlgebraicResult::Irrational(Algebraic {
            basis: self.basis.clone(),
            terms,
        })
    }

    /// `self · q` for a rational `q`. Multiplying by zero demotes to the
    /// rational `0`; any other rational keeps the value irrational.
    pub fn mul_fraction(&self, q: &Fraction) -> AlgebraicResult {
        if q.is_zero() {
            return AlgebraicResult::Rational(Fraction::new(BigInt::zero(), BigInt::one()));
        }
        let terms = self
            .terms
            .iter()
            .map(|(m, c)| (m.clone(), c.mul(q)))
            .collect();
        AlgebraicResult::from_parts(self.basis.clone(), terms)
    }

    /// `self · other`. For monomials over a shared GCD-free basis,
    /// √m₁·√m₂ = g·√(m₁m₂/g²) with g = gcd(m₁, m₂), again a subset
    /// product — so the product stays in normal form.
    pub fn mul(&self, other: &Algebraic) -> AlgebraicResult {
        let basis = Basis::merged(&self.basis, &other.basis, &[]);
        let a = self.rebased_terms(&basis);
        let b = other.rebased_terms(&basis);
        let mut terms: BTreeMap<BigInt, Fraction> = BTreeMap::new();
        for (m1, c1) in &a {
            for (m2, c2) in &b {
                let g = balanced_bigint_gcd(m1, m2);
                let monomial = (m1 / &g) * (m2 / &g);
                let coeff = c1.mul(c2).mul(&Fraction::new(g, BigInt::one()));
                add_term(&mut terms, monomial, coeff);
            }
        }
        AlgebraicResult::from_parts(basis, terms)
    }

    // ---- Decidable observations ----

    /// Exact sign. A non-empty normal form is non-zero by linear
    /// independence, so interval refinement over rational √ enclosures
    /// separates the sum from zero after finitely many doublings; the
    /// loop is a speed-up of a decidable fact, not a budget.
    pub fn sign(&self) -> Ordering {
        debug_assert!(!self.terms.is_empty(), "normal form of an irrational");
        if self.terms.len() == 1 {
            // c·√m with √m > 0: the sign is the coefficient's.
            let coeff = self.terms.values().next().expect("single term");
            return if coeff.is_positive() {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        let mut bits = 8u64;
        loop {
            let (lo, hi) = self.bounds(bits);
            if lo.is_positive() {
                return Ordering::Greater;
            }
            if hi.numerator().is_negative() {
                return Ordering::Less;
            }
            bits *= 2;
        }
    }

    /// Exact, total three-way comparison with another algebraic value.
    // Not `Ord::cmp`: comparison is a semantic operation on the normal form,
    // deliberately an inherent method, not the derived structural ordering.
    #[allow(clippy::should_implement_trait)]
    pub fn cmp(&self, other: &Algebraic) -> Ordering {
        match self.sub(other) {
            AlgebraicResult::Rational(f) => rational_sign(&f),
            AlgebraicResult::Irrational(d) => d.sign(),
        }
    }

    /// Exact, total three-way comparison with a rational.
    pub fn cmp_fraction(&self, q: &Fraction) -> Ordering {
        match self.add_fraction(&q.neg()) {
            AlgebraicResult::Rational(f) => rational_sign(&f),
            AlgebraicResult::Irrational(d) => d.sign(),
        }
    }

    /// A rational enclosure `[lo, hi]` at 2⁻ᵇⁱᵗˢ per-monomial precision:
    /// with s = ⌊√(m·4ᵇⁱᵗˢ)⌋, √m ∈ [s, s+1]/2ᵇⁱᵗˢ; a point for m = 1.
    /// Deeper `bits` give nested, shrinking enclosures.
    pub fn bounds(&self, bits: u64) -> (Fraction, Fraction) {
        // Summed over one common denominator and reduced once: adding the
        // terms as `Fraction`s took a gcd per term, which made the enclosure
        // of a wide value the dominant cost of hashing, digesting and
        // comparing it (a stack of 2,000 512-term values took ~25 s to digest).
        let common = self
            .terms
            .values()
            .fold(BigInt::one(), |acc, c| acc.lcm(&c.denominator()));
        let scale = BigInt::one() << bits;
        let mut lo = BigInt::zero();
        let mut hi = BigInt::zero();
        for (m, c) in &self.terms {
            // √m ∈ [m_lo, m_hi] / 2^bits.
            let (m_lo, m_hi) = if m.is_one() {
                (scale.clone(), scale.clone())
            } else {
                let s = (m.clone() << (2 * bits)).sqrt();
                let s_plus = &s + 1;
                (s, s_plus)
            };
            let (num, den) = c.to_bigint_pair();
            let weight = num * (&common / den);
            if weight.is_negative() {
                lo += &weight * m_hi;
                hi += weight * m_lo;
            } else {
                lo += &weight * m_lo;
                hi += weight * m_hi;
            }
        }
        let denominator = common << bits;
        (
            Fraction::new(lo, denominator.clone()),
            Fraction::new(hi, denominator),
        )
    }
}

/// Semantic equality: two normal forms may differ structurally when their
/// bases have different granularity (√12 alone keeps the coarse basis
/// {12}; √12 built as 2·√3 carries {3}), so equality is decided by
/// comparison, which rebases both sides over a common refinement.
impl PartialEq for Algebraic {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Algebraic {}

/// Bits of fractional precision the hash key is taken at. Chosen generously
/// past what any realistic near-collision needs — collisions only cost a
/// fallback `==` inside the bucket, never wrong dedup — so it is a
/// performance knob, not a correctness one.
const HASH_KEY_BITS: u64 = 64;

/// `floor(f * 2^target_bits)`, exactly (`BigInt` division, not `f64`). Also
/// the value digest's algebraic key (`interpreter::value_digest`).
pub(crate) fn floor_scaled(f: &Fraction, target_bits: u64) -> BigInt {
    let (num, den) = f.to_bigint_pair();
    (num << target_bits).div_floor(&den)
}

impl std::hash::Hash for Algebraic {
    /// A representation-independent hash key: `floor(self * 2^HASH_KEY_BITS)`.
    ///
    /// Radicands are square-free (`squarefree.rs`), so equal values carry
    /// equal terms; the basis they carry still depends on history (`√6` over
    /// `{6}` or over `{2, 3}`), and a probable prime misjudged above the
    /// proven range would leave a radicand that is not square-free. Hashing
    /// the number rather than any structure is immune to both.
    ///
    /// This reuses the decidability `sign` already relies on:
    /// `bounds(bits)` is a shrinking enclosure of the true value, and an
    /// irrational value (the only kind this type holds; rationals demote
    /// eagerly) is never exactly on a `2^-HASH_KEY_BITS` grid line. So
    /// doubling `bits` until the enclosure's floor is decided always
    /// terminates, and it converges to the same integer for any two
    /// representations of the same number — the key is a property of the
    /// *number*, not of the basis or term structure used to reach it.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let mut bits = HASH_KEY_BITS + 8;
        loop {
            let (lo, hi) = self.bounds(bits);
            let lo_key = floor_scaled(&lo, HASH_KEY_BITS);
            let hi_key = floor_scaled(&hi, HASH_KEY_BITS);
            if lo_key == hi_key {
                lo_key.hash(state);
                return;
            }
            debug_assert!(
                bits < (1 << 24),
                "algebraic hash key failed to converge — an irrational value \
                 should never sit exactly on a precision grid line"
            );
            bits *= 2;
        }
    }
}

fn rational_sign(f: &Fraction) -> Ordering {
    if f.is_zero() {
        Ordering::Equal
    } else if f.is_positive() {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

/// Merge a term into a coefficient map, dropping the monomial when the
/// coefficients cancel (the no-zero-coefficients invariant). Answers whether
/// a monomial the map held cancelled away.
pub(crate) fn add_term(
    terms: &mut BTreeMap<BigInt, Fraction>,
    monomial: BigInt,
    coeff: Fraction,
) -> bool {
    if coeff.is_zero() {
        return false;
    }
    match terms.entry(monomial) {
        std::collections::btree_map::Entry::Vacant(e) => {
            e.insert(coeff);
            false
        }
        std::collections::btree_map::Entry::Occupied(mut e) => {
            let sum = e.get().add(&coeff);
            if sum.is_zero() {
                e.remove();
                true
            } else {
                e.insert(sum);
                false
            }
        }
    }
}
