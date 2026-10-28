//! Operational absence and the truth observation for [`Value`].
//!
//! `try_collect_dense` is test-only: `dense_columns::try_promote_columns`
//! writes the same lanes straight into the tensor's columns and is the one
//! promotion route the runtime takes, while this walk remains as the
//! independently written oracle `dense_columns_tests` checks it against.

#[cfg(test)]
use super::fraction::Fraction;
use super::{Value, ValueData};
use crate::error::NilReason;
use crate::semantic::AbsenceMetadata;

impl Value {
    #[inline]
    pub fn nil() -> Self {
        Self::new(
            ValueData::Nil(crate::semantic::NilAbsence::None),
            Some(AbsenceMetadata::literal()),
        )
    }

    /// An absence, with `absence` for why.
    #[inline]
    pub fn nil_with_absence(absence: AbsenceMetadata) -> Self {
        Self::new(
            ValueData::Nil(crate::semantic::NilAbsence::None),
            Some(absence),
        )
    }

    /// The observable `truthValue` axis (LANG.VALUES.TRUTH,
    /// LANG.OBSERVATION.PROTOCOL): `Some("true")` or `Some("false")` for a
    /// Boolean, `None` for every other value. It is read off the value's
    /// domain alone. UNKNOWN is a NIL (LANG.VALUES.TRUTH) and is observed as
    /// one, with its reason, so it has no entry on this axis.
    pub fn truth_value(&self) -> Option<&'static str> {
        match &self.data {
            ValueData::Boolean(b) => Some(if *b { "true" } else { "false" }),
            _ => None,
        }
    }

    /// The absent `source` carried whole — its reason and the rest of its
    /// absence — for a Word that passes it through
    /// (LANG.FAILURE.PASSTHROUGH). A present `source` gives a written NIL.
    #[inline]
    pub fn nil_inheriting_absence_from(source: &Self) -> Self {
        match source.normalized_absence_metadata() {
            Some(absence) => Self::nil_with_absence(absence),
            None => Self::nil(),
        }
    }

    /// A reasoned NIL for the NIL Projection Rule (LANG.FAILURE.PROJECT): a
    /// well-formed operation that cannot produce a value answers its reason.
    #[inline]
    pub fn nil_with_reason(reason: NilReason) -> Self {
        Self::nil_with_absence(AbsenceMetadata::with_reason(reason))
    }

    /// The NIL `ABSENT` produces: reason `userDeclared`, carrying `detail`.
    #[inline]
    pub fn nil_user_declared(detail: &str) -> Self {
        Self::nil_with_absence(AbsenceMetadata::user_declared(detail))
    }

    #[inline]
    pub fn absence_metadata(&self) -> Option<&AbsenceMetadata> {
        match &self.data {
            ValueData::Nil(absence) => absence.metadata(),
            _ => None,
        }
    }

    /// The text a `userDeclared` absence carries beside its reason.
    #[inline]
    pub fn absence_detail(&self) -> Option<&str> {
        self.absence_metadata()
            .and_then(AbsenceMetadata::detail_text)
    }

    #[inline]
    pub fn normalized_absence_metadata(&self) -> Option<AbsenceMetadata> {
        if !self.is_nil() {
            return None;
        }
        Some(
            self.absence_metadata()
                .cloned()
                .unwrap_or_else(AbsenceMetadata::with_reasonless_unknown),
        )
    }

    #[inline]
    pub fn nil_reason(&self) -> Option<&NilReason> {
        self.absence_metadata()
            .and_then(|absence| absence.reason.as_ref())
    }
}

/// A rectangular numeric value flattened into the two things a dense tensor
/// stores: the lanes and their shape.
#[cfg(test)]
pub(super) struct DenseCollect {
    pub(super) data: Vec<Fraction>,
    pub(super) shape: Vec<usize>,
}

/// Walk a list of `Value`s into a [`DenseCollect`] if every leaf is a numeric
/// lane — a Fraction scalar or a child Tensor — and the shape is rectangular.
/// `None` if any leaf is outside that (NIL, Boolean, Text, Symbol,
/// ExactScalar, Record) or if shapes disagree.
#[cfg(test)]
pub(super) fn try_collect_dense(values: &[Value]) -> Option<DenseCollect> {
    let mut data = Vec::with_capacity(values.len());
    let shape = append_dense_values(values, &mut data)?;
    Some(DenseCollect { data, shape })
}

/// Append every value's lanes to `data`, and answer with the shape the whole
/// run forms.
#[cfg(test)]
fn append_dense_values(values: &[Value], data: &mut Vec<Fraction>) -> Option<Vec<usize>> {
    if values.is_empty() {
        return None;
    }
    let mut inner_shape: Option<Vec<usize>> = None;
    for value in values {
        let child_shape = append_dense_value(value, data)?;
        match &inner_shape {
            None => inner_shape = Some(child_shape),
            Some(expected) if *expected == child_shape => {}
            Some(_) => return None,
        }
    }
    let mut shape = vec![values.len()];
    shape.extend(inner_shape.expect("values is non-empty, so a shape was set"));
    Some(shape)
}

/// Append one value's lanes, answering with its own shape (empty for a leaf).
#[cfg(test)]
fn append_dense_value(value: &Value, data: &mut Vec<Fraction>) -> Option<Vec<usize>> {
    match &value.data {
        ValueData::Truths(t) => append_dense_value(&t.to_vector(), data),
        ValueData::Scalar(fraction) => {
            data.push(fraction.clone());
            Some(Vec::new())
        }
        ValueData::Tensor {
            data: tensor,
            shape,
        } => {
            data.extend(tensor.iter());
            Some((**shape).clone())
        }
        ValueData::Vector(children) => append_dense_values(children, data),
        // A NIL is not a numeric lane, and nor is anything below.
        ValueData::Nil(_)
        | ValueData::ExactScalar(_)
        | ValueData::Record(_)
        | ValueData::Boolean(_)
        | ValueData::Text(_)
        | ValueData::Symbol(_) => None,
    }
}
