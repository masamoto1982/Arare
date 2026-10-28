//! Test suite for `crate::types::tensor_storage` — the dense numeric
//! tensor store.
//!
//! Lifted out of an inline `#[cfg(test)] mod` so the module it tests stays
//! inside the 500-line budget (docs/dev/specification-implementation-rules.md),
//! and so it sits where every other test file in `types/` already does.

use super::fraction::Fraction;
use super::tensor_storage::DenseTensor;

// ── reversed_lanes ──────────────────────────────────────────────────────────

/// A flat tensor whose lane `at` is `point`, and whose other lanes hold
/// their index.
fn with_point_at(len: usize, at: usize, point: Fraction) -> DenseTensor {
    let (pn, pd) = point.extract_i64_pair().expect("a point is a small pair");
    let numerators: Vec<i64> = (0..len)
        .map(|i| if i == at { pn } else { i as i64 })
        .collect();
    let denominators: Vec<i64> = (0..len).map(|i| if i == at { pd } else { 1 }).collect();
    DenseTensor::from_columns(numerators, denominators, vec![len], false)
}

#[test]
fn reversed_lanes_reverses_both_columns() {
    let tensor = DenseTensor::from_integers(vec![1, 2, 3, 4, 5]);
    let reversed = tensor.reversed_lanes();
    assert_eq!(reversed.numerators.as_slice(), [5, 4, 3, 2, 1]);
    assert_eq!(reversed.denominators.as_slice(), [1, 1, 1, 1, 1]);
    assert_eq!(reversed.shape.as_slice(), [5]);
    assert!(reversed.is_pure_integer);
}

#[test]
fn reversed_lanes_carries_a_point_over_zero_to_its_new_index() {
    // `1/0` at lane 1 of 8 belongs at lane 6 afterwards (len - 1 - index).
    let tensor = with_point_at(8, 1, Fraction::infinity());
    let reversed = tensor.reversed_lanes();
    assert_eq!(reversed.fraction_at(6), Fraction::infinity());
    assert_eq!(reversed.fraction_at(1), Fraction::from(6));
    assert!(!reversed.all_finite());
}

#[test]
fn reversed_lanes_is_its_own_inverse() {
    for tensor in [
        DenseTensor::from_integers(vec![7, -3, 0, 11]),
        with_point_at(6, 0, Fraction::nullity()),
        with_point_at(6, 5, Fraction::infinity()),
        with_point_at(1, 0, Fraction::infinity()),
    ] {
        assert_eq!(
            tensor.reversed_lanes().reversed_lanes(),
            tensor,
            "reversing twice must restore the tensor"
        );
    }
}

#[test]
fn reversed_lanes_keeps_a_rational_tensor_rational() {
    // Halves, so `is_pure_integer` is false and must stay false: the flag
    // describes the lanes, and reversing does not change what they are.
    let tensor = DenseTensor::from_fractions(
        vec![
            Fraction::new(1.into(), 2.into()),
            Fraction::new(3.into(), 2.into()),
        ],
        vec![2],
    )
    .expect("halves build a dense tensor");
    let reversed = tensor.reversed_lanes();
    assert!(!reversed.is_pure_integer);
    assert_eq!(reversed.numerators.as_slice(), [3, 1]);
    assert_eq!(reversed.denominators.as_slice(), [2, 2]);
}

// ── reading a lane reads the columns rather than re-deriving them ──────────
//
// `fraction_at` reads the stored `i64` pair as the normal form it was stored
// in. These pin that the cheaper read is the *same* read: the answer for every
// lane must be the one `Fraction::new` would have given.

/// The oracle is the normalizing constructor.
#[test]
fn a_lane_reads_as_the_fraction_its_columns_denote() {
    // Dense over a range that crosses zero and both signs, plus the extremes
    // where the old i128 widening existed specifically to stay total.
    let mut lanes: Vec<i64> = (-500..=500).collect();
    lanes.extend([i64::MAX, i64::MIN + 1, i64::MIN]);
    let tensor = DenseTensor::from_fractions(
        lanes.iter().copied().map(Fraction::from).collect(),
        vec![lanes.len()],
    )
    .expect("integer lanes build");

    for (index, lane) in lanes.iter().enumerate() {
        let read = tensor.fraction_at(index);
        let oracle = Fraction::new((*lane).into(), 1.into());
        assert_eq!(read, oracle, "lane {index} holding {lane}");
        assert_eq!(
            format!("{read}"),
            format!("{oracle}"),
            "lane {index} renders"
        );
    }
}

/// Rationals too: the lanes a division leaves behind are where a denominator
/// other than 1 comes from, and where a gcd would have had something to do had
/// the value not already been reduced when it was stored.
#[test]
fn a_rational_lane_reads_as_the_fraction_its_columns_denote() {
    let fractions: Vec<Fraction> = (1..=60)
        .flat_map(|d| (-3..=3).map(move |n| Fraction::new(n.into(), d.into())))
        .collect();
    let tensor =
        DenseTensor::from_fractions(fractions.clone(), vec![fractions.len()]).expect("lanes build");

    for (index, expected) in fractions.iter().enumerate() {
        let read = tensor.fraction_at(index);
        assert_eq!(&read, expected, "lane {index}");
    }
}

/// A lane over zero reads as the point it is, and an untrusted column reduces
/// a pair over zero to its point like any other pair: `-7/0` is `1/0`.
#[test]
fn a_lane_over_zero_reads_as_its_point() {
    let tensor = DenseTensor::from_fractions(
        vec![Fraction::from(1), Fraction::nullity(), Fraction::infinity()],
        vec![3],
    )
    .expect("lanes build");
    assert_eq!(tensor.fraction_at(0), Fraction::from(1));
    assert_eq!(tensor.fraction_at(1), Fraction::nullity());
    assert_eq!(tensor.fraction_at(2), Fraction::infinity());

    let untrusted =
        DenseTensor::from_untrusted_columns(vec![100, 4, -7], vec![0, 2, 0], vec![3], true);
    assert_eq!(untrusted.numerators.as_slice(), [1, 2, 1]);
    assert_eq!(untrusted.denominators.as_slice(), [0, 1, 0]);
    assert!(!untrusted.is_pure_integer);
}
