//! The reciprocal division's setup, proved by bounded model checking
//! (AQ-VER-001-U, docs/quality/TRACEABILITY_MATRIX.md).
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take. Every divisor the walk
//! (and `fraction_word_term`'s running sum) divides by goes through
//! [`reciprocal`] once, and [`div_two_by_one`] is exact only when what it is
//! handed is the reciprocal Möller and Granlund's algorithm 4 assumes. That
//! is proved here, for every normalized machine word.
//!
//! The division step itself is not: its correctness is an identity between
//! 64-bit products, and neither the SAT solver nor Z3 decides it over every
//! word within an hour. `small_divisor_tests` holds the whole walk, and so
//! every step, equal to `num-bigint`'s operators on sampled operands.

use super::reciprocal;

/// `v` is the reciprocal of a normalized `d` as Möller and Granlund define
/// it, `v = ⌊(2¹²⁸ − 1) / d⌋ − 2⁶⁴`, stated by its bounds rather than by a
/// division so it is a second statement of what [`reciprocal`] computes:
/// `V = 2⁶⁴ + v` is the largest multiplier with `V · d ≤ 2¹²⁸ − 1`.
fn is_reciprocal(d: u64, v: u64) -> bool {
    // V · d = 2⁶⁴·d + v·d, carried in u128 with overflow standing for "past
    // 2¹²⁸ − 1".
    let product = (u128::from(d) << 64).checked_add(u128::from(v) * u128::from(d));
    let next = product.and_then(|p| p.checked_add(u128::from(d)));
    product.is_some() && next.is_none()
}

/// For every divisor with its top bit set, the reciprocal's quotient fits a
/// word — so the `as u64` drops nothing — and it is the reciprocal.
#[kani::proof]
fn reciprocal_is_moller_granlund() {
    let d: u64 = kani::any();
    kani::assume(d >> 63 == 1);
    let wide = (u128::MAX - (u128::from(d) << 64)) / u128::from(d);
    assert!(wide <= u128::from(u64::MAX));
    assert!(is_reciprocal(d, reciprocal(d)));
}
