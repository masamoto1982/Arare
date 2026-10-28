//! `Value::from_vector_promoted` (which tries `dense_columns` first) against
//! the two-step route it replaces on the dense path: every lane read into a
//! `Vec<Fraction>`, then packed by `DenseTensor::from_fractions`, with the
//! nested Vector as the answer when either step declines. The two must agree
//! on every value down to its columns, purity flag, shape and nesting.

use std::sync::Arc;

use proptest::prelude::*;

use super::value_absence::try_collect_dense;
use super::{DenseTensor, Value, ValueData};
use crate::error::NilReason;

fn reference_promoted(values: Vec<Value>) -> Value {
    if let Some(collected) = try_collect_dense(&values) {
        if let Some(tensor) = DenseTensor::from_fractions(collected.data, collected.shape.clone()) {
            return Value::new(
                ValueData::Tensor {
                    data: Arc::new(tensor),
                    shape: Arc::new(collected.shape),
                },
                None,
            );
        }
    }
    Value::from_vector(values)
}

fn leaf() -> impl Strategy<Value = Value> {
    prop_oneof![
        8 => (-40i64..40, 1i64..5)
            .prop_map(|(n, d)| Value::from_fraction(super::fraction::Fraction::new(n.into(), d.into()))),
        1 => Just(Value::from_int(i64::MAX)),
        // Wider than a machine word.
        1 => Just(Value::from_fraction(super::fraction::Fraction::new(
            num_bigint::BigInt::from(i64::MAX) * 4,
            1.into(),
        ))),
        // The two points over zero, which are lanes like any other.
        1 => (-1i64..=1).prop_map(|sign| Value::from_fraction(
            super::fraction::Fraction::new(sign.into(), 0.into()),
        )),
        1 => prop_oneof![
            Just(NilReason::DomainMiss),
            Just(NilReason::IndexOutOfBounds),
            Just(NilReason::NotFound),
        ]
        .prop_map(Value::nil_with_reason),
        1 => Just(Value::nil()),
        1 => Just(Value::from_bool(true)),
        1 => Just(Value::from_string("s")),
    ]
}

/// A value as a `MAP` row might be: a leaf, a promoted row of leaves (often
/// a dense Tensor), or a Vector of those.
fn element() -> impl Strategy<Value = Value> {
    let row = || prop::collection::vec(leaf(), 0..4).prop_map(Value::from_vector_promoted);
    let nested = prop::collection::vec(row(), 0..3).prop_map(Value::from_vector);
    prop_oneof![4 => leaf(), 4 => row(), 1 => nested]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn promotion_agrees_with_the_two_step_route(
        values in prop::collection::vec(element(), 0..6),
        uniform in any::<bool>(),
    ) {
        // Rows of one shape are the case that densifies; a random list is
        // mostly ragged, so half the cases repeat one element.
        let values = if uniform && !values.is_empty() {
            vec![values[0].clone(); values.len()]
        } else {
            values
        };
        let promoted = Value::from_vector_promoted(values.clone());
        let reference = reference_promoted(values);
        prop_assert_eq!(format!("{promoted:?}"), format!("{reference:?}"));
        prop_assert_eq!(promoted.nesting(), reference.nesting());
    }
}
