//! Sequence and child-access behavior for [`Value`].
//!
//! Invariant: mutation hydrates dense tensors before exposing child storage, so
//! callers observe one sequence API regardless of the internal representation.

use super::fraction::Fraction;
use super::value_tensor::{tensor_child, tensor_child_sharing, tensor_to_nested_values};
use super::{Value, ValueData};
use std::sync::Arc;

impl Value {
    /// Children `start..end` of a Vector or dense Tensor, as one Vector — the
    /// answer `TAKE` and `DROP` give. The caller has already clamped the range
    /// to `0..=self.len()`.
    ///
    /// A dense Tensor is cut as columns: the rows in the range are one run of
    /// lanes, so the slice is a copy of that run (`DenseTensor::lanes`) and
    /// nothing outside it is touched. The nested route materialized every lane
    /// of the operand as a boxed `Value` first and then copied the slice out of
    /// *that*, so `10 TAKE` of a million-lane range cost what boxing a million
    /// numbers costs — 39 ms — and answered an AoS `Vector`, which threw the
    /// dense representation away for every Word downstream.
    ///
    /// The two answers are the same value: a Tensor equals the nested Vector
    /// holding its lanes, absent lanes' reasons included (`tensor_eq_vector`).
    /// An empty slice is the empty Vector either way.
    pub fn children_range(&self, start: usize, end: usize) -> Value {
        if start >= end {
            return Self::from_vector(Vec::new());
        }
        match &self.data {
            ValueData::Tensor { data, shape } if !shape.is_empty() => {
                let stride: usize = shape[1..].iter().product();
                let mut sliced_shape = shape.to_vec();
                sliced_shape[0] = end - start;
                let lanes = data.lanes(start * stride, (end - start) * stride, &sliced_shape);
                Self::from_dense_tensor(lanes, sliced_shape)
            }
            ValueData::Vector(children) => Self::from_vector(children[start..end].to_vec()),
            _ => Self::from_vector(
                self.as_vector_view()
                    .map(|children| children[start..end].to_vec())
                    .unwrap_or_default(),
            ),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        match &self.data {
            ValueData::Nil(_) => 0,
            // A String is one value, not a sequence of characters. Its
            // character count is reached through `CHARS`, which is what makes
            // the Vector domain explicit; LENGTH raises `nonVector` on it.
            ValueData::Boolean(_) | ValueData::Text(_) | ValueData::Symbol(_) => 1,
            ValueData::Scalar(_) | ValueData::ExactScalar(_) => 1,
            // A Record's extent is its number of keys — what a cost meter
            // scans — though it is not a sequence: `get_child` answers `None`.
            ValueData::Record(record) => record.len(),
            ValueData::Vector(v) => v.len(),
            ValueData::Truths(t) => t.len(),
            ValueData::Tensor { data, shape } => {
                if shape.is_empty() {
                    data.len()
                } else {
                    shape[0]
                }
            }
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Representation-agnostic child accessor. Works for both `Vector` and
    /// `Tensor` payloads by materializing a sub-Value (Scalar leaf or
    /// sub-Tensor) when the receiver is dense. Cloning is cheap because
    /// inner buffers are reference-counted.
    pub fn child(&self, index: usize) -> Option<Value> {
        match &self.data {
            ValueData::Vector(v) => v.get(index).cloned(),
            ValueData::Scalar(_) | ValueData::ExactScalar(_) if index == 0 => Some(self.clone()),
            ValueData::Tensor { data, shape } => tensor_child(data, shape, index),
            ValueData::Truths(t) => (index < t.len()).then(|| t.lane(index)),
            ValueData::Boolean(_)
            | ValueData::Text(_)
            | ValueData::Scalar(_)
            | ValueData::ExactScalar(_)
            | ValueData::Nil(_)
            | ValueData::Symbol(_)
            | ValueData::Record(_) => None,
        }
    }

    /// A reader of this value's children by index, each as `child` answers
    /// it, for a walk that reads many (`Children`).
    #[inline]
    pub fn children_reader(&self) -> Children<'_> {
        Children {
            value: self,
            row_shape: None,
        }
    }

    /// Convert a `ValueData::Tensor` in-place to a nested `ValueData::Vector`
    /// so that mutating helpers (push/pop/insert/remove/replace) can operate
    /// on a uniform `Vec<Value>` representation.
    fn hydrate_tensor_to_vector(&mut self) {
        let children = match &self.data {
            ValueData::Tensor { data, shape } => tensor_to_nested_values(data, shape),
            ValueData::Truths(t) => t.to_values(),
            _ => return,
        };
        self.data = ValueData::Vector(Arc::new(children));
    }

    pub fn push_child(&mut self, child: Value) {
        let child = match &child.data {
            ValueData::Truths(column) => column.to_vector(),
            _ => child,
        };
        if matches!(self.data, ValueData::Tensor { .. } | ValueData::Truths(_)) {
            self.hydrate_tensor_to_vector();
        }
        match &mut self.data {
            ValueData::Vector(v) => {
                // One more child: the nesting can only grow to hold it, so it
                // is updated from that child alone rather than rescanned.
                let with_child = child.nesting().saturating_add(1);
                Arc::make_mut(v).push(child);
                self.raise_nesting_to(with_child);
                return;
            }
            ValueData::Nil(_) => {
                self.data = ValueData::Vector(Arc::new(vec![child]));
            }
            ValueData::Scalar(f) => {
                let old = Value::from_fraction(f.clone());
                self.data = ValueData::Vector(Arc::new(vec![old, child]));
            }
            ValueData::ExactScalar(_) => {
                // Cannot push_child into an ExactScalar — silently ignore
                // (ExactScalar is always a scalar leaf, never mutated into a vector).
            }
            ValueData::Boolean(_)
            | ValueData::Text(_)
            | ValueData::Tensor { .. }
            | ValueData::Truths(_)
            | ValueData::Symbol(_)
            | ValueData::Record(_) => {}
        }
        self.refresh_nesting();
    }

    #[inline]
    pub fn as_scalar(&self) -> Option<&Fraction> {
        match &self.data {
            ValueData::Scalar(f) => Some(f),
            ValueData::Boolean(_)
            | ValueData::Text(_)
            | ValueData::ExactScalar(_)
            | ValueData::Vector(_)
            | ValueData::Tensor { .. }
            | ValueData::Truths(_)
            | ValueData::Nil(_)
            | ValueData::Symbol(_)
            | ValueData::Record(_) => None,
        }
    }

    #[inline]
    pub fn as_i64(&self) -> Option<i64> {
        self.as_scalar().and_then(|f| f.to_i64())
    }

    #[inline]
    pub fn as_usize(&self) -> Option<usize> {
        self.as_scalar().and_then(|f| f.as_usize())
    }

    #[inline]
    pub fn as_vector(&self) -> Option<&Vec<Value>> {
        match &self.data {
            ValueData::Vector(v) => Some(v),
            ValueData::Tensor { .. } | ValueData::Truths(_) => None,
            ValueData::Boolean(_)
            | ValueData::Text(_)
            | ValueData::Scalar(_)
            | ValueData::ExactScalar(_)
            | ValueData::Nil(_)
            | ValueData::Symbol(_)
            | ValueData::Record(_) => None,
        }
    }

    pub fn collect_fractions_flat(&self) -> Vec<Fraction> {
        let mut buf = Vec::new();
        self.collect_fractions_flat_into(&mut buf);
        buf
    }

    pub fn collect_fractions_flat_into(&self, buf: &mut Vec<Fraction>) {
        match &self.data {
            // A NIL is not a number and has no lane; the flat route — which
            // never meets one, being steered off by `contains_absent_lane` —
            // sees a short buffer.
            ValueData::Nil(_) => {}
            ValueData::Scalar(f) => buf.push(f.clone()),
            ValueData::ExactScalar(er) => {
                // Use best rational approximation for ExactScalar in flat collection
                if let Some(f) = er.as_rational() {
                    buf.push(f.clone());
                }
                // non-rational ExactScalars are not representable as a single Fraction
            }
            ValueData::Vector(v) => {
                for child in v.iter() {
                    child.collect_fractions_flat_into(buf);
                }
            }
            ValueData::Tensor { data, .. } => {
                buf.extend(data.iter());
            }
            // A Boolean/Text/Symbol flattens to no fraction lane. Kept in
            // lock-step with `count_fractions` below so buffer sizing stays
            // exact.
            ValueData::Boolean(_)
            | ValueData::Truths(_)
            | ValueData::Text(_)
            | ValueData::Symbol(_)
            | ValueData::Record(_) => {}
        }
    }

    pub fn count_fractions(&self) -> usize {
        match &self.data {
            ValueData::Nil(_) => 1,
            ValueData::Scalar(_) | ValueData::ExactScalar(_) => 1,
            ValueData::Vector(v) => v.iter().map(|c| c.count_fractions()).sum(),
            ValueData::Tensor { data, .. } => data.len(),
            // A truth column's UNKNOWN lanes are NILs, one lane each.
            ValueData::Truths(t) => t
                .codes()
                .iter()
                .filter(|&&c| c == super::truth_column::UNKNOWN)
                .count(),
            // U takes the `Nil` arm above and contributes one fraction lane
            // (see `collect_fractions_flat_into`); a Boolean/Text/Symbol
            // contributes none.
            ValueData::Boolean(_) | ValueData::Text(_) | ValueData::Symbol(_) => 0,
            ValueData::Record(_) => 0,
        }
    }

    pub fn shape(&self) -> Vec<usize> {
        match &self.data {
            // U and NIL are both rank-0 (empty shape), like a Boolean/Scalar.
            ValueData::Nil(_) => vec![],
            ValueData::Scalar(_) | ValueData::ExactScalar(_) => vec![],
            ValueData::Vector(v) => {
                if v.is_empty() {
                    vec![0]
                } else {
                    let first_shape: Vec<usize> = v[0].shape();
                    let all_same: bool = v.iter().skip(1).all(|c| c.shape() == first_shape);
                    if all_same && !first_shape.is_empty() {
                        let mut shape = vec![v.len()];
                        shape.extend(first_shape);
                        shape
                    } else {
                        vec![v.len()]
                    }
                }
            }
            ValueData::Tensor { shape, .. } => (**shape).clone(),
            ValueData::Truths(t) => vec![t.len()],
            ValueData::Boolean(_)
            | ValueData::Text(_)
            | ValueData::Symbol(_)
            | ValueData::Record(_) => vec![],
        }
    }
}

/// `Value::child`, for a walk that reads child after child of one value: the
/// rows of a dense Tensor of rank two or more share one shape allocation
/// instead of each building its own (`tensor_child_sharing`).
pub struct Children<'a> {
    value: &'a Value,
    row_shape: Option<Arc<Vec<usize>>>,
}

impl Children<'_> {
    /// The child at `index`, exactly as `Value::child` answers it.
    #[inline]
    pub fn get(&mut self, index: usize) -> Option<Value> {
        match &self.value.data {
            ValueData::Tensor { data, shape } => {
                tensor_child_sharing(data, shape, index, &mut self.row_shape)
            }
            _ => self.value.child(index),
        }
    }
}
