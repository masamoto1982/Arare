//! `xʸ` inside the exact field (LANG.VALUES.EXACT).
//!
//! An integer exponent stays in the base's own tier: a rational base powers
//! exactly, an algebraic base by repeated multiplication in the field. An
//! exponent `p/2` over a non-negative rational base stays in the field too —
//! `x^(p/2)` is `(√x)ᵖ`, which is why `SQRT` remains the Word that builds the
//! field and `POW` is not sugar for it. A negative base under `p/2` has no
//! real value, and every other exponent — a denominator other than 1 or 2, an
//! algebraic base under `p/2`, an irrational exponent — leaves the field:
//! both are `DomainMiss`. A zero base under a negative exponent divides by
//! zero, and answers `1/0` as division does; a base over zero powers by the
//! same products, so `1/0 2 POW` is `1/0` and `1/0 -1 POW` is `0`. An
//! exponent over zero is neither an integer nor `p/2`, and leaves the field
//! like any other.

use std::cmp::Ordering;

use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::types::exact::algebraic::Algebraic;
use crate::types::exact::value::ExactReal;
use crate::types::fraction::Fraction;

/// What asking for a power produced.
#[derive(Debug, Clone)]
pub enum PowOutcome {
    Value(ExactReal),
    /// A negative base under `p/2`, or an answer outside the field.
    DomainMiss,
    /// A result wider than [`INTEGER_POWER_RESULT_BITS`], the bound this
    /// module keeps for itself; the host's own `bigintBits` ceiling is
    /// checked by the caller against [`ExactReal::power_size`] first.
    TooWide,
    /// The work budget could not factor the base's radicand into its
    /// square-free normal form (`squarefree.rs`).
    WorkExhausted,
}

/// Bits the result of an integer power may reach: the exponent times the
/// base's own bit length, so `2^(500000)` is answered and `999^1000001`,
/// a ten-million-bit number no comparison will read, is refused.
const INTEGER_POWER_RESULT_BITS: u64 = 1 << 20;

/// The sign of an exact real. Decidable over the whole domain.
fn sign_of(x: &ExactReal) -> Ordering {
    x.signum()
}

/// `xⁿ` for an integer `n ≥ 0` by square-and-multiply, in `x`'s own tier.
fn integer_power(x: &ExactReal, n: &BigInt) -> ExactReal {
    if let Some(q) = x.as_rational() {
        let (num, den) = q.to_bigint_pair();
        let e = u32::try_from(n).expect("bounded by INTEGER_POWER_RESULT_BITS");
        return ExactReal::from_fraction(Fraction::new(num.pow(e), den.pow(e)));
    }
    let mut result = ExactReal::from_fraction(Fraction::from(1));
    let mut base = x.clone();
    let mut e = n.clone();
    while !e.is_zero() {
        if e.is_odd() {
            result = result.mul(&base);
        }
        e >>= 1;
        if !e.is_zero() {
            base = base.mul(&base);
        }
    }
    result
}

/// What an exponent asks of a base, before any power is taken.
#[derive(Debug, Clone)]
pub enum PowPlan {
    /// Answered without taking a power: a projection, or `0^(p/2)`.
    Answered(PowOutcome),
    /// `base` raised to the integer `exponent`: `x` itself, or `√x` under an
    /// exponent `p/2`.
    Integer { base: ExactReal, exponent: BigInt },
}

/// The most a power can occupy, read off its base before it is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerSize {
    /// The widest coefficient half, in bits.
    pub bits: u64,
    /// The most terms its normal form can hold.
    pub terms: u64,
}

impl ExactReal {
    /// `self` raised to `exponent`, with no bound on the work a root's
    /// normal form may take.
    pub fn pow(&self, exponent: &ExactReal) -> PowOutcome {
        match self.plan_pow_within(exponent, &mut u64::MAX.clone()) {
            PowPlan::Answered(outcome) => outcome,
            PowPlan::Integer { base, exponent } => base.power_by_integer(&exponent),
        }
    }

