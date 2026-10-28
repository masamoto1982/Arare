//! AR-VER-206: MC/DC tables for the kernel's compound decisions. Each table
//! shows every condition flipping its decision's outcome while the others
//! hold still (docs/quality/VERIFICATION_PLAN.md, QL-A).

use crate::codes::{fit, Val};
use crate::column::and_word;
use crate::num::{rescale_code, Num};
use crate::width::Width;

/// `fit`: `−max ≤ x` (A) `∧ x ≤ max` (B) decides Fin over 1/0.
#[test]
fn ar_ver_206_a_fit() {
    let w = Width::W8;
    // (A, B) → outcome
    assert_eq!(fit(w, 5), Val::Fin(5)); //      (T, T) → Fin
    assert_eq!(fit(w, -127), Val::Inf); //      (F, T) → 1/0: A alone flips it
    assert_eq!(fit(w, 127), Val::Inf); //       (T, F) → 1/0: B alone flips it
}

/// `rescale_code`, s = 2 (half = 2): keep `q` when `r < half` (A)
/// `∨ (r = half` (B) `∧ q even` (C)`)`, otherwise `q + 1`.
#[test]
fn ar_ver_206_b_rescale() {
    // k     r  q   (A, B, C)  → result
    assert_eq!(rescale_code(1, 2), 0); //  1 0  (T, F, T) → q      0.25 → 0
    assert_eq!(rescale_code(3, 2), 1); //  3 0  (F, F, T) → q + 1  0.75 → 1: A flips
    assert_eq!(rescale_code(2, 2), 0); //  2 0  (F, T, T) → q      0.5  → 0
                                       //                                      against (F, F, T) above: B flips
    assert_eq!(rescale_code(6, 2), 2); //  2 1  (F, T, F) → q + 1  1.5  → 2: C flips
                                       // The floor shift on negatives: −1.5 → −2, −0.5 → 0, −0.25 → 0.
    assert_eq!(rescale_code(-6, 2), -2);
    assert_eq!(rescale_code(-2, 2), 0);
    assert_eq!(rescale_code(-1, 2), 0);
    assert_eq!(rescale_code(7, 0), 7);
    assert_eq!(rescale_code(i64::MAX - 1, 64), 0);
}

/// DIV's rounding on the truncating quotient: `2|r| < d` (D1) keeps `q`;
/// otherwise `2|r| > d` (X) `∨ q odd` (Y) moves away from zero.
#[test]
fn ar_ver_206_c_div_rounding() {
    let div = |a: i64, b: i64| match num(a).div(num(b)).val() {
        Val::Fin(k) => k,
        other => panic!("{a}/{b} gave {other:?}"),
    };
    // n d      (D1, X, Y) → result
    assert_eq!(div(1, 4), 0); //   (T, F, F) → q        0.25 → 0
    assert_eq!(div(3, 4), 1); //   (F, T, F) → away     0.75 → 1: D1 flips
    assert_eq!(div(2, 4), 0); //   (F, F, F) → q        0.5  → 0: X flips
    assert_eq!(div(6, 4), 2); //   (F, F, T) → away     1.5  → 2: Y flips
    assert_eq!(div(-6, 4), -2); // away from zero on a negative quotient
    assert_eq!(div(-2, 4), 0);
    assert_eq!(div(6, -4), -2); // a negative divisor reduces to a positive one
}

/// MUL with 1/0: zero (A) decides 0/0 over 1/0, from either side.
#[test]
fn ar_ver_206_d_mul_inf() {
    let inf = Num::new(Val::Inf, Width::W8, 0).unwrap();
    assert_eq!(inf.mul(num(0)).unwrap().val(), Val::Bot); // A = T
    assert_eq!(inf.mul(num(3)).unwrap().val(), Val::Inf); // A = F
    assert_eq!(num(0).mul(inf).unwrap().val(), Val::Bot);
    assert_eq!(num(-3).mul(inf).unwrap().val(), Val::Inf);
}

/// Kleene AND on one lane: known when both are known (A), or the left is a
/// known FALSE (B), or the right is (C). Lane bits are (valid, value).
#[test]
fn ar_ver_206_e_and_lane() {
    let lane = |va: u64, xa: u64, vb: u64, xb: u64| and_word(va, xa, vb, xb);
    // (A, B, C) → (valid, value)
    assert_eq!(lane(1, 1, 1, 1), (1, 1)); // (T, F, F): TRUE ∧ TRUE = TRUE
    assert_eq!(lane(0, 0, 1, 1), (0, 0)); // (F, F, F): UNKNOWN ∧ TRUE = UNKNOWN: A flips
    assert_eq!(lane(1, 0, 0, 0), (1, 0)); // (F, T, F): FALSE ∧ UNKNOWN = FALSE: B flips
    assert_eq!(lane(0, 0, 1, 0), (1, 0)); // (F, F, T): UNKNOWN ∧ FALSE = FALSE: C flips
}

fn num(k: i64) -> Num {
    Num::new(Val::Fin(k), Width::W8, 0).unwrap()
}
