//! The two numbers with denominator 0, and the arithmetic that reaches them
//! (LANG.VALUES.EXACT): Ajisai computes in the wheel of fractions over the
//! integers.
//!
//! A number is an integer pair taken up to a nonzero integer factor:
//! `(a, b)` and `(ka, kb)` are one number for every `k ≠ 0`. The normal form
//! divides both halves by their gcd and makes the denominator non-negative.
//! Since `gcd(n, 0) = |n|`, and a factor of `-1` relates `(1, 0)` and
//! `(-1, 0)`, every pair over zero reduces to one of two: `1/0`, the one
//! unsigned point at infinity, and `0/0`, the wheel's bottom. Nothing here is
//! added to the rationals by hand — `100 0 DIV` and `-5 0 DIV` are both `1/0`
//! for the reason `4/2` is `2/1`.
//!
//! The operations are the wheel's pair formulas, `a/b + c/d = (ad + bc)/bd`,
//! `a/b · c/d = ac/bd`, and the reciprocal `/(a/b) = b/a`, which needs no
//! sign rule because `(b, a)` and `(-b, -a)` are one number. Division is
//! multiplication by the reciprocal. Over a pair with denominator 0 only
//! whether a finite operand is zero reaches the result (`ad` with `d = 0`),
//! which is why a finite operand enters these formulas as its sign over 1
//! (`sign_pair`): the answer is the same, and it is all an irrational operand
//! has to offer them too.
//!
//! What follows from the formulas, and is not decided separately:
//! `x + 1/0 = 1/0` for finite `x`, `1/0 + 1/0 = 0/0`, `0 · 1/0 = 0/0`,
//! `1 ÷ 1/0 = 0`, `1/0 ÷ 1/0 = 0/0`, `0/0` absorbs every operation, and the
//! reciprocal is an involution. Every axiom of a wheel holds at both points;
//! the field laws that mention zero — `0x = 0`, `x - x = 0`, `x/x = 1` and
//! distributivity — give way there, and nowhere else.

use super::fraction::{Fraction, FractionRepr};
use num_bigint::BigInt;

impl Fraction {
    /// `1/0`, the quotient of any nonzero number by zero.
    #[inline]
    pub fn infinity() -> Self {
        Fraction::from_repr(FractionRepr::Small(1, 0))
    }

    /// `0/0`, the quotient of zero by zero.
    #[inline]
    pub fn nullity() -> Self {
        Fraction::from_repr(FractionRepr::Small(0, 0))
    }

    /// The reduced pair over zero a numerator of sign `sign` reduces to:
    /// `1/0` for a nonzero one, whatever its sign, and `0/0` for zero.
    #[inline]
    pub(crate) fn over_zero(sign: std::cmp::Ordering) -> Self {
        match sign {
            std::cmp::Ordering::Equal => Self::nullity(),
            _ => Self::infinity(),
        }
    }

    /// Whether the denominator is non-zero: the number is a rational.
    #[inline]
    pub fn is_finite(&self) -> bool {
        !matches!(self.repr, FractionRepr::Small(_, 0))
    }

    /// Whether this is `0/0`.
    #[inline]
    pub fn is_nullity(&self) -> bool {
        matches!(self.repr, FractionRepr::Small(0, 0))
    }

    /// The sign of the numerator, which is the sign of a rational: the
    /// denominator is never negative. `1/0` reads as `Greater` and `0/0` as
    /// `Equal`; neither is a sign of the number, which has none.
    #[inline]
    pub fn signum(&self) -> std::cmp::Ordering {
        match &self.repr {
            FractionRepr::Small(n, _) => n.cmp(&0),
            FractionRepr::Big(big) => big.numerator.sign().cmp(&num_bigint::Sign::NoSign),
        }
    }

    /// The pair this number enters an operation with when the other operand
    /// has denominator 0: its own pair for one over zero, and its sign over 1
    /// for a finite one, since only whether a finite operand is zero reaches
    /// such a result.
    #[inline]
    fn sign_pair(&self) -> (i64, i64) {
        let sign = ordering_to_i64(self.signum());
        if self.is_finite() {
            (sign, 1)
        } else {
            (sign, 0)
        }
    }

    /// `self ± other` when at least one operand has denominator 0.
    pub(crate) fn extended_add(&self, other: &Fraction, subtract: bool) -> Fraction {
        debug_assert!(!self.is_finite() || !other.is_finite());
        let (an, ad) = self.sign_pair();
        let (bn, bd) = other.sign_pair();
        let bn = if subtract { -bn } else { bn };
        reduced_small(an * bd + bn * ad, ad * bd)
    }

    /// `self · other` when at least one operand has denominator 0.
    pub(crate) fn extended_mul(&self, other: &Fraction) -> Fraction {
        debug_assert!(!self.is_finite() || !other.is_finite());
        let (an, ad) = self.sign_pair();
        let (bn, bd) = other.sign_pair();
        reduced_small(an * bn, ad * bd)
    }

    /// `/self`: the pair swapped. Total and an involution — the reciprocal
    /// of `0` is `1/0`, of `1/0` is `0`, and of `0/0` is `0/0`.
    pub fn reciprocal(&self) -> Fraction {
        match &self.repr {
            FractionRepr::Small(n, d) => {
                if *d == 0 || *n == 0 {
                    // A point, or zero: every answer is one of the small
                    // pairs below.
                    return match (*n, *d) {
                        (0, 0) => Self::nullity(),
                        (_, 0) => Fraction::from(0),
                        _ => Self::infinity(),
                    };
                }
                if *n < 0 {
                    match (d.checked_neg(), n.checked_neg()) {
                        (Some(num), Some(den)) => {
                            Fraction::from_repr(FractionRepr::Small(num, den))
                        }
                        _ => Fraction::new(-BigInt::from(*d), -BigInt::from(*n)),
                    }
                } else {
                    Fraction::from_repr(FractionRepr::Small(*d, *n))
                }
            }
            FractionRepr::Big(big) => {
                Fraction::create_already_reduced(big.denominator.clone(), big.numerator.clone())
            }
        }
    }
}

