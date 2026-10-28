//! Dense tensor construction, promotion, and representation-boundary helpers.
//!
//! Invariant: promotion is lossless and occurs only for rectangular numeric
//! values; all other vectors retain their ordinary nested representation.

use super::fraction::Fraction;
use super::{DenseTensor, Value, ValueData};
use std::sync::Arc;

impl Value {
    /// The lane at `index` of a dense tensor as a `Value`: the number its
    /// columns denote, one of the two points over zero included.
    pub fn from_dense_lane(data: &DenseTensor, index: usize) -> Value {
        Value::from_fraction(data.fraction_at(index))
    }

    /// Wrap a flat `Vec<i64>` as a 1-D pure-integer dense `Tensor` (SoA),
    /// without materializing per-element `Value`s or `Fraction`s. This is the
    /// output constructor for the integer SIMD lane: it keeps the result in
    /// the same dense column representation as its inputs instead of degrading
    /// to an AoS `Vector`.
    pub fn from_int_tensor(numerators: Vec<i64>) -> Self {
        let len = numerators.len();
        let tensor = DenseTensor::from_integers(numerators);
        Self::new(
            ValueData::Tensor {
                data: Arc::new(tensor),
                shape: Arc::new(vec![len]),
            },
            None,
        )
    }

    /// Wrap an already-assembled dense tensor, keeping its columns as they are.
    ///
    /// The constructor for a Word that *rearranges* lanes rather than
    /// recomputing them — a reversal, a permutation. Going through
    /// [`Value::from_tensor`] would mean handing it `Fraction`s rebuilt from
    /// columns the caller already holds, which is the round-trip the dense
    /// representation exists to avoid.
    pub fn from_dense_tensor(data: DenseTensor, shape: Vec<usize>) -> Self {
        let resolved_shape = if shape.is_empty() {
            vec![data.len()]
        } else {
            shape
        };
        Self::new(
            ValueData::Tensor {
                data: Arc::new(data),
                shape: Arc::new(resolved_shape),
            },
            None,
        )
    }

    /// Construct a dense `Tensor` value. `data.len()` must equal the product
    /// of `shape` (or `shape` may be empty for a flat 1-D buffer; in that
    /// case `[data.len()]` is used). A lane too wide for two machine words
    /// keeps the whole value in its nested form.
    pub fn from_tensor(data: Vec<Fraction>, shape: Vec<usize>) -> Self {
        let resolved_shape = if shape.is_empty() {
            vec![data.len()]
        } else {
            shape
        };
        let Some(tensor) = DenseTensor::from_fractions(data.clone(), resolved_shape.clone()) else {
            return Self::from_vector(tensor_fractions_to_nested_values(&data, &resolved_shape));
        };
        Self::new(
            ValueData::Tensor {
                data: Arc::new(tensor),
                shape: Arc::new(resolved_shape),
            },
            None,
        )
    }

    /// Build a Vector value, promoted to a dense `Tensor` when every leaf is
    /// a Fraction scalar and the shape is rectangular. Otherwise the nested
    /// form is preserved.
    ///
    /// `try_promote_columns` is the one promotion walk; `dense_columns_tests`
    /// holds it equal to the two-step route through `Vec<Fraction>`.
    pub fn from_vector_promoted(values: Vec<Value>) -> Self {
        match super::dense_columns::try_promote_columns(&values) {
            Some(promoted) => promoted,
            None => Self::from_vector(values),
        }
    }
}

/// Materialize the i-th child of a dense Tensor as an owned `Value`. For 1-D
/// shape `[n]` the child is a Scalar; for higher rank the child is itself a
/// dense Tensor with the trailing dimensions.
pub(super) fn tensor_child(data: &DenseTensor, shape: &[usize], index: usize) -> Option<Value> {
    tensor_child_sharing(data, shape, index, &mut None)
}

/// `tensor_child`, a row's shape taken from `row_shape` when it holds one and
/// left there when it did not. Every row of a Tensor has the same shape, so a
/// walk that reads row after row (`Children`) allocates it once; a shape is
/// compared by content wherever it is read, so sharing one is unobservable.
pub(super) fn tensor_child_sharing(
    data: &DenseTensor,
    shape: &[usize],
    index: usize,
    row_shape: &mut Option<Arc<Vec<usize>>>,
) -> Option<Value> {
    if shape.is_empty() {
        return None;
    }
    let outer = shape[0];
    if index >= outer {
        return None;
    }
    if shape.len() == 1 {
        return Some(Value::from_dense_lane(data, index));
    }
    let rest = &shape[1..];
    let stride: usize = rest.iter().product();
    let tensor = data.lanes(index * stride, stride, rest);
    let shape = row_shape.get_or_insert_with(|| Arc::new(rest.to_vec()));
    Some(Value::new(
        ValueData::Tensor {
            data: Arc::new(tensor),
            shape: Arc::clone(shape),
        },
        None,
    ))
}

