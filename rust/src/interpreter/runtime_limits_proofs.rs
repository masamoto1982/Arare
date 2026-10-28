//! LANG.MACHINE.LIMITS's work meter, proved never to undercharge, and its
//! ceilings, proved to refuse exactly past their limits, by bounded model
//! checking.
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take. The meter is a safety
//! control: it is charged before the work runs, so the property that matters
//! is that it never prices work below what it is — that a wider operand, more
//! lanes or a longer sort is never cheaper, that nothing is free, and that
//! the order a Word sees its operands in does not move the price. The
//! harnesses take every `u64` where the arithmetic is linear, and operands
//! below a bound where it multiplies two measures together (a product of two
//! free 64-bit words is beyond the solver).

use super::WORK_LIMB_BITS;
use super::{
    binary_numeric_work, broadcast_numeric_work, work_limbs, ElementCost, OperandWork,
    ResourceLimit, RuntimeLimits,
};
use crate::error::AjisaiError;

fn any_work() -> OperandWork {
    OperandWork {
        lanes: kani::any(),
        bits: kani::any(),
        terms: kani::any(),
        basis: kani::any(),
    }
}

/// The largest lane count a bounded measure takes.
const LANES: u64 = 1 << 8;

/// The widest number a bounded measure takes, in bits: sixteen limbs.
const BITS: u64 = 1 << 10;

/// What a measure can be, bounded so the products the meter forms stay in
/// the solver's reach.
fn bounded_work() -> OperandWork {
    let w = any_work();
    kani::assume(w.lanes <= LANES && w.bits <= BITS && w.terms <= 1 << 4);
    w
}

/// The limbs charged cover every bit, there is always at least one, and a
/// wider number is never charged fewer.
#[kani::proof]
fn work_limbs_cover_every_bit() {
    let (bits, more): (u64, u64) = (kani::any(), kani::any());
    let limbs = work_limbs(bits);
    assert!(limbs >= 1);
    assert!(u128::from(limbs) * u128::from(WORK_LIMB_BITS) >= u128::from(bits));
    // And not a limb more than the bits need.
    assert!(limbs == 1 || (limbs - 1) * WORK_LIMB_BITS < bits);
    if bits <= more {
        assert!(work_limbs(bits) <= work_limbs(more));
    }
}

/// Joining sibling measures is a commutative monoid with the empty measure as
/// its unit, so how a Word walks an operand's leaves cannot change its price.
#[kani::proof]
fn join_is_a_commutative_monoid() {
    let (a, b, c) = (any_work(), any_work(), any_work());
    let empty = OperandWork {
        lanes: 0,
        bits: 0,
        terms: 0,
        basis: 0,
    };
    assert_eq!(a.join(b), b.join(a));
    assert_eq!(a.join(b).join(c), a.join(b.join(c)));
    assert_eq!(a.join(empty), a);
}

/// Swapping the operands of a binary operation does not change its price.
/// Bounded like the harness below: commutativity of a product of two free
/// 64-bit words is beyond the solver.
#[kani::proof]
fn binary_work_is_symmetric() {
    let (a, b) = (bounded_work(), bounded_work());
    let units: u64 = kani::any();
    kani::assume(units <= 1 << 6);
    assert_eq!(
        binary_numeric_work(a.bits, b.bits),
        binary_numeric_work(b.bits, a.bits)
    );
    assert_eq!(
        broadcast_numeric_work(a, b, units),
        broadcast_numeric_work(b, a, units)
    );
}

/// A wider operand or more lanes is never cheaper, and no operation is free.
///
/// Comparing two prices is comparing two products, which the solver decides
/// only over a narrower scope than the harnesses above: up to 16 lanes, four
/// limbs a side and 8 units a pair.
#[kani::proof]
fn broadcast_work_is_monotone_and_never_free() {
    let narrow = |w: OperandWork| w.lanes <= 16 && w.bits <= 4 * WORK_LIMB_BITS;
    let (a, b) = (bounded_work(), bounded_work());
    kani::assume(narrow(a) && narrow(b));
    let units: u64 = kani::any();
    kani::assume((1..=8).contains(&units));
    let price = broadcast_numeric_work(a, b, units);
    assert!(price >= 1);
    let wider = OperandWork {
        bits: kani::any(),
        lanes: kani::any(),
        ..a
    };
    kani::assume(narrow(wider) && a.bits <= wider.bits && a.lanes <= wider.lanes);
    assert!(broadcast_numeric_work(wider, b, units) >= price);
}

