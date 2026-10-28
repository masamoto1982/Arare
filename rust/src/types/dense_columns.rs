//! Promoting a Vector of numeric values to a dense Tensor straight into the
//! tensor's two columns.
//!
//! `Value::from_vector_promoted` used to read every lane into a
//! `Vec<Fraction>` (`value_absence::try_collect_dense`) and then split that
//! list into numerator and denominator columns again
//! (`DenseTensor::from_fractions_with_absences`). A `MAP` whose block
//! answers a Vector promotes its whole result that way, so each lane of
//! every row went through a `Fraction` it never needed. Here the lanes are
//! written into the columns directly, and a child Tensor's columns are
//! copied as they stand.
//!
//! The result is the tensor the two-step route builds, lane for lane: a
//! scalar's pair as it is held, a point over zero as its reduced pair like
//! any other, the shape from the same rectangular walk, and purity
//! recomputed over every lane. Whatever that route would not have stored
//! densely — a lane wider than a machine word, a NIL (which is not a
//! number), a Boolean, a String, an irrational, a Record, a ragged or empty
//! Vector — answers `None` here, and the caller keeps the nested form; the
//! two-step route would have declined it too, which is why the caller no
//! longer retries through it. `dense_columns_tests` holds the two equal.

use std::sync::Arc;

use super::tensor_storage::{Column, DenseTensor};
use super::{Value, ValueData};
use crate::types::fraction::{Fraction, FractionRepr};

/// A shape on the way up the walk; four dimensions fit without allocating.
type Shape = smallvec::SmallVec<[usize; 4]>;

/// Plain `Vec`s while the walk appends: a child Tensor's columns are copied
/// in by `extend_from_slice`, which a `Vec` does as one copy and a `SmallVec`
/// as an insertion; the finished columns move into the tensor uncopied.
struct Columns {
    nums: Vec<i64>,
    dens: Vec<i64>,
}

/// The plain scalars a per-element walk answers, written into the two columns
/// as they arrive, so that a walk whose every answer is one never holds them as
/// `Value`s at all (`MAP`'s unfused loop).
///
/// A scalar is taken only when it is held as a machine-word pair and carries
/// no absence; anything else is declined, and the
/// caller turns what has been taken back into the `Value`s it came from
/// (`into_values`) and carries on with a list. `finish` is the Tensor
/// `Value::from_vector_promoted` builds from that list.
pub(crate) struct ScalarColumns {
    nums: Column,
    dens: Column,
}

impl ScalarColumns {
    pub(crate) fn with_capacity(lanes: usize) -> Self {
        Self {
            nums: Column::with_capacity(lanes),
            dens: Column::with_capacity(lanes),
        }
    }

    /// Take `value` as the next lane, or answer `false` having taken nothing.
    #[inline]
    pub(crate) fn push(&mut self, value: &Value) -> bool {
        match &value.data {
            ValueData::Scalar(Fraction {
                repr: FractionRepr::Small(n, d),
            }) => {
                self.nums.push(*n);
                self.dens.push(*d);
                true
            }
            _ => false,
        }
    }

    /// The values taken so far, as the walk held them.
    pub(crate) fn into_values(self, capacity: usize) -> Vec<Value> {
        let mut values = Vec::with_capacity(capacity.max(self.nums.len()));
        values.extend(
            self.nums.iter().zip(&self.dens).map(|(&n, &d)| {
                Value::from_fraction(Fraction::from_repr(FractionRepr::Small(n, d)))
            }),
        );
        values
    }

    /// The one-dimensional Tensor of the lanes taken, which is what promoting
    /// them as a list builds.
    pub(crate) fn finish(self) -> Value {
        let shape = vec![self.nums.len()];
        let is_pure_integer = self.dens.iter().all(|&d| d == 1);
        let tensor =
            DenseTensor::from_columns(self.nums, self.dens, shape.clone(), is_pure_integer);
        Value::new(
            ValueData::Tensor {
                data: Arc::new(tensor),
                shape: Arc::new(shape),
            },
            None,
        )
    }
}

/// `values` promoted to a dense Tensor, or `None` for the two-step route.
pub(super) fn try_promote_columns(values: &[Value]) -> Option<Value> {
    let mut columns = Columns {
        nums: Vec::with_capacity(values.len()),
        dens: Vec::with_capacity(values.len()),
    };
    let shape = append_values(values, &mut columns)?.into_vec();
    if shape.iter().product::<usize>() != columns.nums.len() {
        return None;
    }
    let is_pure_integer = columns.dens.iter().all(|&d| d == 1);
    let tensor = DenseTensor::from_columns(
        Column::from_vec(columns.nums),
        Column::from_vec(columns.dens),
        shape.clone(),
        is_pure_integer,
    );
    Some(Value::new(
        ValueData::Tensor {
            data: Arc::new(tensor),
            shape: Arc::new(shape),
        },
        None,
    ))
}

fn append_values(values: &[Value], columns: &mut Columns) -> Option<Shape> {
    if values.is_empty() {
        return None;
    }
    let mut inner_shape: Option<Shape> = None;
    for value in values {
        let child_shape = append_value(value, columns)?;
        match &inner_shape {
            None => inner_shape = Some(child_shape),
            Some(expected) if *expected == child_shape => {}
            Some(_) => return None,
        }
    }
    let mut shape = Shape::new();
    shape.push(values.len());
    shape.extend(inner_shape?);
    Some(shape)
}

fn append_value(value: &Value, columns: &mut Columns) -> Option<Shape> {
    match &value.data {
        ValueData::Truths(t) => append_value(&t.to_vector(), columns),
        ValueData::Scalar(fraction) => {
            let (n, d) = fraction.extract_i64_pair()?;
            columns.nums.push(n);
            columns.dens.push(d);
            Some(Shape::new())
        }
        ValueData::Tensor {
            data: tensor,
            shape,
        } => {
            columns.nums.extend_from_slice(&tensor.numerators);
            columns.dens.extend_from_slice(&tensor.denominators);
            Some(Shape::from_slice(shape))
        }
        ValueData::Vector(children) => append_values(children, columns),
        // A NIL is not a number, so a Vector holding one keeps its nested
        // form, as it does for a String or a Boolean.
        ValueData::Nil(_)
        | ValueData::ExactScalar(_)
        | ValueData::Record(_)
        | ValueData::Boolean(_)
        | ValueData::Text(_)
        | ValueData::Symbol(_) => None,
    }
}
