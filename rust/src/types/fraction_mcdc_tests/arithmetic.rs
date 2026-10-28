//! MC/DC tables for the decisions of `crate::types::fraction_arithmetic` that
//! `fraction_mcdc_tests` does not cover. That suite tables `add`'s i64 fast
//! path (AQ-VER-001-D, -H, -I) and `floor`'s `Small` arm (AQ-VER-001-E); this
//! one covers the machine-integer fast path ahead of them, `sub`, the `BigInt`
//! arms of all four operations, Henrici's reduction, the guards of `div`,
//! `floor`'s remainder condition, `round`, and the widening of `neg`/`abs`.
//!
//! A child of `fraction_mcdc_tests` rather than appended to it: that file is
//! grandfathered near the 500-line budget
//! (docs/dev/specification-implementation-rules.md).
//!
//! Trace: docs/quality/TRACEABILITY_MATRIX.md, requirement AQ-REQ-001.

use crate::types::fraction::Fraction;
use num_bigint::BigInt;

fn small(n: i64, d: i64) -> Fraction {
    Fraction::new(BigInt::from(n), BigInt::from(d))
}

fn big(n: BigInt, d: i64) -> Fraction {
    Fraction::new(n, BigInt::from(d))
}

/// 2^70: an integer past a machine word, `≡ 1 (mod 3)` and odd plus one.
fn w() -> BigInt {
    BigInt::from(1) << 70
}

fn parse(text: &str) -> Fraction {
    Fraction::from_str(text).unwrap()
}

// ---------------------------------------------------------------------------
// AQ-VER-001-O
// DUT: rust/src/types/fraction_arithmetic.rs `add`, `sub`, `mul`: the
// machine-integer fast path, and `sub`'s copies of AQ-VER-001-D's decisions.
//
//     if let (Small(a, 1), Small(c, 1)) = (..) {          // A, B
//         if let Some(n) = a.checked_add(*c) { .. }        // C (checked_sub / checked_mul)
//     }
//     if b == 1 && d == 1 { .. }   if b == d { .. }        // sub: as AQ-VER-001-D
//
// A && B (both integers):
//   row 1: (T, T), C = T -> integer result, no gcd
//   row 2: (F, T)        -> i64 pair path            pair (1,2) shows A
//   row 3: (T, F)        -> i64 pair path            pair (1,3) shows B
// C (no overflow), reached with A && B:
//   row 4: C = F -> the i64 pair path widens to Big   pair (1,4) shows C
// ---------------------------------------------------------------------------
mod machine_integer_fast_path {
    use super::*;

    #[test]
    fn aq_ver_001_o_rows_1_to_3_add_and_sub() {
        assert_eq!(small(7, 1).add(&small(5, 1)), small(12, 1));
        assert_eq!(small(7, 1).sub(&small(5, 1)), small(2, 1));
        assert_eq!(small(1, 2).add(&small(5, 1)), small(11, 2));
        assert_eq!(small(1, 2).sub(&small(5, 1)), small(-9, 2));
        assert_eq!(small(5, 1).add(&small(1, 2)), small(11, 2));
        assert_eq!(small(5, 1).sub(&small(1, 2)), small(9, 2));
    }

    #[test]
    fn aq_ver_001_o_row4_an_overflowing_integer_sum_widens() {
        let over = small(i64::MAX, 1).add(&small(1, 1));
        assert!(!over.is_small());
        assert_eq!(over, big(BigInt::from(i64::MAX) + 1, 1));
        let under = small(i64::MIN, 1).sub(&small(1, 1));
        assert!(!under.is_small());
        assert_eq!(under, big(BigInt::from(i64::MIN) - 1, 1));
    }

    #[test]
    fn aq_ver_001_o_an_overflowing_integer_product_widens() {
        assert_eq!(small(1 << 31, 1).mul(&small(3, 1)), small(3 << 31, 1));
        let product = small(1 << 62, 1).mul(&small(4, 1));
        assert!(!product.is_small());
        assert_eq!(product, big(BigInt::from(1) << 64, 1));
        // The i64 pair path, past a word: (2^62/3)·(5/7) = 5·2^62/21.
        let wide = small(1 << 62, 3).mul(&small(5, 7));
        assert!(!wide.is_small());
        assert_eq!(wide, big(BigInt::from(5) << 62, 21));
    }