    /// What `self` raised to `exponent` comes to before a power is taken,
    /// charging a root's factorization to `budget`. The power itself is the
    /// caller's to take, by [`Self::power_by_integer`], once it has read
    /// [`Self::power_size`] and decided the power is worth taking.
    pub fn plan_pow_within(&self, exponent: &ExactReal, budget: &mut u64) -> PowPlan {
        // `0/0` absorbs every operation (LANG.VALUES.EXACT), in either
        // operand: `0/0 0 POW` is not the empty product and `2 0/0 POW` is
        // not a domain miss, since the nullity met the operation first.
        let is_nullity = |x: &ExactReal| x.as_rational().is_some_and(Fraction::is_nullity);
        if is_nullity(self) || is_nullity(exponent) {
            return PowPlan::Answered(PowOutcome::Value(ExactReal::from_fraction(
                Fraction::nullity(),
            )));
        }
        let Some(y) = exponent.as_rational() else {
            return PowPlan::Answered(PowOutcome::DomainMiss);
        };
        let (p, q) = y.to_bigint_pair();
        if q.is_one() {
            return PowPlan::Integer {
                base: self.clone(),
                exponent: p,
            };
        }
        if q != BigInt::from(2) {
            return PowPlan::Answered(PowOutcome::DomainMiss);
        }
        PowPlan::Answered(match sign_of(self) {
            Ordering::Less => PowOutcome::DomainMiss,
            // `0^(p/2)` is `0^p`: 0 for a positive `p`, and `1/0` for a
            // negative one, as the reciprocal of zero is.
            Ordering::Equal => {
                return PowPlan::Integer {
                    base: ExactReal::from_fraction(Fraction::from(0)),
                    exponent: p,
                }
            }
            Ordering::Greater => match self.as_rational() {
                Some(x) => match ExactReal::try_sqrt_rational(x.clone(), budget) {
                    Ok(root) => {
                        return PowPlan::Integer {
                            base: root.expect("a positive rational has a square root"),
                            exponent: p,
                        }
                    }
                    Err(_) => PowOutcome::WorkExhausted,
                },
                None => PowOutcome::DomainMiss,
            },
        })
    }

    /// `self^n` for an integer exponent.
    pub fn power_by_integer(&self, n: &BigInt) -> PowOutcome {
        let base_bits = BigInt::from(self.max_coefficient_bits().max(2));
        if n.abs() * base_bits > BigInt::from(INTEGER_POWER_RESULT_BITS) {
            return PowOutcome::TooWide;
        }
        if n.is_zero() {
            return PowOutcome::Value(ExactReal::from_fraction(Fraction::from(1)));
        }
        let positive = integer_power(self, &n.abs());
        if n.is_positive() {
            return PowOutcome::Value(positive);
        }
        PowOutcome::Value(positive.reciprocal())
    }

    /// How wide `self^n` can be, and how many terms it can hold, without
    /// taking it.
    ///
    /// Write `self` as `(1/d)·Σ aᵢ√mᵢ` with integer `aᵢ`, and let
    /// `S = Σ |aᵢ|√mᵢ`. `S` is submultiplicative — `√m·√m′` is exactly
    /// `√(m·m′)`, whatever square leaves it — so every coefficient of
    /// `(Σ aᵢ√mᵢ)ⁿ` is at most `Sⁿ` and every denominator divides `dⁿ`: a
    /// positive power is at most `n·log₂ max(S, d)` bits wide. For a rational
    /// that is the width of `aⁿ` and `dⁿ` exactly. A negative power inverts
    /// the positive one by its conjugates — a product of one fewer of them
    /// than the field's degree, over their norm — so it is bounded by that
    /// degree, plus one, times as many bits.
    ///
    /// The terms: `xⁿ` is `√m₀ⁿ` times a power of `x/√m₀`, whose monomials
    /// all lie in the span, over GF(2), of the differences `mᵢ ⊕ m₀` between
    /// `x`'s monomials as sets of basis elements. So every power, negative
    /// ones included, holds at most `2^rank` terms; a positive one also at
    /// most as many as there are ways to choose `n` of the `t` terms.
    pub fn power_size(&self, n: &BigInt) -> PowerSize {
        let terms: Vec<(Fraction, BigInt)> = match self {
            ExactReal::Rational(q) => vec![(q.clone(), BigInt::one())],
            ExactReal::Algebraic(a) => a.normal_form_terms(),
        };
        let lcm = terms
            .iter()
            .fold(BigInt::one(), |lcm, (c, _)| lcm.lcm(&c.denominator()));
        let log_lcm = log2_magnitude(&lcm);
        // log₂ S, summed in the log domain so no coefficient overflows a float.
        let logs: Vec<f64> = terms
            .iter()
            .map(|(c, m)| {
                log2_magnitude(&c.numerator()) + log_lcm - log2_magnitude(&c.denominator())
                    + log2_magnitude(m) / 2.0
            })
            .collect();
        let top = logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let log_s = top + logs.iter().map(|l| (l - top).exp2()).sum::<f64>().log2();
        let width = log_s.max(log_lcm).max(0.0);
        let exponent = n.abs().to_f64().unwrap_or(f64::INFINITY);
        // `0 · ∞` (a base of ±1 under a vast exponent) is one bit, not NaN.
        let product = if width == 0.0 { 0.0 } else { exponent * width };
        let positive_bits = (product.floor() as u64).saturating_add(1);
        let ExactReal::Algebraic(a) = self else {
            return PowerSize {
                bits: positive_bits,
                terms: 1,
            };
        };
        let (span, degree) = radical_ranks(a);
        let span_terms = 1u64.checked_shl(span).unwrap_or(u64::MAX);
        if n.is_negative() {
            let degree_terms = 1u64.checked_shl(degree).unwrap_or(u64::MAX);
            return PowerSize {
                bits: positive_bits.saturating_mul(degree_terms.saturating_add(1)),
                terms: span_terms,
            };
        }
        PowerSize {
            bits: positive_bits,
            terms: span_terms.min(multisets(a.term_count() as u64, n, span_terms)),
        }
    }
}

