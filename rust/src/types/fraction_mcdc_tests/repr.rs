//! MC/DC tables for the decisions of `crate::types::fraction` (and
//! `fraction_order`) that `fraction_mcdc_tests` does not cover: the 128-bit
//! gcd dispatch, the machine-word normalizer, the integer reads, the exponent
//! guard of `from_str`, and the mixed-representation arms of `order` and `==`.
//!
//! A child of `fraction_mcdc_tests` rather than appended to it: that file is
//! grandfathered near the 500-line budget
//! (docs/dev/specification-implementation-rules.md).
//!
//! Trace: docs/quality/TRACEABILITY_MATRIX.md, requirement AQ-REQ-001.

use crate::types::fraction::{binary_gcd_u128, Fraction};
use num_bigint::BigInt;
use std::cmp::Ordering;

fn small(n: i64, d: i64) -> Fraction {
    Fraction::new(BigInt::from(n), BigInt::from(d))
}

fn big(n: BigInt, d: i64) -> Fraction {
    Fraction::new(n, BigInt::from(d))
}

/// 2^k as a `BigInt`.
fn pow2(k: u32) -> BigInt {
    BigInt::from(1) << k
}

// ---------------------------------------------------------------------------
// AQ-VER-001-J
// DUT: rust/src/types/fraction.rs `binary_gcd_u128`
//
//     if let (Ok(x), Ok(y)) = (u64::try_from(a), u64::try_from(b))   // D1
//     if a > b && b != 0 && u64::try_from(b).is_ok()                   // D2
//     if b > a && a != 0 && u64::try_from(a).is_ok()                   // D3
//     if a == 0 { return b; }                                          // D4
//     if b == 0 { return a; }                                          // D5
//     ... Stein's loop on two wide operands ...                        // tail
//
// Conditions: A1 = a fits u64, B1 = b fits u64 (D1); P = a > b, Q = b != 0,
// R = b fits u64 (D2); P' = b > a, Q' = a != 0, R' = a fits u64 (D3).
//
// Which arm runs is not observable — every arm answers the gcd — so each row
// asserts the gcd against the value it must be. The rows are chosen so every
// arm is the one that answers.
//
// D1 (A1 && B1):
//   row 1: (T, T) -> 64-bit form
//   row 2: (F, T) -> not taken (D2 answers)      pair (1,2) shows A1
//   row 3: (T, F) -> not taken (D3 answers)      pair (1,3) shows B1
// D2 (P && Q && R), reached with D1 = F:
//   row 2: (T, T, T) -> one division, then the 64-bit form
//   row 4: (T, F, *) -> not taken (D5 answers)   pair (2,4) shows Q
//   row 5: (T, T, F) -> not taken (tail answers) pair (2,5) shows R
//   P = F with Q = R = T is unreachable: b fits u64 and a <= b put a in u64
//   too, so D1 had already answered. P's effect is masked by D1.
// D3 (P' && Q' && R'), the mirror of D2:
//   row 3: (T, T, T) -> one division, then the 64-bit form
//   row 6: (T, F, *) -> not taken (D4 answers)   pair (3,6) shows Q'
//   row 7: (T, T, F) -> not taken (tail answers) pair (3,7) shows R'
// ---------------------------------------------------------------------------
mod gcd_u128_dispatch {
    use super::*;

    const WIDE: u128 = 1 << 70;

    #[test]
    fn aq_ver_001_j_row1_both_in_a_word() {
        assert_eq!(binary_gcd_u128(48, 18), 6);
        assert_eq!(
            binary_gcd_u128(u64::MAX as u128, u64::MAX as u128),
            u64::MAX as u128
        );
    }

    #[test]
    fn aq_ver_001_j_row2_wide_against_a_word() {
        assert_eq!(binary_gcd_u128(3 * WIDE, 12), 12);
        assert_eq!(binary_gcd_u128(3 * WIDE + 1, 12), 1);
    }

