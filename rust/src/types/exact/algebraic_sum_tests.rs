//! The sum of `algebraic_sum.rs` answers what the sum it replaced answers.
//!
//! The reference below is the sum as it was before: refine the union of both
//! bases from scratch, rebase both operands' terms over it, and prune.
//! Sums of random signed multiples of √(p/q), folded one term at a time in
//! both orders, must reach the same normal form by both routes — cancellation
//! included, which is where the sum hands back to `from_parts` — and every
//! irrational it builds must keep the basis invariants the reference's
//! pruning establishes.

use crate::test_support::frac;
use crate::types::exact::algebraic::{add_term, Algebraic, AlgebraicResult};
use crate::types::exact::basis::Basis;
use crate::types::fraction::Fraction;
use num_bigint::BigInt;
use num_traits::{One, Zero};
use proptest::prelude::*;

fn reference_add(a: &Algebraic, b: &Algebraic) -> AlgebraicResult {
    let inputs = a
        .basis()
        .elements()
        .iter()
        .chain(b.basis().elements())
        .cloned()
        .collect();
    let basis = Basis::build(inputs);
    let mut terms = a.rebased_terms(&basis);
    for (m, c) in b.rebased_terms(&basis) {
        add_term(&mut terms, m, c);
    }
    AlgebraicResult::from_parts(basis, terms)
}

/// Pairwise coprime, and every element divides a monomial that decomposes
/// over the basis into itself: the invariants pruning establishes.
fn assert_basis_invariants(a: &Algebraic) {
    let elems = a.basis().elements();
    for (i, x) in elems.iter().enumerate() {
        for y in &elems[i + 1..] {
            assert!(
                num_integer::gcd(x.clone(), y.clone()).is_one(),
                "{x} and {y} share a factor"
            );
        }
        assert!(
            a.terms().keys().any(|m| (m % x).is_zero()),
            "basis element {x} divides no monomial of {a:?}"
        );
    }
    for m in a.terms().keys() {
        let (outside, monomial) = a.basis().decompose_sqrt(m).expect("monomial over basis");
        assert!(
            outside.is_one() && &monomial == m,
            "{m} is not a subset product"
        );
    }
}

/// `x + y` over the result type, with `irr` the irrational-irrational sum.
fn sum(
    x: &AlgebraicResult,
    y: &AlgebraicResult,
    irr: fn(&Algebraic, &Algebraic) -> AlgebraicResult,
) -> AlgebraicResult {
    match (x, y) {
        (AlgebraicResult::Rational(p), AlgebraicResult::Rational(q)) => {
            AlgebraicResult::Rational(p.add(q))
        }
        (AlgebraicResult::Rational(q), AlgebraicResult::Irrational(a))
        | (AlgebraicResult::Irrational(a), AlgebraicResult::Rational(q)) => a.add_fraction(q),
        (AlgebraicResult::Irrational(a), AlgebraicResult::Irrational(b)) => irr(a, b),
    }
}

fn term((coefficient, num, den): (i64, i64, i64)) -> AlgebraicResult {
    let c = frac(coefficient, 1);
    match Algebraic::sqrt_of_fraction(&frac(num, den)).expect("non-negative radicand") {
        AlgebraicResult::Rational(r) => AlgebraicResult::Rational(r.mul(&c)),
        AlgebraicResult::Irrational(a) => a.mul_fraction(&c),
    }
}

/// The normal form as comparable data: `(coefficient, radicand)` pairs.
fn normal_form(x: &AlgebraicResult) -> Vec<(Fraction, BigInt)> {
    match x {
        AlgebraicResult::Rational(r) => vec![(r.clone(), BigInt::one())],
        AlgebraicResult::Irrational(a) => a.normal_form_terms(),
    }
}

fn fold(
    terms: &[AlgebraicResult],
    irr: fn(&Algebraic, &Algebraic) -> AlgebraicResult,
) -> AlgebraicResult {
    terms[1..].iter().fold(terms[0].clone(), |acc, t| {
        let next = sum(&acc, t, irr);
        if let AlgebraicResult::Irrational(a) = &next {
            assert_basis_invariants(a);
        }
        next
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn sums_reach_the_replaced_sums_normal_form(
        raw in prop::collection::vec(
            (prop_oneof![-3i64..=-1, 1i64..=3], 1i64..=40, 1i64..=3),
            1..14,
        )
    ) {
        let terms: Vec<AlgebraicResult> = raw.iter().copied().map(term).collect();
        let reference = normal_form(&fold(&terms, reference_add));
        let forward = normal_form(&fold(&terms, Algebraic::add));
        let reversed: Vec<AlgebraicResult> = terms.iter().rev().cloned().collect();
        let backward = normal_form(&fold(&reversed, Algebraic::add));
        prop_assert_eq!(&forward, &reference);
        prop_assert_eq!(&backward, &reference);
        // Sums of several terms added to one another: an addend with a new
        // radical *and* one that cancels, or one that splits a composite
        // element several monomials share, which one term at a time rarely
        // builds.
        let chunked: Vec<AlgebraicResult> =
            terms.chunks(3).map(|chunk| fold(chunk, Algebraic::add)).collect();
        prop_assert_eq!(&normal_form(&fold(&chunked, Algebraic::add)), &reference);
    }
}

#[test]
fn a_union_whose_shared_radical_cancels_prunes_it() {
    // (√2 + √3) + (−√3 + √5): the bases {2, 3} and {3, 5} merge by union, and
    // √3 cancels, so 3 must leave the basis.
    let left = fold(&[term((1, 2, 1)), term((1, 3, 1))], Algebraic::add);
    let right = fold(&[term((-1, 3, 1)), term((1, 5, 1))], Algebraic::add);
    let total = fold(&[left, right], Algebraic::add);
    let AlgebraicResult::Irrational(a) = &total else {
        panic!("√2+√5 is irrational, got {total:?}")
    };
    assert_eq!(a.basis().elements(), &[BigInt::from(2), BigInt::from(5)]);
}

#[test]
fn splitting_an_element_many_monomials_share_rebases_each_of_them() {
    // √6 + √30 + √42 keeps 6 whole in its basis ({5, 6, 7}), and all three
    // monomials contain it; adding √2 splits 6 into 2 and 3, so all three are
    // carried over to {2, 3, 5, 7}.
    let shared = fold(
        &[term((1, 6, 1)), term((1, 30, 1)), term((1, 42, 1))],
        Algebraic::add,
    );
    let total = fold(&[shared, term((1, 2, 1))], Algebraic::add);
    let AlgebraicResult::Irrational(a) = &total else {
        panic!("√6+√30+√42+√2 is irrational, got {total:?}")
    };
    assert_eq!(a.basis().elements(), &[2, 3, 5, 7].map(BigInt::from));
    let radicands: Vec<BigInt> = a.normal_form_terms().into_iter().map(|(_, m)| m).collect();
    assert_eq!(radicands, [2, 6, 30, 42].map(BigInt::from));
}

#[test]
fn a_sum_that_cancels_to_a_rational_demotes() {
    let terms: Vec<AlgebraicResult> = [
        (1, 2, 1),
        (1, 3, 1),
        (1, 5, 1),
        (-1, 3, 1),
        (-1, 2, 1),
        (-1, 5, 1),
    ]
    .into_iter()
    .map(term)
    .collect();
    let total = fold(&terms, Algebraic::add);
    assert!(
        matches!(&total, AlgebraicResult::Rational(r) if r.is_zero()),
        "√2+√3+√5−√3−√2−√5 is 0, got {total:?}"
    );
}