/// log₂|h|, or 0 when `|h| ≤ 1`; read off the top 53 bits so a value wider
/// than a float still has one.
fn log2_magnitude(h: &BigInt) -> f64 {
    let bits = h.bits();
    if bits <= 1 {
        return 0.0;
    }
    let shift = bits.saturating_sub(53);
    let top = (h.magnitude() >> shift)
        .to_f64()
        .expect("53 bits fit a float");
    top.log2() + shift as f64
}

/// The ranks over GF(2) of the monomials of `a` read as sets of its basis
/// elements: of the differences between them (the span every power of `a`
/// lies in a coset of), and of the monomials themselves (the degree of the
/// field they generate).
fn radical_ranks(a: &Algebraic) -> (u32, u32) {
    let basis = a.basis().elements();
    let words = basis.len().div_ceil(64);
    let set_of = |m: &BigInt| {
        let mut set = vec![0u64; words];
        for (i, b) in basis.iter().enumerate() {
            if (m % b).is_zero() {
                set[i / 64] |= 1 << (i % 64);
            }
        }
        set
    };
    let mut monomials = a.terms().keys().map(set_of);
    let first = monomials.next().expect("an Algebraic has terms");
    // Each row is reduced against the rows before it, so it is clear at their
    // pivots, and reducing a set against the rows in order clears them all.
    let reduce = |rows: &[(usize, Vec<u64>)], mut set: Vec<u64>| {
        for (pivot, row) in rows {
            if set[pivot / 64] >> (pivot % 64) & 1 == 1 {
                set.iter_mut().zip(row).for_each(|(s, r)| *s ^= r);
            }
        }
        let pivot = (0..words * 64).find(|i| set[i / 64] >> (i % 64) & 1 == 1);
        pivot.map(|pivot| (pivot, set))
    };
    let mut rows: Vec<(usize, Vec<u64>)> = Vec::new();
    for set in monomials {
        let difference = set.iter().zip(&first).map(|(s, f)| s ^ f).collect();
        if let Some(row) = reduce(&rows, difference) {
            rows.push(row);
        }
    }
    let span = rows.len() as u32;
    let degree = span + u32::from(reduce(&rows, first).is_some());
    (span, degree)
}

/// How many ways there are to choose `n` of `t` terms with repetition,
/// `C(t+n-1, n)`, or anything past `cap` once it is.
fn multisets(t: u64, n: &BigInt, cap: u64) -> u64 {
    let Some(n) = n.to_u128() else {
        return cap.saturating_add(1);
    };
    // C(n+i, i) = C(n+i-1, i-1) · (n+i) / i, exactly, for i up to t-1.
    let mut count: u128 = 1;
    for i in 1..u128::from(t) {
        count = match count.checked_mul(n + i) {
            Some(product) => product / i,
            None => return cap.saturating_add(1),
        };
        if count > u128::from(cap) {
            return cap.saturating_add(1);
        }
    }
    count as u64
}