/// The nested fallback for lanes too wide for `i64` columns.
fn tensor_fractions_to_nested_values(data: &[Fraction], shape: &[usize]) -> Vec<Value> {
    fn build(data: &[Fraction], shape: &[usize], offset: usize) -> Vec<Value> {
        if shape.is_empty() || shape.len() == 1 {
            let len = shape
                .first()
                .copied()
                .unwrap_or_else(|| data.len().saturating_sub(offset));
            return (offset..offset + len)
                .map(|index| Value::from_fraction(data[index].clone()))
                .collect();
        }
        let outer = shape[0];
        let rest = &shape[1..];
        let stride: usize = rest.iter().product();
        let mut out = Vec::with_capacity(outer);
        for i in 0..outer {
            out.push(Value::from_children(build(data, rest, offset + i * stride)));
        }
        out
    }
    build(data, shape, 0)
}

/// Materialize a dense Tensor (`data` + `shape`) as a tree of nested `Value`s.
/// Used by mutating helpers that need a uniform `Vec<Value>` representation,
pub(super) fn tensor_to_nested_values(data: &DenseTensor, shape: &[usize]) -> Vec<Value> {
    fn build(data: &DenseTensor, shape: &[usize], offset: usize) -> Vec<Value> {
        if shape.is_empty() || shape.len() == 1 {
            let len = shape
                .first()
                .copied()
                .unwrap_or_else(|| data.len().saturating_sub(offset));
            return (offset..offset + len)
                .map(|lane| Value::from_dense_lane(data, lane))
                .collect();
        }
        let outer = shape[0];
        let rest = &shape[1..];
        let stride: usize = rest.iter().product();
        let mut out = Vec::with_capacity(outer);
        for i in 0..outer {
            let inner = build(data, rest, offset + i * stride);
            out.push(Value::from_children(inner));
        }
        out
    }
    build(data, shape, 0)
}

#[cfg(test)]
mod tensor_boundary_tests {
    use super::*;

    #[test]
    fn tensor_and_nested_vector_compare_equal_when_flatten_matches() {
        let dense = Value::from_tensor(
            vec![
                Fraction::from(1),
                Fraction::from(2),
                Fraction::from(3),
                Fraction::from(4),
            ],
            vec![2, 2],
        );
        let nested = Value::from_children(vec![
            Value::from_children(vec![Value::from_int(1), Value::from_int(2)]),
            Value::from_children(vec![Value::from_int(3), Value::from_int(4)]),
        ]);
        assert_eq!(dense.data, nested.data);
        assert_eq!(nested.data, dense.data);
    }

    #[test]
    fn tensor_shape_matches_nested_shape() {
        let dense = Value::from_tensor(
            vec![
                Fraction::from(1),
                Fraction::from(2),
                Fraction::from(3),
                Fraction::from(4),
            ],
            vec![2, 2],
        );
        assert_eq!(dense.shape(), vec![2, 2]);
        assert_eq!(dense.count_fractions(), 4);
        assert_eq!(dense.collect_fractions_flat().len(), 4);
    }

    #[test]
    fn tensor_with_different_shape_compares_unequal_to_nested() {
        let dense = Value::from_tensor(
            vec![
                Fraction::from(1),
                Fraction::from(2),
                Fraction::from(3),
                Fraction::from(4),
            ],
            vec![4],
        );
        let nested = Value::from_children(vec![
            Value::from_children(vec![Value::from_int(1), Value::from_int(2)]),
            Value::from_children(vec![Value::from_int(3), Value::from_int(4)]),
        ]);
        assert_ne!(dense.data, nested.data);
    }

    #[test]
    fn tensor_is_vector_predicate_holds() {
        let dense = Value::from_tensor(vec![Fraction::from(1)], vec![1]);
        assert!(dense.is_vector());
        assert!(matches!(dense.data, crate::types::ValueData::Tensor { .. }));
    }

    #[test]
    fn tensor_hydrates_to_vector_on_push_child() {
        let mut dense = Value::from_tensor(vec![Fraction::from(1), Fraction::from(2)], vec![2]);
        dense.push_child(Value::from_int(3));
        assert!(matches!(dense.data, ValueData::Vector(_)));
        assert_eq!(dense.len(), 3);
    }

    #[test]
    fn dense_tensor_uses_soa_buffers() {
        let dense = Value::from_tensor(
            vec![Fraction::from(1), Fraction::new(3.into(), 2.into())],
            vec![2],
        );
        let ValueData::Tensor { data, shape } = dense.data else {
            panic!("expected DenseTensor representation");
        };
        assert_eq!(&*shape, &[2]);
        assert_eq!(data.numerators.as_slice(), [1, 3]);
        assert_eq!(data.denominators.as_slice(), [1, 2]);
        assert!(!data.is_pure_integer);
    }