    #[test]
    fn aq_ver_001_j_row3_a_word_against_wide() {
        assert_eq!(binary_gcd_u128(12, 3 * WIDE), 12);
        assert_eq!(binary_gcd_u128(12, 3 * WIDE + 1), 1);
    }

    #[test]
    fn aq_ver_001_j_row4_wide_against_zero() {
        assert_eq!(binary_gcd_u128(3 * WIDE, 0), 3 * WIDE);
    }

    #[test]
    fn aq_ver_001_j_row6_zero_against_wide() {
        assert_eq!(binary_gcd_u128(0, 3 * WIDE), 3 * WIDE);
    }

    #[test]
    fn aq_ver_001_j_row5_row7_both_wide_run_steins_loop() {
        // gcd(3·2^70, 5·2^68) = 2^68, from either side.
        assert_eq!(binary_gcd_u128(3 * WIDE, 5 << 68), 1 << 68);
        assert_eq!(binary_gcd_u128(5 << 68, 3 * WIDE), 1 << 68);
        assert_eq!(binary_gcd_u128(u128::MAX, u128::MAX - 1), 1);
        assert_eq!(binary_gcd_u128(u128::MAX, u128::MAX), u128::MAX);
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-K
// DUT: rust/src/types/fraction.rs `Fraction::create_from_i128`, the
// machine-word fast path before the `i128` one AQ-VER-001-G covers:
//
//     if let (Ok(n), Ok(d)) = (i64::try_from(num), i64::try_from(den)) {  // E1
//         let g = binary_gcd_u64(..);
//         if let Ok(g) = i64::try_from(g) {                                 // E2
//             let (n, d) = if d < 0 {                                       // E3
//                 (n.checked_neg(), d.checked_neg())
//             } else { (Some(n), Some(d)) };
//             if let (Some(n), Some(d)) = (n, d) { return Small(n, d) }     // E4
//         }
//     }
//
// E1 (N = num fits i64, D = den fits i64):
//   row 1: (T, T) -> fast path                       e.g. 6/4
//   row 2: (F, T) -> i128 path                       pair (1,2) shows N
//   row 3: (T, F) -> i128 path                       pair (1,3) shows D
// E2 (gcd fits i64): false only for g = 2^63, which takes both halves a
//   multiple of 2^63 within i64 — i64::MIN over i64::MIN, or 0 over i64::MIN.
//   row 4: F -> i128 path, still the right pair      pair (1,4)
// E3 (d < 0):
//   row 1: F -> pair kept as reduced
//   row 5: T -> both halves negated                  pair (1,5)
// E4 (X = -n fits i64, Y = -d fits i64), reached with E3 = T:
//   row 5: (T, T) -> Small
//   row 6: (F, T) -> i128 path: n = i64::MIN         pair (5,6) shows X
//   row 7: (T, F) -> i128 path: d = i64::MIN         pair (5,7) shows Y
// ---------------------------------------------------------------------------
mod create_from_i128_machine_word_path {
    use super::*;

    #[test]
    fn aq_ver_001_k_row1_both_halves_in_a_word_reduce_there() {
        let f = Fraction::create_from_i128(6, 4);
        assert!(f.is_small());
        assert_eq!(f.extract_i64_pair(), Some((3, 2)));
    }

    #[test]
    fn aq_ver_001_k_row2_a_wide_numerator_reduces_in_i128() {
        // 2^63 / 2 = 2^62: the numerator alone was past i64, the result is not.
        let f = Fraction::create_from_i128(i64::MAX as i128 + 1, 2);
        assert!(f.is_small());
        assert_eq!(f.extract_i64_pair(), Some((1 << 62, 1)));
    }

    #[test]
    fn aq_ver_001_k_row3_a_wide_denominator_reduces_in_i128() {
        let f = Fraction::create_from_i128(4, i64::MAX as i128 + 1);
        assert!(f.is_small());
        assert_eq!(f.extract_i64_pair(), Some((1, 1 << 61)));
        let g = Fraction::create_from_i128(1, i64::MAX as i128 + 1);
        assert!(!g.is_small());
        assert_eq!(g.to_bigint_pair(), (BigInt::from(1), pow2(63)));
    }

    #[test]
    fn aq_ver_001_k_row4_a_gcd_of_two_to_the_63_takes_the_i128_path() {
        let one = Fraction::create_from_i128(i64::MIN as i128, i64::MIN as i128);
        assert_eq!(one.extract_i64_pair(), Some((1, 1)));
        let zero = Fraction::create_from_i128(0, i64::MIN as i128);
        assert_eq!(zero.extract_i64_pair(), Some((0, 1)));
        assert!(zero.is_zero());
    }

    #[test]
    fn aq_ver_001_k_row5_a_negative_denominator_moves_its_sign_up() {
        let f = Fraction::create_from_i128(3, -6);
        assert!(f.is_small());
        assert_eq!(f.extract_i64_pair(), Some((-1, 2)));
    }

    #[test]
    fn aq_ver_001_k_row6_i64_min_numerator_over_a_negative_widens() {
        // -2^63 / -3 = 2^63/3: the numerator's negation is past i64.
        let f = Fraction::create_from_i128(i64::MIN as i128, -3);
        assert!(!f.is_small());
        assert_eq!(f.to_bigint_pair(), (pow2(63), BigInt::from(3)));
    }

    #[test]
    fn aq_ver_001_k_row7_i64_min_denominator_widens() {
        // 1 / -2^63 = -1 / 2^63: the denominator's negation is past i64.
        let f = Fraction::create_from_i128(1, i64::MIN as i128);
        assert!(!f.is_small());
        assert_eq!(f.to_bigint_pair(), (BigInt::from(-1), pow2(63)));
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-L
// DUT: rust/src/types/fraction.rs, the compound guards of the integer reads:
//
//   `is_zero`:                only the canonical 0/1
//   `as_usize` (Big):         through `as_u64`
//   `as_u64` (Big):           !denominator.is_one()      -> None
//
// AQ-VER-001-C covers the `Small` arm of `as_usize`; these are the rest.
//
// is_zero (Z = n == 0, D = d != 0):
//   (T, T) 0/1 -> true;  (F, T) 1/1 -> false [Z];  (T, F) 0/0 -> false [D]
//   The `Big` arm has no true row: zero is always `Small(0, 1)`.
// to_u64 Small (Z = d == 0, N = n < 0):
//   (F, F) 7/2 -> Some(3);  (T, F) 1/0 -> None [Z];  (F, T) -7/2 -> None [N]
// as_usize Big (I = !d.is_one(), N = n < 0):
//   (F, F) 2^63 -> Some;  (T, F) (2^63+2)/3 -> None [I];  (F, T) -(2^63+1) -> None [N]
// The single-condition reads the coverage run found unexecuted: the
// inherent `to_i64` on `Big`, `Display` of a `Big` non-integer, and the
// empty-string and signed trailing-dot arms of `from_str`.
// ---------------------------------------------------------------------------
mod integer_reads {
    use super::*;

    #[test]
    fn aq_ver_001_l_is_zero_needs_a_zero_numerator_over_a_nonzero_denominator() {
        assert!(small(0, 1).is_zero());
        assert!(!small(1, 1).is_zero(), "Z flips the outcome");
        assert!(
            !Fraction::nullity().is_zero(),
            "D flips the outcome: 0/0 is not zero"
        );
        assert!(!big(pow2(70), 1).is_zero());
    }

    #[test]
    fn aq_ver_001_l_as_usize_big_declines_a_fraction_or_a_negative() {
        let integer = big(pow2(63), 1);
        assert!(!integer.is_small());
        assert_eq!(integer.as_usize(), usize::try_from(1u64 << 63).ok());
        let fraction = big(pow2(63) + 2, 3);
        assert!(!fraction.is_small(), "2^63 + 2 is coprime to 3");
        assert_eq!(fraction.as_usize(), None, "I flips the outcome");
        assert_eq!(
            big(-(pow2(63) + 1i32), 1).as_usize(),
            None,
            "N flips the outcome"
        );
    }

    #[test]
    fn aq_ver_001_l_inherent_to_i64_on_big() {
        // Integral but past i64, and not integral: both decline.
        assert_eq!(big(pow2(63), 1).to_i64(), None);
        assert_eq!(big(pow2(70) + 1, 3).to_i64(), None);
        assert_eq!(small(-7, 1).to_i64(), Some(-7));
        assert_eq!(small(7, 2).to_i64(), None);
    }

    #[test]
    fn aq_ver_001_l_display_and_the_remaining_parse_arms() {
        assert_eq!(big(pow2(70) + 1, 3).to_string(), "1180591620717411303425/3");
        assert_eq!(big(pow2(70), 1).to_string(), "1180591620717411303424");
        assert!(Fraction::from_str("").is_err());
        assert_eq!(Fraction::from_str("-5.").unwrap(), small(-5, 1));
        assert_eq!(Fraction::from_str("+5.").unwrap(), small(5, 1));
    }

    #[test]
    fn aq_ver_001_l_as_u64_big_declines_a_fraction() {
        assert_eq!(big(pow2(63), 1).as_u64(), Some(1u64 << 63));
        assert_eq!(big(pow2(63) + 2, 3).as_u64(), None);
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-M
// DUT: rust/src/types/fraction.rs `Fraction::from_str`, the exponent branch
//
//     if exponent_digits.is_empty()                                  // E
//         || !exponent_digits.bytes().all(|b| b.is_ascii_digit())    // X
//     { return Err(..) }
//     if mn.is_zero() { return Ok(0) }                               // Z
//     let exponent: i32 = exponent_str.parse()?;                     // R
//
// E || X (form):
//   (F, F) "1e5"  -> Ok;  (T, *) "1e" / "1e+" -> Err [E];  (F, T) "1eX" -> Err [X]
// Z, then R (range): the form is checked before the range, and a zero
// mantissa before the range too:
//   Z = T, R = F  "0e99999999999" -> Ok(0)
//   Z = F, R = F  "1e99999999999" -> Err      pair shows Z
//   Z = F, R = T  "1e-2"          -> Ok(1/100)
// ---------------------------------------------------------------------------
mod from_str_exponent_guard {
    use super::*;

    #[test]
    fn aq_ver_001_m_a_well_formed_exponent_scales() {
        assert_eq!(Fraction::from_str("1e5").unwrap(), small(100_000, 1));
        assert_eq!(Fraction::from_str("1e-2").unwrap(), small(1, 100));
        assert_eq!(Fraction::from_str("3E+1").unwrap(), small(30, 1));
    }

    #[test]
    fn aq_ver_001_m_an_empty_exponent_is_refused() {
        for text in ["1e", "1e+", "1e-", "1.5E"] {
            assert!(Fraction::from_str(text).is_err(), "`{text}`");
        }
    }

    #[test]
    fn aq_ver_001_m_a_non_digit_exponent_is_refused() {
        for text in ["1eX", "1e+5x", "1e--5", "1e5.0"] {
            assert!(Fraction::from_str(text).is_err(), "`{text}`");
        }
    }

    #[test]
    fn aq_ver_001_m_a_zero_mantissa_skips_the_range_check() {
        assert_eq!(Fraction::from_str("0e99999999999").unwrap(), small(0, 1));
        assert_eq!(Fraction::from_str("0.0e-99999999999").unwrap(), small(0, 1));
        assert!(Fraction::from_str("1e99999999999").is_err());
    }

    /// A non-ASCII letter is never the exponent marker.
    #[test]
    fn aq_ver_001_m_a_non_ascii_character_is_no_exponent() {
        assert!(Fraction::from_str("１e5").is_err());
        assert!(Fraction::from_str("1ｅ5").is_err());
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-N
// DUT: rust/src/types/fraction_order.rs `Fraction::order` / `cmp_finite`, and
// rust/src/types/fraction.rs `impl PartialEq`: the rows AQ-VER-001-A and
// AQ-VER-001-B leave out.
//
// order, `!self.is_finite() || !other.is_finite()` (S, O):
//   AQ-VER-001-B has (T, F) and (T, T); here (F, T), rational against a point.
// cmp_finite and PartialEq, `(Some(..), Some(..))` on the i64 pairs (A, B):
//   AQ-VER-001-B has (T, T) and (F, F); here (T, F) and (F, T).
// PartialEq and impl Hash read the canonical pair as it is held.
// ---------------------------------------------------------------------------
mod mixed_representation_order_and_equality {
    use super::*;

    #[test]
    fn aq_ver_001_n_a_rational_against_a_point() {
        // The point at infinity is unsigned, so no rational of either
        // representation lies above or below it.
        let wide = big(pow2(70), 1);
        assert_eq!(wide.order(&Fraction::infinity()), None);
        assert_eq!(Fraction::infinity().order(&wide), None);
        assert_eq!(small(1, 1).order(&Fraction::infinity()), None);
        assert_eq!(small(0, 1).order(&Fraction::infinity()), None);
    }

    #[test]
    fn aq_ver_001_n_small_against_big_orders_both_ways() {
        let wide = big(pow2(70), 1);
        assert_eq!(small(1, 1).order(&wide), Some(Ordering::Less));
        assert_eq!(wide.order(&small(1, 1)), Some(Ordering::Greater));
        let wide_negative = big(-pow2(70), 3);
        assert_eq!(small(-1, 1).order(&wide_negative), Some(Ordering::Greater));
        assert_eq!(wide_negative.order(&small(-1, 1)), Some(Ordering::Less));
    }

    #[test]
    fn aq_ver_001_n_small_against_big_is_never_equal() {
        let wide = big(pow2(70) + 1, 3);
        assert_ne!(small(1, 3), wide);
        assert_ne!(wide, small(1, 3));
    }

    #[test]
    fn aq_ver_001_n_big_equality_with_a_shared_denominator_compares_numerators() {
        let a = big(pow2(70) + 1, 3);
        assert_eq!(a, big(pow2(70) + 1, 3));
        assert_ne!(a, big(pow2(70) + 4, 3), "(C'=T): the numerators decide");
        assert_ne!(
            a,
            big(pow2(70) + 1, 6),
            "(C'=F), unequal by cross-multiplication"
        );
    }

    fn hash_of(f: &Fraction) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        f.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn aq_ver_001_n_every_fraction_is_held_canonically() {
        // Lowest terms, and `Small` exactly when both halves fit a word, so
        // equality and the hash read the pair as it is held.
        let third = Fraction::new(BigInt::from(2), BigInt::from(6));
        assert!(third.is_small());
        assert_eq!(third, small(1, 3));
        let three = Fraction::new(pow2(70) * 3, pow2(70));
        assert!(three.is_small());
        assert_eq!(hash_of(&three), hash_of(&small(3, 1)));
        let wide = Fraction::new(pow2(71) + 2, BigInt::from(6));
        assert_eq!(wide, big(pow2(70) + 1, 3));
        assert_eq!(hash_of(&wide), hash_of(&big(pow2(70) + 1, 3)));
        assert_ne!(
            hash_of(&big(pow2(70) + 1, 3)),
            hash_of(&big(pow2(70) + 4, 3))
        );
        assert_eq!(big(-pow2(63), 1).neg(), big(pow2(63), 1));
        assert!(big(pow2(63), 1).neg().is_small());
    }
}
