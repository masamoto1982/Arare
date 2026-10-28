//! MC/DC test suite for `crate::types::fraction`.

// AQ-VER-001: Fraction MC/DC tests for QL-A boolean decisions.
//
// Scope: numeric core (`Fraction`) — boolean decisions whose
// independent atomic conditions can each cause an incorrect
// arithmetic or comparison result if mis-evaluated.
//
// Each submodule below documents:
//   * DUT (decision under test) — file:line and the boolean expression
//   * Conditions — atomic predicates A, B, ...
//   * MC/DC table — pairs of rows that demonstrate each condition
//     independently flipping the outcome
//
// Trace: docs/quality/TRACEABILITY_MATRIX.md, requirement AQ-REQ-001.

use crate::types::fraction::Fraction;
use num_bigint::BigInt;
use std::str::FromStr;

// The tables past this file's line budget: AQ-VER-001-J to -N, and -O to -R.
mod arithmetic;
mod repr;

fn small(n: i64, d: i64) -> Fraction {
    Fraction::new(BigInt::from(n), BigInt::from(d))
}

fn big_int(n: i64) -> Fraction {
    let big = BigInt::from(i64::MAX) * BigInt::from(2i64) + BigInt::from(n);
    Fraction::new(big, BigInt::from(1))
}

// AQ-VER-001-A
// DUT: `impl PartialEq for Fraction`
//
//     if !self.is_finite() || !other.is_finite() {
//         return self.extract_i64_pair() == other.extract_i64_pair();
//     }
//
// Conditions:
//   A = !self.is_finite()
//   B = !other.is_finite()
//
// MC/DC for A||B (entry guard):
//   row 1: (A=T, B=F) -> guard taken
//   row 2: (A=F, B=T) -> guard taken
//   row 3: (A=F, B=F) -> guard skipped
//   Pair (1,3) shows A independently flips outcome (B held F).
//   Pair (2,3) shows B independently flips outcome (A held F).
//
// MC/DC for A&&B (returned value when guard taken):
//   row 4: (A=T, B=T) -> true
//   row 5: (A=T, B=F) -> false
//   row 6: (A=F, B=T) -> false
//   Pair (4,5) shows B independently flips outcome (A held T).
//   Pair (4,6) shows A independently flips outcome (B held T).
mod point_equality_guard {
    use super::*;

    #[test]
    fn aq_ver_001_a_row1_self_point_other_rational_guard_taken_returns_false() {
        // (A=T, B=F): guard taken, the pairs differ -> not equal.
        let lhs = Fraction::infinity();
        let rhs = small(1, 1);
        assert_ne!(lhs, rhs, "1/0 and 1/1 must not be equal");
    }

    #[test]
    fn aq_ver_001_a_row2_other_point_self_rational_guard_taken_returns_false() {
        // (A=F, B=T): guard taken, the pairs differ -> not equal.
        let lhs = small(0, 1);
        let rhs = Fraction::nullity();
        assert_ne!(lhs, rhs, "0/1 and 0/0 must not be equal");
    }

    #[test]
    fn aq_ver_001_a_row3_neither_nil_guard_skipped_compares_repr() {
        // (A=F, B=F): guard skipped, falls through to repr comparison.
        let lhs = small(2, 4); // reduces to 1/2
        let rhs = small(1, 2);
        assert_eq!(lhs, rhs, "1/2 and 2/4 must compare equal after reduction");

        let unequal = small(1, 3);
        assert_ne!(lhs, unequal);
    }

    #[test]
    fn aq_ver_001_a_row4_both_points_compare_as_pairs() {
        // (A=T, B=T): the pairs decide — equal when the same point, and never
        // by cross-multiplication, which would make 1/0 and 0/0 equal.
        assert_eq!(
            Fraction::nullity(),
            Fraction::nullity(),
            "0/0 must equal 0/0"
        );
        assert_eq!(Fraction::infinity(), Fraction::infinity());
        assert_ne!(
            Fraction::infinity(),
            Fraction::nullity(),
            "1/0 and 0/0 are two points"
        );
    }
}

