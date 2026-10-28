//! The one-lane Tensors a lane walk (`FusedBlock::lane_flow`) answers with:
//! the walk computes on each lane alone, and what it hands back is rebuilt
//! here in the form the interpreted walk holds it.

use crate::interpreter::fused_block::{FusedWalk, Plain};
use crate::types::fraction::Fraction;
use crate::types::{Value, ValueData};

/// The one-lane value the interpreted walk holds for `lane`: a one-lane
/// Tensor while both halves fit a machine word, as the column kernel and the
/// scalar fast path build it (`DenseTensor::from_fractions`) — the lane's pair
/// as its two columns, pure when its denominator is 1, no absences — and, once
/// either half outgrows one, the lift's Vector of that one scalar. The form is
/// a function of the value alone: a lane that grows past a word and shrinks
/// back is a Tensor again.
fn one_lane(lane: Fraction) -> Value {
    let Some((n, d)) = lane.extract_i64_pair() else {
        return Value::from_vector(vec![Value::from_fraction(lane)]);
    };
    use crate::types::Column;
    let data = crate::types::DenseTensor::from_columns(
        Column::from_slice(&[n]),
        Column::from_slice(&[d]),
        [1],
        d == 1,
    );
    Value::new(
        ValueData::Tensor {
            data: std::sync::Arc::new(data),
            shape: ONE_LANE_SHAPE.with(std::sync::Arc::clone),
        },
        None,
    )
}

thread_local! {
    /// The shape every one-lane Tensor `one_lane` builds carries, allocated
    /// once: a nested `FOLD` answers one per row, and a shape is read by its
    /// content wherever it is read, so sharing one is unobservable.
    static ONE_LANE_SHAPE: std::sync::Arc<Vec<usize>> = std::sync::Arc::new(vec![1]);
}

/// The answer of a lane walk, as the interpreted walk gives it: `FOLD`'s
/// last accumulator a one-lane Tensor; `SCAN`'s every accumulator one, in a
/// Vector built as `higher_order_fold` builds it; `MAP`'s every result one,
/// promoted as `op_map` promotes a list of answers that are not all scalars.
pub(crate) fn rewrap_one_lane(walk: FusedWalk, value: &Value) -> Option<Value> {
    let lane = |v: &Value| match Plain::of(v)? {
        Plain::Num(f) => Some(one_lane(f)),
        Plain::Bool(_) => None,
    };
    match walk {
        FusedWalk::Fold => lane(value),
        FusedWalk::Scan => {
            let lanes = (0..value.len())
                .map(|i| lane(&value.child(i)?))
                .collect::<Option<Vec<_>>>()?;
            Some(Value::from_vector(lanes))
        }
        FusedWalk::Map => {
            // Every result a lane of two machine words, as the tiers hold
            // them: `from_vector_promoted` stacks such one-lane Tensors into
            // their columns under the shape `[n, 1]`, so those columns are
            // the tiers' own.
            if let ValueData::Tensor { data, shape } = &value.data {
                if shape.len() == 1 {
                    let dims = vec![shape[0], 1];
                    let integer = data.denominators.iter().all(|&d| d == 1);
                    let tensor = crate::types::DenseTensor::from_columns(
                        data.numerators.clone(),
                        data.denominators.clone(),
                        dims.clone(),
                        integer,
                    );
                    return Some(Value::new(
                        ValueData::Tensor {
                            data: std::sync::Arc::new(tensor),
                            shape: std::sync::Arc::new(dims),
                        },
                        None,
                    ));
                }
            }
            let lanes = (0..value.len())
                .map(|i| lane(&value.child(i)?))
                .collect::<Option<Vec<_>>>()?;
            Some(Value::from_vector_promoted(lanes))
        }
        FusedWalk::Filter => None,
    }
}
