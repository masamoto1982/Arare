//! GCD-free radicand basis for the Tier 1 algebraic normal form.
//!
//! Instead of factoring radicands into primes (unbounded cost for large
//! semiprimes), the normal form works over a **GCD-free basis**:
//! pairwise-coprime integers ≥ 2, none a perfect square, covering every
//! radicand as a product of powers. Distinct subset products of such a
//! basis are never perfect squares — exactly the hypothesis the
//! linear-independence theorem needs — so a coefficient map keyed by
//! subset products is a true normal form; primality is never required.
//! (Same construction as the LANG.VALUES.EXACT comparison pre-pass this module's
//! type grew out of.)

use crate::types::exact::squarefree::exact_sqrt;
use crate::types::fraction_arithmetic::balanced_bigint_gcd;
use num_bigint::BigInt;
use num_traits::{One, Zero};

/// Pairwise-coprime positive integers, each ≥ 2 and none a perfect
/// square, kept sorted. Every monomial of an [`super::algebraic::Algebraic`]
/// value is a product of a distinct subset of its basis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Basis(Vec<BigInt>);

impl Basis {
    pub(crate) fn elements(&self) -> &[BigInt] {
        &self.0
    }

    /// Build a GCD-free basis covering every input integer. Inputs ≤ 1
    /// are ignored (they carry no radical content).
    pub(crate) fn build(inputs: Vec<BigInt>) -> Basis {
        let one = BigInt::one();
        let mut elems: Vec<BigInt> = inputs.into_iter().filter(|n| *n > one).collect();
        elems.sort();
        elems.dedup();
        // GCD-free refinement: replace any pair sharing a factor by
        // {gcd, a/gcd, b/gcd}. Each replacement strictly decreases the
        // product of all elements, so the loop terminates.
        loop {
            let mut shared: Option<(usize, usize)> = None;
            'search: for i in 0..elems.len() {
                for j in (i + 1)..elems.len() {
                    if !balanced_bigint_gcd(&elems[i], &elems[j]).is_one() {
                        shared = Some((i, j));
                        break 'search;
                    }
                }
            }
            let Some((i, j)) = shared else { break };
            let a = elems.swap_remove(j);
            let b = elems.swap_remove(i);
            let g = balanced_bigint_gcd(&a, &b);
            for part in [&a / &g, &b / &g, g] {
                if part > one && !elems.contains(&part) {
                    elems.push(part);
                }
            }
        }
        // Square reduction: a basis element that is a perfect square would
        // make √element rational, breaking independence. Replacing b = c²
        // by c preserves pairwise coprimality (any common factor of c and
        // another element would divide b too) and keeps every input a
        // product of basis-element powers.
        for e in &mut elems {
            while let Some(root) = exact_sqrt(e) {
                *e = root;
            }
        }
        elems.sort();
        Basis(elems)
    }

    /// The common refinement of two bases plus any extra radicands: the
    /// GCD-free basis over which every monomial of either side (and every
    /// extra radicand) decomposes.
    pub(crate) fn merged(a: &Basis, b: &Basis, extra: &[BigInt]) -> Basis {
        if extra.is_empty() {
            return Self::merged_where_tangled(a, b);
        }
        let mut inputs: Vec<BigInt> = Vec::with_capacity(a.0.len() + b.0.len() + extra.len());
        inputs.extend(a.0.iter().cloned());
        inputs.extend(b.0.iter().cloned());
        inputs.extend(extra.iter().cloned());
        Self::build(inputs)
    }

    /// The common refinement of two bases, refining only what needs it.
    ///
    /// An element of either basis that is equal or coprime to every element
    /// of the other is already coprime to everything the merge can produce:
    /// its own basis is pairwise coprime, so it shares no factor with any
    /// element that does tangle, nor with the pieces refining those yields.
    /// Only the tangled elements go through `build`, and the rest are kept as
    /// they are — for a sum gaining a new prime that is all of them, and the
    /// merge costs `|a|·|b|` gcds instead of the refinement's search over
    /// every pair of the union.
    fn merged_where_tangled(a: &Basis, b: &Basis) -> Basis {
        let mut tangled_a = vec![false; a.0.len()];
        let mut tangled_b = vec![false; b.0.len()];
        for (i, x) in a.0.iter().enumerate() {
            for (j, y) in b.0.iter().enumerate() {
                if x != y && !balanced_bigint_gcd(x, y).is_one() {
                    tangled_a[i] = true;
                    tangled_b[j] = true;
                }
            }
        }
        let mut elems = Vec::with_capacity(a.0.len() + b.0.len());
        let mut tangled = Vec::new();
        for (e, t) in a.0.iter().zip(tangled_a).chain(b.0.iter().zip(tangled_b)) {
            if t {
                tangled.push(e.clone());
            } else {
                elems.push(e.clone());
            }
        }
        if !tangled.is_empty() {
            elems.extend(Self::build(tangled).0);
        }
        elems.sort();
        elems.dedup();
        Basis(elems)
    }

    /// The elements of `self` that `other` does not have, as a basis: the
    /// elements a merge made, read off the merge and one operand's basis.
    pub(crate) fn without(&self, other: &Basis) -> Basis {
        Basis(
            self.0
                .iter()
                .filter(|e| !other.contains(e))
                .cloned()
                .collect(),
        )
    }

    /// Whether `e` is one of this basis's elements.
    pub(crate) fn contains(&self, e: &BigInt) -> bool {
        self.0.binary_search(e).is_ok()
    }

    /// √n as `outside · √monomial` over this basis: factor n into
    /// basis-element powers, halve even exponents into `outside`, and keep
    /// the odd-exponent elements' product as the monomial key. `None` when
    /// n is not a product of basis-element powers — the callers all build
    /// the basis from the very integers they decompose, so `None` is a
    /// defensive impossibility, never a wrong answer.
    pub(crate) fn decompose_sqrt(&self, n: &BigInt) -> Option<(BigInt, BigInt)> {
        let mut rest = n.clone();
        let mut outside = BigInt::one();
        let mut monomial = BigInt::one();
        for b in &self.0 {
            let mut exp = 0u32;
            while (&rest % b).is_zero() {
                rest /= b;
                exp += 1;
            }
            outside *= b.pow(exp / 2);
            if exp % 2 == 1 {
                monomial *= b;
            }
        }
        if !rest.is_one() {
            return None;
        }
        Some((outside, monomial))
    }

    /// Drop the elements dividing none of `monomials`, keeping merges
    /// between long-lived values from accreting dead radicands.
    pub(crate) fn pruned_to<'a>(
        &self,
        monomials: impl Iterator<Item = &'a BigInt> + Clone,
    ) -> Basis {
        let kept: Vec<BigInt> = self
            .0
            .iter()
            .filter(|b| monomials.clone().any(|m| (m % *b).is_zero()))
            .cloned()
            .collect();
        Basis(kept)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bi(n: i64) -> BigInt {
        BigInt::from(n)
    }

    #[test]
    fn build_refines_shared_factors_and_squares() {
        // {6, 10} share 2 → {2, 3, 5}.
        let b = Basis::build(vec![bi(6), bi(10)]);
        assert_eq!(b.elements(), &[bi(2), bi(3), bi(5)]);
        // A perfect-square input reduces to its root.
        let b = Basis::build(vec![bi(49)]);
        assert_eq!(b.elements(), &[bi(7)]);
        // Inputs ≤ 1 are ignored.
        let b = Basis::build(vec![bi(1), bi(0)]);
        assert_eq!(b.elements(), &[] as &[BigInt]);
    }

    #[test]
    fn decompose_sqrt_splits_square_part_from_monomial() {
        let b = Basis::build(vec![bi(2), bi(3)]);
        // √72 = √(2³·3²) = 2·3·√2.
        let (outside, monomial) = b.decompose_sqrt(&bi(72)).unwrap();
        assert_eq!(outside, bi(6));
        assert_eq!(monomial, bi(2));
        // √1 = 1·√1.
        let (outside, monomial) = b.decompose_sqrt(&bi(1)).unwrap();
        assert_eq!(outside, bi(1));
        assert_eq!(monomial, bi(1));
        // 5 is not covered by {2, 3}.
        assert!(b.decompose_sqrt(&bi(5)).is_none());
    }

    #[test]
    fn merged_covers_monomials_of_both_sides() {
        // √12 alone keeps the coarse basis {12}; meeting √3 refines it.
        let a = Basis::build(vec![bi(12)]);
        assert_eq!(a.elements(), &[bi(12)]);
        let b = Basis::build(vec![bi(3)]);
        let m = Basis::merged(&a, &b, &[]);
        assert_eq!(m.elements(), &[bi(2), bi(3)]);
        // The old coarse monomial still decomposes over the refinement.
        let (outside, monomial) = m.decompose_sqrt(&bi(12)).unwrap();
        assert_eq!(outside, bi(2));
        assert_eq!(monomial, bi(3));
    }

    #[test]
    fn pruned_to_drops_unused_elements() {
        let b = Basis::build(vec![bi(2), bi(3), bi(5)]);
        let monomials = [bi(6)];
        let p = b.pruned_to(monomials.iter());
        assert_eq!(p.elements(), &[bi(2), bi(3)]);
    }
}