// AQ-VER-001-B
// DUT: `Fraction::cmp_finite`, reached through `Fraction::order`
//
//     if let (Some((a, b)), Some((c, d))) = (self.extract_i64_pair(),
//                                            other.extract_i64_pair()) {
//         if b == d { return a.cmp(&c); }
//         ...
//     }
//
// Conditions for the same-denominator branch:
//   A = self.extract_i64_pair().is_some()
//   B = other.extract_i64_pair().is_some()
//   C = b == d
//
// We test the three boundary combinations that exercise the if-let pattern
// (small/small, small/big, big/big) and, within the small/small arm, both
// values of C.
mod cmp_small_fast_path {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn aq_ver_001_b_small_small_same_denominator_compares_numerators() {
        // A=T, B=T, C=T: enters fast path, compares a vs c directly.
        let a = small(3, 7);
        let b = small(5, 7);
        assert_eq!(a.order(&b), Some(Ordering::Less));
        assert_eq!(b.order(&a), Some(Ordering::Greater));
        assert_eq!(a.order(&a.clone()), Some(Ordering::Equal));
    }

    #[test]
    fn aq_ver_001_b_small_small_different_denominator_cross_multiplies() {
        // A=T, B=T, C=F: skips a.cmp(&c), takes the i128 cross-multiply path.
        // 1/3 vs 1/4 -> 4 vs 3 -> Greater.
        let a = small(1, 3);
        let b = small(1, 4);
        assert_eq!(a.order(&b), Some(Ordering::Greater));
    }

    #[test]
    fn aq_ver_001_b_big_path_when_extraction_fails() {
        // A=F or B=F: at least one side is BigInt, falls through to BigInt
        // arithmetic. The same-denominator BigInt branch must also work.
        let a = big_int(0); // 2*i64::MAX, denominator 1
        let b = big_int(1); // 2*i64::MAX + 1, denominator 1
        assert_eq!(a.order(&b), Some(Ordering::Less));
        assert_eq!(b.order(&a), Some(Ordering::Greater));

        // Different-denominator BigInt path.
        let c = Fraction::new(BigInt::from(1), BigInt::from(i64::MAX) * BigInt::from(2));
        let d = Fraction::new(BigInt::from(1), BigInt::from(i64::MAX) * BigInt::from(3));
        assert_eq!(c.order(&d), Some(Ordering::Greater));
    }

    #[test]
    fn aq_ver_001_b_the_points_over_zero_order_or_do_not() {
        // Neither point is ordered against anything, however wide the other
        // operand, itself included.
        for point in [Fraction::infinity(), Fraction::nullity()] {
            assert_eq!(point.order(&big_int(0)), None);
            assert_eq!(big_int(0).order(&point), None);
            assert_eq!(point.order(&small(1, 1)), None);
            assert_eq!(point.order(&point), None);
        }
        assert_eq!(Fraction::infinity().order(&Fraction::nullity()), None);
    }
}

// AQ-VER-001-C
// DUT: rust/src/types/fraction.rs `Fraction::as_usize` (Small arm; the Big
// arm is AQ-VER-001-L in `fraction_mcdc_tests::repr`)
//
//     if *d != 1 || *n < 0 { return None; }   // = !(d == 1 && n >= 0)
//     usize::try_from(*n).ok()
//
// Conditions:
//   A = (d == 1)
//   B = (n >= 0)
//
// MC/DC table:
//   row 1: (T, T) -> Some
//   row 2: (T, F) -> None  (proves B flips outcome with A held T)
//   row 3: (F, T) -> None  (proves A flips outcome with B held T)
//   row 4: (F, F) -> None  (boundary; not required for MC/DC but documented)
//
// Note on reachability: a normalized Small fraction with d != 1 always has
// gcd(n, d) = 1, so non-integer values (rows 3, 4) are constructible via
// Fraction::new and remain in Small form when both numerator and denominator
// fit in i64.
mod as_usize_small {
    use super::*;

    #[test]
    fn aq_ver_001_c_row1_integer_nonneg_returns_some() {
        let f = small(42, 1);
        assert_eq!(f.as_usize(), Some(42));
    }

    #[test]
    fn aq_ver_001_c_row2_integer_negative_returns_none() {
        let f = small(-1, 1);
        assert_eq!(
            f.as_usize(),
            None,
            "negative integer must not coerce to usize"
        );
    }

    #[test]
    fn aq_ver_001_c_row3_nonintegral_positive_returns_none() {
        let f = small(3, 4);
        assert_eq!(f.as_usize(), None, "non-integer fraction must not coerce");
    }

