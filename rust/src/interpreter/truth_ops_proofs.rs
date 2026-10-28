//! LANG.VALUES.TRUTH, proved of the lane tables by bounded model checking.
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) runs each harness over
//! every lane code a `kani::any()` can name, so a harness that passes is a
//! proof over the whole three-element domain rather than a sample of it. The
//! specification they are held to is the strong Kleene one, written here a
//! second way: the truth order F < U < T, with `AND` its meet and `NOT` the
//! involution that reverses it.

use super::{negate_code, CONJUNCTION};
use crate::types::truth_column::{FALSE, TRUE, UNKNOWN};

/// A lane code: one of the three a column holds.
fn any_code() -> u8 {
    let code: u8 = kani::any();
    kani::assume(code <= UNKNOWN);
    code
}

/// A code's place in the truth order F < U < T.
fn rank(code: u8) -> u8 {
    match code {
        FALSE => 0,
        UNKNOWN => 1,
        _ => 2,
    }
}

fn and(a: u8, b: u8) -> u8 {
    CONJUNCTION[usize::from(3 * a + b)]
}

/// `AND` is the meet of the truth order: the lesser operand.
#[kani::proof]
fn conjunction_is_the_meet() {
    let (a, b) = (any_code(), any_code());
    let meet = if rank(a) <= rank(b) { a } else { b };
    assert_eq!(and(a, b), meet);
}

/// Every entry of the table is a lane code.
#[kani::proof]
fn conjunction_stays_in_the_domain() {
    let (a, b) = (any_code(), any_code());
    assert!(and(a, b) <= UNKNOWN);
}

/// A lattice meet: commutative, associative, idempotent, TRUE its unit and
/// FALSE its zero.
#[kani::proof]
fn conjunction_is_a_bounded_semilattice() {
    let (a, b, c) = (any_code(), any_code(), any_code());
    assert_eq!(and(a, b), and(b, a));
    assert_eq!(and(and(a, b), c), and(a, and(b, c)));
    assert_eq!(and(a, a), a);
    assert_eq!(and(TRUE, a), a);
    assert_eq!(and(FALSE, a), FALSE);
}

/// `NOT` reverses the truth order, fixes UNKNOWN and undoes itself.
#[kani::proof]
fn negation_is_the_order_reversing_involution() {
    let (a, b) = (any_code(), any_code());
    assert!(negate_code(a) <= UNKNOWN);
    assert_eq!(negate_code(negate_code(a)), a);
    assert_eq!(rank(negate_code(a)), 2 - rank(a));
    assert_eq!(negate_code(UNKNOWN), UNKNOWN);
    // So the derived disjunction is the join, and De Morgan holds.
    let or = negate_code(and(negate_code(a), negate_code(b)));
    let join = if rank(a) >= rank(b) { a } else { b };
    assert_eq!(or, join);
}

/// Excluded middle fails exactly at UNKNOWN: `a ∧ ¬a` is FALSE for a known
/// truth value and UNKNOWN otherwise.
#[kani::proof]
fn contradiction_is_false_only_when_known() {
    let a = any_code();
    let expected = if a == UNKNOWN { UNKNOWN } else { FALSE };
    assert_eq!(and(a, negate_code(a)), expected);
}
