//! Addition on the Tier 1 normal form. Split from `algebraic.rs` to respect
//! the file-size budget (the file-size budget in
//! docs/dev/specification-implementation-rules.md).
//!
//! A sum merges the two bases, carries both operands' terms over to the merge
//! and drops basis elements no surviving monomial needs. A fold adding one
//! radical at a time to a long sum — `1 1000 RANGE SQRT 0 [ ADD ] FOLD` —
//! once paid all three in full on every step, for a sum of hundreds of terms,
//! although almost every step changes nothing they compute. Each now does only
//! the part that can change something:
//!
//! - the merge refines only the elements that share a factor across the two
//!   bases (`Basis::merged`);
//! - a term is decomposed again only if its monomial contains an element the
//!   merge split; every other monomial is a product of elements the merge
//!   kept, and is already a monomial over it;
//! - the basis is pruned only after a cancellation. Without one every
//!   monomial survives, every element either basis had divides one of them,
//!   and every element the merge made divides one of those.

use crate::types::exact::algebraic::{add_term, Algebraic, AlgebraicResult};
use crate::types::exact::basis::Basis;
use crate::types::fraction::Fraction;
use num_bigint::BigInt;
use num_traits::{One, Zero};
use std::collections::BTreeMap;

impl Algebraic {
    /// `self + other`.
    pub fn add(&self, other: &Algebraic) -> AlgebraicResult {
        let basis = Basis::merged(self.basis(), other.basis(), &[]);
        let mut terms = self.terms_over(&basis);
        let mut cancelled = false;
        for (m, c) in other.terms_over(&basis) {
            cancelled |= add_term(&mut terms, m, c);
        }
        if cancelled {
            return AlgebraicResult::from_parts(basis, terms);
        }
        // With every monomial intact, the irrational one each operand has is
        // still there, so the sum neither prunes nor demotes.
        AlgebraicResult::Irrational(Algebraic::from_normal_parts(basis, terms))
    }

    /// `self - other`.
    pub fn sub(&self, other: &Algebraic) -> AlgebraicResult {
        self.add(&other.neg())
    }

    /// `self`'s terms over `target`, a refinement of `self`'s basis.
    ///
    /// A monomial is a product of elements of `self`'s basis. The part of it
    /// made of elements `target` kept is a monomial over `target` as it is;
    /// only the part made of elements `target` split changes, and it is a
    /// product of the elements the split made, so only it is decomposed, over
    /// those alone. Splitting an element every monomial shares then costs a
    /// few divisions per term, not a pass over the whole basis for each.
    fn terms_over(&self, target: &Basis) -> BTreeMap<BigInt, Fraction> {
        let split: Vec<&BigInt> = self
            .basis()
            .elements()
            .iter()
            .filter(|e| !target.contains(e))
            .collect();
        if split.is_empty() {
            return self.terms().clone();
        }
        let made = target.without(self.basis());
        let mut out = BTreeMap::new();
        for (m, c) in self.terms() {
            let changed: BigInt = split
                .iter()
                .filter(|e| (m % **e).is_zero())
                .fold(BigInt::one(), |product, e| product * *e);
            if changed.is_one() {
                add_term(&mut out, m.clone(), c.clone());
                continue;
            }
            let (outside, monomial) = made
                .decompose_sqrt(&changed)
                .expect("a split element is a product of the elements it split into");
            add_term(
                &mut out,
                (m / &changed) * monomial,
                c.mul(&Fraction::new(outside, BigInt::one())),
            );
        }
        out
    }
}

#[cfg(test)]
#[path = "algebraic_sum_tests.rs"]
mod tests;