#[inline]
fn ordering_to_i64(sign: std::cmp::Ordering) -> i64 {
    match sign {
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
    }
}

/// A pair whose halves are each in `-2..=2`, reduced: over zero it is one of
/// the two points, and otherwise it is already in lowest terms or is zero.
#[inline]
fn reduced_small(numerator: i64, denominator: i64) -> Fraction {
    if denominator == 0 {
        return Fraction::over_zero(numerator.cmp(&0));
    }
    Fraction::create_from_i128(i128::from(numerator), i128::from(denominator))
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_traits::Signed;

    fn n(v: i64) -> Fraction {
        Fraction::from(v)
    }
    fn inf() -> Fraction {
        Fraction::infinity()
    }
    fn phi() -> Fraction {
        Fraction::nullity()
    }

    #[test]
    fn a_pair_over_zero_reduces_to_one_of_two_points() {
        assert_eq!(Fraction::new(100.into(), 0.into()), inf());
        assert_eq!(Fraction::new((-5).into(), 0.into()), inf());
        assert_eq!(Fraction::new(0.into(), 0.into()), phi());
        assert_eq!(Fraction::from_str("100/0").unwrap(), inf());
        assert_eq!(Fraction::from_str("-7/0").unwrap(), inf());
        assert_eq!(Fraction::from_str("0/0").unwrap(), phi());
        assert_eq!(inf().to_string(), "1/0");
        assert_eq!(phi().to_string(), "0/0");
        // A negative denominator carries its sign into the numerator.
        assert_eq!(
            Fraction::new(3.into(), (-4).into()),
            Fraction::new((-3).into(), 4.into())
        );
    }

    #[test]
    fn the_two_points_are_two_values() {
        assert_ne!(inf(), phi());
        assert_ne!(inf(), n(1));
        assert_ne!(phi(), n(0));
        assert_eq!(inf(), inf());
        assert_eq!(phi(), phi());
    }

    #[test]
    fn division_by_zero_is_the_point_at_infinity() {
        assert_eq!(n(100).div(&n(0)), inf());
        assert_eq!(n(-3).div(&n(0)), inf());
        assert_eq!(n(0).div(&n(0)), phi());
        let half = Fraction::new(1.into(), 2.into());
        assert_eq!(half.div(&n(0)), inf());
    }

    #[test]
    fn the_pair_formulas_decide_the_two_points() {
        // Addition: a finite operand contributes nothing beside 1/0.
        assert_eq!(inf().add(&n(5)), inf());
        assert_eq!(n(-5).add(&inf()), inf());
        assert_eq!(inf().add(&inf()), phi());
        assert_eq!(inf().sub(&inf()), phi());
        assert_eq!(n(1).sub(&inf()), inf());
        assert_eq!(phi().add(&n(1)), phi());
        // Multiplication: zero against 1/0 is 0/0, anything else is 1/0.
        assert_eq!(inf().mul(&n(2)), inf());
        assert_eq!(inf().mul(&n(-2)), inf());
        assert_eq!(inf().mul(&inf()), inf());
        assert_eq!(inf().mul(&n(0)), phi());
        assert_eq!(n(0).mul(&inf()), phi());
        assert_eq!(phi().mul(&n(0)), phi());
        // Division: by the reciprocal.
        assert_eq!(n(1).div(&inf()), n(0));
        assert_eq!(inf().div(&inf()), phi());
        assert_eq!(inf().div(&n(-2)), inf());
        assert_eq!(inf().div(&n(0)), inf());
        assert_eq!(n(1).div(&phi()), phi());
        assert_eq!(phi().div(&n(0)), phi());
    }

    #[test]
    fn the_reciprocal_is_a_total_involution() {
        assert_eq!(n(0).reciprocal(), inf());
        assert_eq!(inf().reciprocal(), n(0));
        assert_eq!(phi().reciprocal(), phi());
        assert_eq!(n(-2).reciprocal(), Fraction::new((-1).into(), 2.into()));
        assert_eq!(
            Fraction::new(3.into(), 4.into()).reciprocal(),
            Fraction::new(4.into(), 3.into())
        );
        assert_eq!(
            n(i64::MIN).reciprocal(),
            Fraction::new((-1).into(), BigInt::from(i64::MIN).abs())
        );
        for x in [n(0), n(-2), n(3), inf(), phi()] {
            assert_eq!(x.reciprocal().reciprocal(), x);
        }
    }

    #[test]
    fn distributivity_gives_way_at_the_two_points() {
        // (1 + 1) · 1/0 = 1/0, but 1/0 + 1/0 = 0/0.
        let lhs = n(1).add(&n(1)).mul(&inf());
        let rhs = n(1).mul(&inf()).add(&n(1).mul(&inf()));
        assert_eq!(lhs, inf());
        assert_eq!(rhs, phi());
        assert_ne!(lhs, rhs);
    }

    #[test]
    fn floor_round_and_negation_over_the_two_points() {
        assert_eq!(inf().neg(), inf());
        assert_eq!(phi().neg(), phi());
        assert_eq!(inf().abs(), inf());
        assert_eq!(inf().floor(), inf());
        assert_eq!(phi().round(), phi());
        assert!(!phi().is_positive());
        assert!(!phi().is_zero());
        assert!(!inf().is_integer());
    }
}