    #[test]
    fn aq_ver_001_o_sub_same_denominator_and_cross_paths() {
        // As AQ-VER-001-D rows 4 and 5, for `sub`.
        assert_eq!(small(3, 5).sub(&small(1, 5)), small(2, 5));
        assert_eq!(small(1, 3).sub(&small(1, 4)), small(1, 12));
        assert_eq!(small(1, 5).sub(&small(1, 5)), small(0, 1));
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-P
// DUT: rust/src/types/fraction_arithmetic.rs `add`/`sub` past the i64 pair
// path, and `add_reduced_bigint` (Henrici):
//
//     if ad == bd {                                  // S
//         if sum.is_zero() { return 0 }              // Z
//         if g.is_one() { return sum/ad }            // G   g = gcd(sum, ad)
//         return (sum/g)/(ad/g)
//     }
//     add_reduced_bigint:
//         if g.is_one() { .. }                       // H   g = gcd(ad, bd)
//         if g2.is_one() { .. }                      // H2  g2 = gcd(t, g)
//
// One operand is past a word, so the i64 pair path declines both rows of each
// pair. W = 2^70, W ≡ 1 (mod 3), and W + 1 is coprime to 6.
//   S = T: Z = T (W+1)/3 - (W+1)/3;  Z = F, G = T  (W+1)/3 + 2/3;
//          Z = F, G = F  (W+1)/3 + 1/3 = (W+2)/3, an integer
//   S = F: H = T (W+1)/3 + 1/7;  H = F, H2 = T  (W+1)/6 + 1/4;
//          H = F, H2 = F  (W+1)/6 + 1/10
//   A zero `t` has no row: values that cancel have equal reduced
//   denominators, so S answers first.
// ---------------------------------------------------------------------------
mod bigint_sum_reduction {
    use super::*;

    #[test]
    fn aq_ver_001_p_shared_denominator_rows() {
        let a = big(w() + 1, 3);
        assert!(!a.is_small());
        assert_eq!(a.sub(&a), small(0, 1), "Z = T");
        assert_eq!(a.add(&a.neg()), small(0, 1), "Z = T");
        assert_eq!(a.add(&small(2, 3)), big(w() + 3, 3), "G = T");
        assert_eq!(a.add(&small(1, 3)), big((w() + 2) / 3, 1), "G = F");
        assert_eq!(a.sub(&small(-1, 3)), big((w() + 2) / 3, 1), "G = F");
        assert!(a.add(&small(1, 3)).is_integer());
    }

    #[test]
    fn aq_ver_001_p_henrici_coprime_denominators() {
        let a = big(w() + 1, 3);
        assert_eq!(a.add(&small(1, 7)), big((w() + 1) * 7 + 3, 21), "H = T");
        assert_eq!(a.sub(&small(1, 7)), big((w() + 1) * 7 - 3, 21), "H = T");
    }

    #[test]
    fn aq_ver_001_p_henrici_shared_factor() {
        let a = big(w() + 1, 6);
        assert!(!a.is_small());
        assert_eq!(a.add(&small(1, 4)), big(w() * 2 + 5, 12), "H2 = T");
        // t = 5(W+1) + 3 is even, so g2 = 2 divides out.
        assert_eq!(a.add(&small(1, 10)), big((w() * 5 + 8) / 2, 15), "H2 = F");
        assert_eq!(a.sub(&small(1, 10)), big((w() * 5 + 2) / 2, 15), "H2 = F");
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-Q
// DUT: rust/src/types/fraction_arithmetic.rs `mul` and `div` past the i64
// pair path, and `div`'s guards:
//
//     if ad.is_one() && bd.is_one() { .. }           // A && B
//     if ad.is_one() { .. }                          // A
//     if bd.is_one() { .. }                          // B
//     ... both cross-cancelled ...
//
//   div: if !other.is_finite() { .. }                       // F
//        if !self.is_finite() || other.is_zero() { .. }      // P || Z
//
// A && B:  (T, T) row 1;  (T, F) row 2 [B];  (F, T) row 3 [A];  (F, F) row 4
// P || Z, reached with F = F:
//   (F, F) 3/2 ÷ 3 -> rational;  (T, F) 1/0 ÷ 2 -> 1/0 [P];  (F, T) 3 ÷ 0 -> 1/0 [Z]
// ---------------------------------------------------------------------------
mod bigint_product_cascade {
    use super::*;

    #[test]
    fn aq_ver_001_q_mul_rows() {
        let wide = big(w(), 1);
        assert_eq!(wide.mul(&big(w() + 1, 1)), big(w() * (w() + 1), 1), "row 1");
        // 2^70 · 1/6 = 2^69/3.
        assert_eq!(wide.mul(&small(1, 6)), big(w() / 2, 3), "row 2");
        let a = big(w() + 1, 3);
        assert_eq!(a.mul(&small(3, 1)), big(w() + 1, 1), "row 3");
        assert_eq!(small(1, 6).mul(&wide), big(w() / 2, 3), "row 2 mirrored");
        assert_eq!(
            a.mul(&big(BigInt::from(3), 1).div(&big(w() + 1, 1))),
            small(1, 1),
            "row 4"
        );
    }

    #[test]
    fn aq_ver_001_q_div_rows() {
        let wide = big(w(), 1);
        assert_eq!(
            wide.div(&big(BigInt::from(1) << 69, 1)),
            small(2, 1),
            "row 1"
        );
        assert_eq!(wide.div(&small(5, 6)), big(w() * 6, 5), "row 2");
        let a = big(w() + 1, 3);
        assert_eq!(a.div(&small(2, 1)), big(w() + 1, 6), "row 3");
        assert_eq!(a.div(&big(w() + 1, 6)), small(2, 1), "row 4");
    }

    #[test]
    fn aq_ver_001_q_div_guards() {
        // F = T: the reciprocal of a point over zero.
        assert_eq!(small(3, 1).div(&Fraction::infinity()), small(0, 1));
        assert_eq!(small(3, 1).div(&Fraction::nullity()), Fraction::nullity());
        // P || Z.
        assert_eq!(small(3, 2).div(&small(3, 1)), small(1, 2), "(F, F)");
        assert_eq!(
            Fraction::infinity().div(&small(2, 1)),
            Fraction::infinity(),
            "(T, F)"
        );
        assert_eq!(
            small(3, 1).div(&small(0, 1)),
            Fraction::infinity(),
            "(F, T)"
        );
        assert_eq!(
            small(-3, 1).div(&small(0, 1)),
            Fraction::infinity(),
            "(F, T)"
        );
        assert_eq!(small(0, 1).div(&small(0, 1)), Fraction::nullity(), "(F, T)");
    }

    #[test]
    fn aq_ver_001_q_mul_entry_guard() {
        // `!self.is_finite() || !other.is_finite()`, one side at a time.
        assert_eq!(Fraction::infinity().mul(&small(2, 1)), Fraction::infinity());
        assert_eq!(
            small(-2, 1).mul(&Fraction::infinity()),
            Fraction::infinity()
        );
        assert_eq!(small(0, 1).mul(&Fraction::infinity()), Fraction::nullity());
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-R
// DUT: rust/src/types/fraction_arithmetic.rs `floor`, `round`, `neg`, `abs`
//
// floor rounds toward negative infinity on both arms: -7/3 -> -3, 7/3 -> 2.
// round: half away from zero, the sign (S = n < 0) deciding the direction:
//   S = F 5/2 -> 3, 7/3 -> 2;  S = T -5/2 -> -3, -7/3 -> -2; and on `Big`.
//   `if self.is_zero()` has no true row: zero is an integer, which returns
//   first.
// neg / abs, `checked_neg` / `checked_abs` Some or None, and `abs`'s Big sign.
// ---------------------------------------------------------------------------
mod rounding_and_sign {
    use super::*;

    #[test]
    fn aq_ver_001_r_floor_with_and_without_a_remainder() {
        assert_eq!(small(-7, 3).floor(), small(-3, 1));
        assert_eq!(small(7, 3).floor(), small(2, 1));
    }

    #[test]
    fn aq_ver_001_r_floor_big_with_and_without_a_remainder() {
        assert_eq!(big(-(w() + 1i32), 3).floor(), big(-(w() + 1i32) / 3 - 1, 1));
        assert_eq!(big(w() + 1i32, 3).floor(), big((w() + 1i32) / 3, 1));
    }

    #[test]
    fn aq_ver_001_r_round_half_away_from_zero() {
        assert_eq!(small(5, 2).round(), small(3, 1));
        assert_eq!(small(7, 3).round(), small(2, 1));
        assert_eq!(small(-5, 2).round(), small(-3, 1));
        assert_eq!(small(-7, 3).round(), small(-2, 1));
        assert_eq!(small(0, 1).round(), small(0, 1));
        // (W+1)/2 = 2^69 + 1/2 rounds away from zero.
        assert_eq!(big(w() + 1, 2).round(), big(w() / 2 + 1, 1));
        assert_eq!(big(-(w() + 1i32), 2).round(), big(-(w() / 2i32 + 1i32), 1));
        // W + 1 = 3k + 2, so (W+1)/3 = k + 2/3 rounds up to k + 1.
        assert_eq!(big(w() + 1, 3).round(), big((w() + 1) / 3 + 1, 1));
        // W + 3 = 3(k + 1) + 1 rounds down.
        assert_eq!(big(w() + 3, 3).round(), big(w() / 3 + 1, 1));
    }

    #[test]
    fn aq_ver_001_r_neg_and_abs_widen_only_i64_min() {
        assert_eq!(small(3, 4).neg(), small(-3, 4));
        assert!(small(3, 4).neg().is_small());
        let widened = small(i64::MIN, 1).neg();
        assert!(!widened.is_small());
        assert_eq!(widened, big(-BigInt::from(i64::MIN), 1));
        assert_eq!(big(w(), 1).neg(), big(-w(), 1));
        assert_eq!(big(-w(), 3).abs(), big(w(), 3));
        assert_eq!(big(w(), 3).abs(), big(w(), 3));
        assert_eq!(parse("-1/0").abs(), Fraction::infinity());
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-S
// DUT: rust/src/types/fraction_arithmetic.rs `balanced_bigint_gcd`
//
//     if a.is_zero() { .. }                                     // A0
//     if b.is_zero() { .. }                                     // B0
//     if let Some(g) = gcd_with_word(wide, narrow) { .. }       // W  narrow one word
//     if wide.bits() <= 128 { .. }                              // U  both two words
//     if narrow.bits() > LEHMER_BITS { lehmer }                 // L
//     narrow.gcd(&(wide % narrow))                              // the rest
//
// Each row answers through a different arm, held to `Integer::gcd`:
//   A0 (0, x);  B0 (x, 0);  W (wide, one word);  U (two words, two words);
//   L (wide, past two words);  rest (past two words, exactly two words).
// The property tests reach these arms at random; these rows name each one.
// Inside U, `(Some(x), Some(y))` from `to_u128` has no false row: a
// magnitude of at most 128 bits always converts.
// ---------------------------------------------------------------------------
mod balanced_gcd_dispatch {
    use crate::types::fraction_arithmetic::balanced_bigint_gcd;
    use num_bigint::BigInt;
    use num_integer::Integer;

    fn p2(k: u32) -> BigInt {
        BigInt::from(1) << k
    }

    fn check(a: BigInt, b: BigInt, expected: BigInt) {
        assert_eq!(balanced_bigint_gcd(&a, &b), expected, "gcd({a}, {b})");
        assert_eq!(balanced_bigint_gcd(&b, &a), expected, "gcd({b}, {a})");
        assert_eq!(a.gcd(&b), expected, "oracle");
    }

    #[test]
    fn aq_ver_001_s_each_arm_answers_the_gcd() {
        check(BigInt::from(0), p2(200) * -3, p2(200) * 3);
        check(p2(200) * 3 + 3, BigInt::from(6), BigInt::from(3));
        check(p2(100) * 3, p2(90) * 5, p2(90));
        check(p2(300) * 3, p2(150) * 5, p2(150));
        check(p2(300) * 3, p2(100) * 5, p2(100));
        check(p2(300) * 3 + 1, p2(100) * 5, BigInt::from(1));
        check(p2(300) * -7, p2(127) * 7, p2(127) * 7);
    }
}
