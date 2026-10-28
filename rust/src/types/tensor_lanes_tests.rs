//! A row of a dense tensor (`Value::child`, `DenseTensor::lanes`) against
//! the row built lane by lane, as it was: every lane read as a `Fraction`
//! and the whole packed again by `Value::from_tensor`. The two must be the
//! same value down to its columns and its purity flag.

use proptest::prelude::*;

use super::fraction::Fraction;
use super::{DenseTensor, Value, ValueData};

fn reference_row(data: &DenseTensor, shape: &[usize], index: usize) -> Value {
    let rest: Vec<usize> = shape[1..].to_vec();
    let stride: usize = rest.iter().product();
    let start = index * stride;
    let slice: Vec<Fraction> = (start..start + stride)
        .map(|lane| data.fraction_at(lane))
        .collect();
    Value::from_tensor(slice, rest)
}

/// A lane: a small rational, or one of the two points over zero.
fn lane() -> impl Strategy<Value = Fraction> {
    prop_oneof![
        6 => (-50i64..50, 1i64..4).prop_map(|(n, d)| Fraction::new(n.into(), d.into())),
        1 => (-1i64..=1).prop_map(|sign| Fraction::new(sign.into(), 0.into())),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn a_row_is_the_lanes_it_was_read_from(
        dims in prop::collection::vec(1usize..4, 2..4),
        lanes in prop::collection::vec(lane(), 64),
    ) {
        let len: usize = dims.iter().product();
        let fractions: Vec<Fraction> = lanes.iter().cycle().take(len).cloned().collect();
        let tensor = DenseTensor::from_fractions(fractions, dims.clone())
            .expect("small lanes pack");
        let value = Value::new(
            ValueData::Tensor {
                data: std::sync::Arc::new(tensor.clone()),
                shape: std::sync::Arc::new(dims.clone()),
            },
            None,
        );
        for index in 0..dims[0] {
            let row = value.child(index).expect("a row");
            let reference = reference_row(&tensor, &dims, index);
            prop_assert_eq!(format!("{row:?}"), format!("{reference:?}"));
            prop_assert_eq!(row.nesting(), reference.nesting());
        }
    }
}