    #[test]
    fn aq_ver_001_c_row4_nonintegral_negative_returns_none() {
        let f = small(-3, 4);
        assert_eq!(f.as_usize(), None);
    }
}

// AQ-VER-001-D
// DUT: rust/src/types/fraction-arithmetic.rs:54-58 in `Fraction::add`
//
//     if b == 1 && d == 1 {
//         return Self::create_from_i128((a as i128) + (c as i128), 1);
//     }
//     if b == d {
//         return Self::create_from_i128((a as i128) + (c as i128), b as i128);
//     }
//
// Decision 1 conditions (integer fast path):
//   A = (b == 1)
//   B = (d == 1)
//
// MC/DC table for A&&B:
//   row 1: (T, T) -> integer fast path
//   row 2: (T, F) -> falls through (B flips outcome with A held T)
//   row 3: (F, T) -> falls through (A flips outcome with B held T)
//
// Decision 2 conditions (same-denominator fast path):
//   C = (b == d)  reached when not (A && B)
//   row 4: C = T  -> common-denominator fast path
//   row 5: C = F  -> generic cross-multiply path
mod add_small_fast_paths {
    use super::*;

    #[test]
    fn aq_ver_001_d_decision1_row1_both_integer() {
        // (A=T, B=T): integer + integer fast path.
        let a = small(7, 1);
        let b = small(11, 1);
        let sum = a.add(&b);
        assert_eq!(sum, small(18, 1));
    }

    #[test]
    fn aq_ver_001_d_decision1_row2_self_integer_other_fraction() {
        // (A=T, B=F): self denominator 1, other not -> falls through to
        // Decision 2; b=1, d=3 -> b != d -> generic path. 7 + 1/3 = 22/3.
        let a = small(7, 1);
        let b = small(1, 3);
        let sum = a.add(&b);
        assert_eq!(sum, small(22, 3));
    }

    #[test]
    fn aq_ver_001_d_decision1_row3_self_fraction_other_integer() {
        // (A=F, B=T): symmetric to row 2. 1/3 + 7 = 22/3.
        let a = small(1, 3);
        let b = small(7, 1);
        let sum = a.add(&b);
        assert_eq!(sum, small(22, 3));
    }

    #[test]
    fn aq_ver_001_d_decision2_row4_same_denominator_nonunit() {
        // (A=F, B=F, C=T): both have denominator 5 -> common-den fast path.
        let a = small(2, 5);
        let b = small(1, 5);
        let sum = a.add(&b);
        assert_eq!(sum, small(3, 5));
    }

    #[test]
    fn aq_ver_001_d_decision2_row5_different_denominator() {
        // (A=F, B=F, C=F): generic cross-multiply path.
        // 1/3 + 1/4 = 7/12.
        let a = small(1, 3);
        let b = small(1, 4);
        let sum = a.add(&b);
        assert_eq!(sum, small(7, 12));
    }

    #[test]
    fn aq_ver_001_d_a_point_over_zero_takes_the_extended_law() {
        // The entry guard: an operand over zero, on either side, decides the
        // sum by the pair formulas rather than by the rational sum.
        let nullity = Fraction::nullity();
        let one = small(1, 1);
        assert_eq!(nullity.add(&one), nullity);
        assert_eq!(one.add(&nullity), nullity);
        assert_eq!(Fraction::infinity().add(&one), Fraction::infinity());
    }
}

// AQ-VER-001-E
// DUT: rust/src/types/fraction-arithmetic.rs:268 in `Fraction::floor` (Small)
//
//     let floored = if *n < 0 && r != 0 { q - 1 } else { q };
//
// Conditions:
//   A = (n < 0)
//   B = (r != 0)
//
// Reachability note: this code runs only after `is_integer()` returns false,
// so when `repr` is `Small` we have d > 1. Because `Fraction::new` reduces by
// gcd, any Small with d > 1 satisfies gcd(n, d) = 1, hence n % d != 0. So in
// normal use B = T always, and rows (T,F)/(F,F) are unreachable. We still
// exercise both A values to demonstrate independent effect of A, and we
// exercise the early-return path for integers as a separate row.
mod floor_negative_remainder {
    use super::*;

    #[test]
    fn aq_ver_001_e_row1_negative_with_remainder_rounds_toward_neg_inf() {
        // (A=T, B=T): -7/3 -> q=-2, r=-1, floored = -3.
        let f = small(-7, 3);
        assert_eq!(f.floor(), small(-3, 1));
    }