    /// A lane over zero is a lane like any other: stored as its reduced pair,
    /// read back as the point it is, and not an integer lane.
    #[test]
    fn dense_tensor_holds_a_point_over_zero_as_a_lane() {
        let tensor = DenseTensor::from_fractions(
            vec![Fraction::from(1), Fraction::nullity(), Fraction::from(3)],
            vec![3],
        )
        .expect("small fractions should admit dense representation");

        assert_eq!(tensor.denominators.as_slice(), [1, 0, 1]);
        assert!(!tensor.all_finite());
        assert!(!tensor.is_pure_integer);
        assert_eq!(tensor.fraction_at(0), Fraction::from(1));
        assert_eq!(tensor.fraction_at(1), Fraction::nullity());
        assert_eq!(
            tensor.to_fractions(),
            vec![Fraction::from(1), Fraction::nullity(), Fraction::from(3)]
        );
        assert_eq!(
            Value::from_dense_lane(&tensor, 1),
            Value::from_fraction(Fraction::nullity())
        );
    }

    /// `from_exact_real` of a rational over zero is the Scalar it is.
    #[test]
    fn from_exact_real_keeps_a_point_over_zero_a_scalar() {
        use crate::types::exact::ExactReal;

        let value = Value::from_exact_real(ExactReal::Rational(Fraction::infinity()));
        assert!(value.is_scalar(), "got {:?}", value.data);
        assert_eq!(value, Value::from_fraction(Fraction::infinity()));
        assert_eq!(value.domain_name(), "Scalar");

        // A present rational still takes the Scalar fast path, and an
        // irrational still keeps its exact form.
        let three = Value::from_exact_real(ExactReal::from_integer(3));
        assert_eq!(three, Value::from_int(3));
        let sqrt2 = ExactReal::from_sqrt_rational(Fraction::from(2)).expect("√2");
        assert!(matches!(
            Value::from_exact_real(sqrt2).data,
            ValueData::ExactScalar(_)
        ));
    }

    #[test]
    fn big_fraction_tensor_falls_back_without_losing_shape() {
        use num_bigint::BigInt;

        let big = Fraction::new(BigInt::from(i128::from(i64::MAX) + 1), 1.into());
        let value = Value::from_tensor(vec![big.clone()], vec![1]);
        assert!(matches!(value.data, ValueData::Vector(_)));
        assert_eq!(value.shape(), vec![1]);
        assert_eq!(value.collect_fractions_flat(), vec![big]);
    }

    // -----------------------------------------------------------------------
    // VTU Phase III boundary helpers: as_vector_view / ensure_hydrated
    // -----------------------------------------------------------------------

    #[test]
    fn as_vector_view_borrows_for_vector_owns_for_tensor() {
        use std::borrow::Cow;

        let nested = Value::from_children(vec![Value::from_int(1), Value::from_int(2)]);
        match nested.as_vector_view() {
            Some(Cow::Borrowed(slice)) => {
                assert_eq!(slice.len(), 2);
            }
            other => panic!(
                "expected Cow::Borrowed for Vector, got {:?}",
                other.is_some()
            ),
        }

        let dense = Value::from_tensor(vec![Fraction::from(1), Fraction::from(2)], vec![2]);
        match dense.as_vector_view() {
            Some(Cow::Owned(vec)) => {
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0].as_scalar().map(|f| f.to_i64().unwrap()), Some(1));
                assert_eq!(vec[1].as_scalar().map(|f| f.to_i64().unwrap()), Some(2));
            }
            other => panic!(
                "expected Cow::Owned for Tensor, got {}",
                if other.is_some() { "Borrowed" } else { "None" }
            ),
        }
    }

    #[test]
    fn as_vector_view_returns_none_for_scalar_and_nil() {
        assert!(Value::from_int(7).as_vector_view().is_none());
        assert!(Value::nil().as_vector_view().is_none());
    }

    #[test]
    fn ensure_hydrated_borrows_non_tensor_in_place() {
        use std::borrow::Cow;

        let nested = Value::from_children(vec![Value::from_int(1)]);
        match nested.ensure_hydrated() {
            Cow::Borrowed(_) => {}
            Cow::Owned(_) => panic!("Vector should not be re-allocated"),
        }

        let scalar = Value::from_int(3);
        match scalar.ensure_hydrated() {
            Cow::Borrowed(_) => {}
            Cow::Owned(_) => panic!("Scalar should be borrowed in place"),
        }
    }

    #[test]
    fn ensure_hydrated_converts_tensor_into_equal_vector() {
        use std::borrow::Cow;

        let dense = Value::from_tensor(
            vec![Fraction::from(1), Fraction::from(2), Fraction::from(3)],
            vec![3],
        );
        let hydrated = dense.ensure_hydrated();
        match hydrated {
            Cow::Owned(v) => {
                assert!(matches!(v.data, ValueData::Vector(_)));
                assert_eq!(v, dense);
                assert_eq!(v.len(), 3);
            }
            Cow::Borrowed(_) => panic!("Tensor should hydrate into an owned Vector"),
        }
    }
}