/// An element is never free to look at or to copy, and a copy costs at least
/// a look.
#[kani::proof]
fn element_cost_is_never_free() {
    let work = bounded_work();
    let count: usize = kani::any();
    kani::assume(count <= 1 << 8);
    let cost = ElementCost::measure(work, count);
    assert!(cost.leaves >= 1 && cost.width >= 1);
    assert!(cost.probe() >= 1);
    assert!(cost.copy() >= cost.probe());
}

/// A comparison sort is priced at no less than the worst case's `n⌈log₂n⌉`
/// comparisons, for every count.
#[kani::proof]
fn comparison_sort_covers_the_worst_case() {
    let count: usize = kani::any();
    let n = count as u64;
    // ⌈log₂ n⌉, as the bit length of n − 1.
    let ceil_log2 = if n <= 1 {
        0
    } else {
        u64::from(64 - (n - 1).leading_zeros())
    };
    let one = ElementCost {
        leaves: 1,
        width: 1,
    };
    assert!(one.comparison_sort(count) >= n.saturating_mul(ceil_log2));
}

fn any_limits() -> RuntimeLimits {
    RuntimeLimits {
        max_materialized_elements: kani::any(),
        max_source_bytes: kani::any(),
        max_numeric_literal_digits: kani::any(),
        max_numeric_work: kani::any(),
        max_collection_work: kani::any(),
        max_bigint_bits: kani::any(),
        max_algebraic_terms: kani::any(),
        max_nesting_depth: kani::any(),
    }
}

/// The refusal `check` answers: the resource named, its limit, and what was
/// observed.
fn refusal(check: crate::error::Result<()>) -> Option<(ResourceLimit, u64, Option<u64>)> {
    match check {
        Ok(()) => None,
        Err(AjisaiError::ResourceLimitExceeded {
            resource,
            limit,
            observed,
            progress: None,
        }) => Some((resource, limit, observed)),
        Err(_) => panic!("a ceiling refuses with resourceLimitExceeded"),
    }
}

/// Every ceiling allows its limit itself and refuses one past it, naming its
/// own resource, its limit and the size observed; and an algebraic result
/// is held to its term count before its width.
#[kani::proof]
fn every_ceiling_refuses_exactly_past_its_limit() {
    let limits = any_limits();
    let (n, bits): (usize, u64) = (kani::any(), kani::any());
    let past = |observed: usize, limit: usize, resource| {
        (observed > limit).then_some((resource, limit as u64, Some(observed as u64)))
    };
    assert_eq!(
        refusal(limits.check_source_bytes(n)),
        past(n, limits.max_source_bytes, ResourceLimit::SourceBytes)
    );
    assert_eq!(
        refusal(limits.check_nesting_depth(n)),
        past(n, limits.max_nesting_depth, ResourceLimit::NestingDepth)
    );
    assert_eq!(
        refusal(limits.check_numeric_literal_digits(n)),
        past(
            n,
            limits.max_numeric_literal_digits,
            ResourceLimit::NumericLiteralDigits
        )
    );
    let too_wide = (bits > limits.max_bigint_bits).then_some((
        ResourceLimit::BigintBits,
        limits.max_bigint_bits,
        Some(bits),
    ));
    assert_eq!(refusal(limits.check_bigint_bits(bits)), too_wide);
    assert_eq!(
        refusal(limits.check_algebraic_size(n, bits)),
        past(n, limits.max_algebraic_terms, ResourceLimit::AlgebraicTerms).or(too_wide)
    );
}