    #[test]
    fn aq_ver_001_e_row2_positive_with_remainder_truncates() {
        // (A=F, B=T): 7/3 -> q=2, r=1, floored = 2 (else branch).
        // Pair (row1, row2) holds B=T and flips A, demonstrating A's
        // independent effect on the outcome.
        let f = small(7, 3);
        assert_eq!(f.floor(), small(2, 1));
    }

    #[test]
    fn aq_ver_001_e_integer_short_circuits_before_decision() {
        // is_integer() short-circuit returns self before the decision is
        // evaluated. Documents the only realistic way to reach B = F.
        let f = small(-6, 1);
        assert_eq!(f.floor(), small(-6, 1));
    }

    #[test]
    fn aq_ver_001_e_big_path_negative_with_remainder() {
        // BigInt twin of row 1 to cover the parallel decision in the Big arm.
        // -18446744073709551616 (= -(2*i64::MAX + 2)) does not fit i64 so the
        // value stays in the Big representation. Divided by 3 it has trunc
        // quotient -6148914691236517205 with remainder -1, so floor = q - 1.
        let big_neg = BigInt::from_str("-18446744073709551616").unwrap();
        let f = Fraction::new(big_neg, BigInt::from(3));
        let expected = Fraction::new(
            BigInt::from_str("-6148914691236517206").unwrap(),
            BigInt::from(1),
        );
        assert_eq!(f.floor(), expected);
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-G
// DUT: rust/src/types/fraction.rs:266-269 in `Fraction::create_from_i128`
//
//     if n >= i64::MIN as i128 && n <= i64::MAX as i128
//         && d >= 0 && d <= i64::MAX as i128
//     {
//         return Fraction { repr: FractionRepr::Small(n as i64, d as i64) };
//     }
//
// This 4-condition AND decides whether the i128 (numerator, denominator) pair
// fits the Small representation. Conditions:
//   A = n >= i64::MIN as i128
//   B = n <= i64::MAX as i128
//   C = d >= 0
//   D = d <= i64::MAX as i128
//
// Reachability:
//   A, B, D each gate a distinct overflow direction (n underflow, n overflow,
//   d overflow). All three are reachable through arithmetic that produces
//   results outside i64 range (e.g., adding two near-i64::MAX values).
//
//   C is structurally always T at this site: the immediately preceding block
//   at fraction.rs:261-264 normalizes d to be non-negative
//   (`if d LT 0 { n EQ -n; d EQ -d; }`), so by the time line 267 evaluates
//   `d >= 0`, this condition is invariant. Treated as defensive code; the
//   row C=F is unreachable without bypassing the normalizer.
//
// MC/DC over A && B && D (with C held T):
//   row 1: (A=T, B=T, D=T) -> Small  (n in i64 range, d in i64 range)
//   row 2: (A=F, B=T, D=T) -> Big    (n < i64::MIN as i128)
//   row 3: (A=T, B=F, D=T) -> Big    (n > i64::MAX as i128)
//   row 4: (A=T, B=T, D=F) -> Big    (d > i64::MAX as i128)
//
// Independent effect:
//   Pair (1, 2) with B,D held T: A flips T->F -> outcome flips Small->Big.
//   Pair (1, 3) with A,D held T: B flips T->F -> outcome flips Small->Big.
//   Pair (1, 4) with A,B held T: D flips T->F -> outcome flips Small->Big.
// ---------------------------------------------------------------------------
mod create_from_i128_small_big_boundary {
    use super::*;

    #[test]
    fn aq_ver_001_g_row1_in_range_returns_small() {
        // (A=T, B=T, D=T): 5/3, both fit i64.
        let f = Fraction::create_from_i128(5, 3);
        assert!(f.is_small(), "in-range pair must produce Small");
        assert_eq!(f, small(5, 3));
    }

    #[test]
    fn aq_ver_001_g_row2_numerator_underflow_returns_big() {
        // (A=F, B=T, D=T): n = i64::MIN - 1 as i128, which is below i64 range.
        // Pair (row1, row2) with B,D held T proves A's independent effect.
        let n: i128 = (i64::MIN as i128) - 1;
        let f = Fraction::create_from_i128(n, 1);
        assert!(!f.is_small(), "n below i64::MIN must promote to Big");
        // Confirm magnitude survives the round-trip through BigInt.
        assert_eq!(f, Fraction::new(BigInt::from(n), BigInt::from(1)));
    }

    #[test]
    fn aq_ver_001_g_row3_numerator_overflow_returns_big() {
        // (A=T, B=F, D=T): n = i64::MAX + 1 as i128, which is above i64 range.
        // Pair (row1, row3) with A,D held T proves B's independent effect.
        let n: i128 = (i64::MAX as i128) + 1;
        let f = Fraction::create_from_i128(n, 1);
        assert!(!f.is_small(), "n above i64::MAX must promote to Big");
        assert_eq!(f, Fraction::new(BigInt::from(n), BigInt::from(1)));
    }

    #[test]
    fn aq_ver_001_g_row4_denominator_overflow_returns_big() {
        // (A=T, B=T, D=F): d > i64::MAX as i128. Use n = 1 (positive, in range)
        // and d = (i64::MAX as i128) + 2 (gcd-coprime with 1, so no reduction
        // shrinks d back into range).
        // Pair (row1, row4) with A,B held T proves D's independent effect.
        let d: i128 = (i64::MAX as i128) + 2;
        let f = Fraction::create_from_i128(1, d);
        assert!(!f.is_small(), "d above i64::MAX must promote to Big");
        assert_eq!(f, Fraction::new(BigInt::from(1), BigInt::from(d)));
    }

    #[test]
    fn aq_ver_001_g_normalizes_negative_denominator_before_check() {
        // C=T invariant check: a negative d input is normalized to positive
        // before line 267 evaluates `d >= 0`. The result still depends on
        // A,B,D. Here d = -3 is normalized to d = +3 with sign flipped onto n.
        let f = Fraction::create_from_i128(5, -3);
        assert!(
            f.is_small(),
            "after normalization both n,d fit i64 -> Small"
        );
        assert_eq!(f, small(-5, 3));
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-H
// DUT: rust/src/types/fraction-arithmetic.rs:53 in `Fraction::add`
//
//     if let (Some((a, b)), Some((c, d))) =
//         (self.extract_i64_pair(), other.extract_i64_pair()) { /* fast path */ }
//
// Tuple-destructuring guard at the entry of the i64 fast path. Conditions:
//   A = self.extract_i64_pair().is_some()    (self fits i64 pair)
//   B = other.extract_i64_pair().is_some()   (other fits i64 pair)
//
// extract_i64_pair returns Some for any Small fraction unconditionally, and
// for Big fractions only when both numerator and denominator individually
// fit i64 (rust/src/types/fraction.rs:204-213). A genuine Big fraction whose
// numerator overflows i64 returns None.
//
// MC/DC over A && B:
//   row 1: (A=T, B=T) -> fast path entered; result computed in i128
//   row 2: (A=F, B=T) -> fast path skipped; falls through to BigInt arm
//   row 3: (A=T, B=F) -> fast path skipped; falls through to BigInt arm
//
// Independent effect:
//   Pair (1, 2) with B held T: A flips T->F -> path flips fast->big.
//   Pair (1, 3) with A held T: B flips T->F -> path flips fast->big.
//
// We observe path selection indirectly: for row 1 we verify the result is
// Small (no boundary crossing); for rows 2 and 3 we verify the BigInt arm
// preserves correctness when one operand exceeds i64.
// ---------------------------------------------------------------------------
mod add_small_fast_path_entry_guard {
    use super::*;

    #[test]
    fn aq_ver_001_h_row1_both_small_uses_fast_path() {
        // (A=T, B=T): both Small, result fits i64 -> Small returned.
        let lhs = small(2, 3);
        let rhs = small(1, 6);
        let result = lhs.add(&rhs);
        assert!(result.is_small(), "two Small operands with i64-fitting sum");
        assert_eq!(result, small(5, 6));
    }

    #[test]
    fn aq_ver_001_h_row2_self_big_skips_fast_path() {
        // (A=F, B=T): self is Big (numerator > i64::MAX) -> falls into BigInt arm.
        // Pair (row1, row2) flips A with B held T.
        let big = big_int(0); // 2*i64::MAX, exceeds i64
        let small_one = small(1, 1);
        let result = big.add(&small_one);
        // Expected: 2*i64::MAX + 1
        let expected_num = BigInt::from(i64::MAX) * BigInt::from(2i64) + BigInt::from(1i64);
        assert_eq!(result, Fraction::new(expected_num, BigInt::from(1)));
        assert!(
            !result.is_small(),
            "Big + Small with overflow must stay Big"
        );
    }

    #[test]
    fn aq_ver_001_h_row3_other_big_skips_fast_path() {
        // (A=T, B=F): other is Big -> falls into BigInt arm.
        // Pair (row1, row3) flips B with A held T.
        let small_one = small(1, 1);
        let big = big_int(0);
        let result = small_one.add(&big);
        let expected_num = BigInt::from(i64::MAX) * BigInt::from(2i64) + BigInt::from(1i64);
        assert_eq!(result, Fraction::new(expected_num, BigInt::from(1)));
        assert!(
            !result.is_small(),
            "Small + Big with overflow must stay Big"
        );
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-001-I
// DUT: rust/src/types/fraction-arithmetic.rs:60-64 in `Fraction::add`
//
//     if let Some(num) = (a as i128).checked_mul(d as i128)        // X
//         .and_then(|ad| (c as i128).checked_mul(b as i128)        // Y
//             .and_then(|cb| ad.checked_add(cb)))                  // Z
//     { return Self::create_from_i128(num, (b as i128) * (d as i128)); }
//
// Three checked operations chained with and_then. Atomic conditions:
//   X = (a as i128).checked_mul(d as i128).is_some()
//   Y = (c as i128).checked_mul(b as i128).is_some()
//   Z = ad.checked_add(cb).is_some()
//
// Reachability proof for the None arm (i.e., the fall-through to the BigInt
// arm at lines 68+):
//   The site is guarded by extract_i64_pair returning Some for both operands,
//   so a, c are i64 (range [-2^63, 2^63 - 1]) and b, d are i64 denominators
//   normalized non-negative (range [1, 2^63 - 1]).
//
//   |a as i128| <= 2^63;  |d as i128| <= 2^63 - 1
//   |a*d| <= 2^63 * (2^63 - 1) = 2^126 - 2^63 < 2^127 = i128::MAX + 1
//   So X = T for all valid inputs. Symmetric argument: Y = T for all valid
//   inputs.
//
//   ad and cb each lie in [-(2^126 - 2^63), 2^126 - 2^63].
//   |ad + cb| <= 2 * (2^126 - 2^63) = 2^127 - 2^64 < 2^127.
//   So Z = T for all valid inputs.
//
//   Therefore X && Y && Z = T whenever the fast path is entered. The None
//   arm is structurally unreachable from i64 operands; it is defensive code
//   guarding against a future regression where the upstream contracts (e.g.,
//   non-negative denominator) are weakened.
//
// MC/DC table:
//   row 1: (X=T, Y=T, Z=T)         -> Some(num); REACHABLE (asserted below)
//   row 2: (X=F, Y=*, Z=*)         -> None;      UNREACHABLE (proof above)
//   row 3: (X=T, Y=F, Z=*)         -> None;      UNREACHABLE (proof above)
//   row 4: (X=T, Y=T, Z=F)         -> None;      UNREACHABLE (proof above)
//
// Because rows 2-4 cannot be constructed without violating the i64 range
// invariant, classic MC/DC pair construction does not apply; the tests below
// instead exercise row 1 at the operand boundary closest to the i128 limit
// to demonstrate the proof's tightness empirically.
// ---------------------------------------------------------------------------
mod add_checked_chain_defensive {
    use super::*;

    // The operands need two different denominators: a fraction such as
    // i64::MAX/i64::MAX reduces to 1/1 and takes the integer fast path, and
    // a shared denominator takes `b == d` (AQ-VER-001-D) — neither reaches
    // the chain. Consecutive integers are coprime, so i64::MAX/(i64::MAX - 1)
    // stays as written.
    #[test]
    fn aq_ver_001_i_row1_extreme_i64_inputs_succeed_in_i128() {
        //   a/b = i64::MAX/(i64::MAX - 1),  c/d = (i64::MAX - 1)/i64::MAX
        //   ad = (2^63 - 1)^2,  cb = (2^63 - 2)^2,  ad + cb < 2^127
        let (a, b) = (i64::MAX, i64::MAX - 1);
        let result = small(a, b).add(&small(b, a));
        let expected = Fraction::new(
            BigInt::from(a) * a + BigInt::from(b) * b,
            BigInt::from(a) * b,
        );
        assert_eq!(result, expected);
        assert!(!result.is_small(), "the sum's numerator is past a word");
    }

    #[test]
    fn aq_ver_001_i_row1_negative_extreme_i64_inputs_succeed_in_i128() {
        // |a| = 2^63, the bound the proof uses:
        //   a/b = i64::MIN/i64::MAX,  c/d = -(2^63 - 1)/(2^63 - 2)
        //   |ad + cb| = 2^63 (2^63 - 2) + (2^63 - 1)^2 = 2^127 - 2^65 + 1 < 2^127
        let (b, d) = (i64::MAX, i64::MAX - 1);
        let result = small(i64::MIN, b).add(&small(-i64::MAX, d));
        let expected = Fraction::new(
            BigInt::from(i64::MIN) * d + BigInt::from(-i64::MAX) * b,
            BigInt::from(b) * d,
        );
        assert_eq!(result, expected);
        assert!(!result.is_small(), "the sum's numerator is past a word");
    }
}

// ---------------------------------------------------------------------------
// i64::MIN normalization (regression)
// DUT: rust/src/types/fraction.rs `compute_gcd_i64` and `Fraction::new`
//
// `i64::MIN` has no positive i64 counterpart, so `i64::MIN.abs()` and
// `-i64::MIN` both overflow and panic. The old gcd reduced operands with
// signed `abs()`, and `Fraction::new` sign-normalized the Small path with
// `num = -num`; either crashed the interpreter as soon as a
// `-9223372036854775808` literal entered the fraction normalizer (most
// visibly when stored inside a vector/tensor). These tests pin the totalized
// behavior: every i64::MIN form must reduce to an exact value rather than
// panic.
// ---------------------------------------------------------------------------
mod i64_min_normalization {
    use super::*;

    #[test]
    fn new_accepts_i64_min_numerator() {
        // Previously panicked in compute_gcd_i64's signed abs().
        let f = Fraction::new(BigInt::from(i64::MIN), BigInt::from(1));
        assert_eq!(f, small(i64::MIN, 1));
    }

    #[test]
    fn new_accepts_i64_min_over_negative_one() {
        // Sign normalization (`num = -num`) used to overflow on i64::MIN; the
        // exact value 2^63 no longer fits in i64 and must widen to Big.
        let f = Fraction::new(BigInt::from(i64::MIN), BigInt::from(-1));
        let expected = Fraction::new(-BigInt::from(i64::MIN), BigInt::from(1));
        assert_eq!(f, expected);
    }

    #[test]
    fn new_reduces_i64_min_over_i64_min_to_one() {
        let f = Fraction::new(BigInt::from(i64::MIN), BigInt::from(i64::MIN));
        assert_eq!(f, small(1, 1));
    }

    #[test]
    fn multiply_i64_min_operands_widens_to_big() {
        // i64::MIN * i64::MIN == 2^126, well outside i64; must not panic.
        let product = small(i64::MIN, 1).mul(&small(i64::MIN, 1));
        let expected = Fraction::new(
            BigInt::from(i64::MIN) * BigInt::from(i64::MIN),
            BigInt::from(1),
        );
        assert_eq!(product, expected);
    }
}

// ---------------------------------------------------------------------------
// Scientific-notation exponent overflow (regression)
// DUT: rust/src/types/fraction.rs `Fraction::from_str` exponent branch
//
// The negative-exponent branch computed `(-exponent) as u32`. For the literal
// `1e-2147483648` the exponent parses to i32::MIN, whose negation overflows i32
// and panicked the interpreter (reachable from a source literal and from `NUM`
// on a string). `unsigned_abs` takes the magnitude without overflow.
// ---------------------------------------------------------------------------
mod scientific_exponent_overflow {
    use super::*;

    #[test]
    fn from_str_handles_i32_min_negative_exponent_small() {
        // 1e-1 still parses fine through the same magnitude path.
        let f = Fraction::from_str("1e-1").unwrap();
        assert_eq!(f, small(1, 10));
    }

    #[test]
    fn from_str_does_not_panic_on_i32_min_exponent_magnitude() {
        // We only assert it returns an Ok value rather than panicking; the
        // resulting denominator is astronomically large but well-formed. Use a
        // mantissa of 0 so the heavy power is multiplied away and the test stays
        // fast while still exercising the magnitude computation.
        let f = Fraction::from_str("0e-2147483648");
        assert!(f.is_ok(), "i32::MIN exponent must not panic");
        assert_eq!(f.unwrap(), small(0, 1));
    }
}

// ---------------------------------------------------------------------------
// `abs` and `as_usize` at the edges of the machine word (regression)
// DUT: rust/src/types/fraction_arithmetic.rs `Fraction::abs`,
//      rust/src/types/fraction.rs `Fraction::as_usize`
//
// `abs` took `i64::abs()` on the `Small` numerator, which overflows on
// `i64::MIN` — a debug panic, and in release the same negative value handed
// back as its own magnitude. `as_usize` cast with `as usize`, which on a
// 32-bit target (wasm32) truncates a count past `u32::MAX` to a small wrong
// count rather than declining it.
// ---------------------------------------------------------------------------
mod machine_word_edges {
    use super::*;

    #[test]
    fn abs_of_i64_min_is_its_exact_magnitude() {
        let magnitude = small(i64::MIN, 1).abs();
        assert_eq!(
            magnitude,
            Fraction::new(-BigInt::from(i64::MIN), BigInt::from(1))
        );
        assert!(magnitude.is_positive());
        // The denominator survives too.
        assert_eq!(
            small(i64::MIN, 3).abs(),
            Fraction::new(-BigInt::from(i64::MIN), BigInt::from(3))
        );
        // Every other `Small` stays `Small`.
        assert_eq!(small(-7, 2).abs(), small(7, 2));
        assert!(small(-7, 2).abs().is_small());
    }

    #[test]
    fn as_usize_declines_what_usize_cannot_hold() {
        assert_eq!(small(7, 1).as_usize(), Some(7));
        assert_eq!(small(-1, 1).as_usize(), None);
        assert_eq!(small(7, 2).as_usize(), None);
        // `i64::MAX` fits a 64-bit `usize` and no 32-bit one; either way the
        // answer is what `usize` can hold, never a truncation of it.
        assert_eq!(
            small(i64::MAX, 1).as_usize(),
            usize::try_from(i64::MAX).ok()
        );
        assert_eq!(
            small(u32::MAX as i64 + 2, 1).as_usize(),
            usize::try_from(u32::MAX as i64 + 2).ok()
        );
    }

    /// `as_u64` reads a count past `u32::MAX` the same on every target, and
    /// declines exactly what is not a non-negative integer of 64 bits.
    #[test]
    fn as_u64_reads_counts_past_32_bits_on_every_target() {
        assert_eq!(small(u32::MAX as i64 + 2, 1).as_u64(), Some(4_294_967_297));
        assert_eq!(small(i64::MAX, 1).as_u64(), Some(i64::MAX as u64));
        assert_eq!(small(-1, 1).as_u64(), None);
        assert_eq!(small(7, 2).as_u64(), None);
        let two_to_64 = Fraction::new(BigInt::from(u64::MAX) + 1, BigInt::from(1));
        assert_eq!(two_to_64.as_u64(), None);
        let u64_max = Fraction::new(BigInt::from(u64::MAX), BigInt::from(1));
        assert_eq!(u64_max.as_u64(), Some(u64::MAX));
    }

    /// Narrowing to 32 bits — what wasm32's `usize` is — saturates instead of
    /// failing or truncating, so a count of `2^32 + 1` stays larger than
    /// anything a 32-bit host holds rather than becoming `1` or malformed.
    #[test]
    fn saturating_narrow_to_32_bits_keeps_a_large_count_large() {
        use crate::types::fraction::saturating_narrow;
        assert_eq!(saturating_narrow::<u32>(7), 7);
        assert_eq!(saturating_narrow::<u32>(u32::MAX as u64), u32::MAX);
        assert_eq!(saturating_narrow::<u32>(u32::MAX as u64 + 2), u32::MAX);
        assert_eq!(saturating_narrow::<u32>(u64::MAX), u32::MAX);
        assert_eq!(
            saturating_narrow::<usize>(u32::MAX as u64 + 2),
            usize::try_from(u32::MAX as u64 + 2).unwrap_or(usize::MAX)
        );
    }
}
