//! Scalar construction and domain classification for [`Value`], and when a
//! dense tensor and a nested `Vector` are the same value.
//!
//! `Value` has two representations for one thing — a rectangular numeric
//! collection is either `ValueData::Tensor` (struct-of-arrays columns) or
//! `ValueData::Vector` (a tree of child `Value`s) — and which one a program
//! ends up holding is a storage decision no clause of the specification makes.
//! LANG.STACK.ORDER makes value identity a semantic question, so the two forms
//! must answer `EQ` and hash alike whenever they hold the same content. This
//! module is that correspondence, kept in one place because `PartialEq` and
//! `Hash` have to agree about it and drift apart the moment they are written
//! twice.
//!
//! A dense tensor holds numbers and nothing else, so a `Vector` holding a NIL
//! child never equals one: a NIL is not a number, and `Value`'s equality
//! reads its reason (`data == data && nil_reason == nil_reason`), which no
//! lane carries.

use super::fraction::{Fraction, FractionRepr};
use super::value_tensor::tensor_to_nested_values;
use super::{DenseTensor, RecordData, Value, ValueData};
use std::sync::Arc;

pub(super) fn tensor_eq_vector(data: &DenseTensor, shape: &[usize], v: &[Value]) -> bool {
    // A dense tensor is always rectangular, so a ragged nested vector (no
    // well-defined rectangular shape) can never equal one. `nested_vector_shape`
    // returns `None` for ragged structures, which fails the comparison here
    // rather than colliding with the dense shape via a count-only fallback.
    let Some(nested_shape) = nested_vector_shape(v) else {
        return false;
    };
    if nested_shape != shape {
        return false;
    }
    let mut idx = 0usize;
    nested_flatten_matches(v, data, &mut idx) && idx == data.len()
}

/// The rectangular shape of a nested vector, or `None` when the structure is
/// ragged (sibling elements with differing shapes, or mixed scalar/vector
/// siblings). Used only for dense-tensor equality, which requires a
/// rectangular counterpart.
fn nested_vector_shape(v: &[Value]) -> Option<Vec<usize>> {
    if v.is_empty() {
        return Some(vec![0]);
    }
    let first_shape = element_rect_shape(&v[0])?;
    for child in v.iter().skip(1) {
        if element_rect_shape(child)? != first_shape {
            return None;
        }
    }
    let mut s = vec![v.len()];
    s.extend(first_shape);
    Some(s)
}

/// Rectangular shape of a single value, or `None` for non-numeric leaves or
/// ragged sub-structures.
fn element_rect_shape(value: &Value) -> Option<Vec<usize>> {
    match &value.data {
        ValueData::Truths(t) => element_rect_shape(&t.to_vector()),
        ValueData::Scalar(_) | ValueData::ExactScalar(_) => Some(Vec::new()),
        // A NIL is not a numeric leaf: a Vector holding one has no dense
        // counterpart to equal.
        ValueData::Nil(_) => None,
        // A String is not a numeric leaf, so it has no rectangular element
        // shape and forces the structural (non-dense) path, like a Boolean.
        ValueData::Text(_) => None,
        ValueData::Tensor { shape, .. } => Some((**shape).clone()),
        ValueData::Vector(items) => nested_vector_shape(items),
        ValueData::Boolean(_) | ValueData::Symbol(_) | ValueData::Record(_) => None,
    }
}

fn nested_flatten_matches(v: &[Value], data: &DenseTensor, idx: &mut usize) -> bool {
    for child in v {
        match &child.data {
            ValueData::Scalar(f) => {
                if *idx >= data.len() || data.fraction_at(*idx) != *f {
                    return false;
                }
                *idx += 1;
            }
            // A NIL is not a number, so no lane equals it; an ExactScalar is
            // irrational, so no lane equals it either.
            ValueData::Nil(_) | ValueData::ExactScalar(_) => return false,
            ValueData::Vector(inner) => {
                if !nested_flatten_matches(inner, data, idx) {
                    return false;
                }
            }
            ValueData::Tensor {
                data: inner_data, ..
            } => {
                for lane in 0..inner_data.len() {
                    if *idx >= data.len() || data.fraction_at(*idx) != inner_data.fraction_at(lane)
                    {
                        return false;
                    }
                    *idx += 1;
                }
            }
            _ => return false,
        }
    }
    true
}

/// Flatten a nested `Vector` into `(shape, leaves)` the same way
/// `nested_flatten_matches` walks it against a dense tensor's lanes:
/// `Scalar` contributes its `Fraction`, `Tensor` its own dense lanes,
/// `Vector` recurses, and anything else (`Nil`, `ExactScalar`, `Boolean`,
/// `Text`, `Symbol`) fails the flatten — mirroring exactly which leaves
/// `nested_flatten_matches` is willing to match against a tensor lane. Used
/// only to make [`ValueData`]'s `Hash` agree with the `Vector`/`Tensor`
/// cross-equality in `PartialEq`: a value that *can* equal a dense tensor
/// must hash the way that tensor does.
pub(super) type DenseFlatten = (Vec<usize>, Vec<Fraction>);

pub(super) fn dense_flatten(v: &[Value]) -> Option<DenseFlatten> {
    let shape = nested_vector_shape(v)?;
    let mut leaves = Vec::new();
    if collect_dense_leaves(v, &mut leaves) {
        Some((shape, leaves))
    } else {
        None
    }
}

fn collect_dense_leaves(v: &[Value], out: &mut Vec<Fraction>) -> bool {
    for child in v {
        match &child.data {
            ValueData::Scalar(f) => out.push(f.clone()),
            ValueData::Vector(inner) => {
                if !collect_dense_leaves(inner, out) {
                    return false;
                }
            }
            ValueData::Tensor { data, .. } => out.extend(data.iter()),
            _ => return false,
        }
    }
    true
}

/// A copy, field for field. Written out rather than derived for the one case
/// the interpreter copies on nearly every read of a binding: a present
/// machine-word rational, which is copied in a few moves here, where the
/// derived copy was an out-of-line call that walked every variant of the
/// data.
impl Clone for Value {
    #[inline]
    fn clone(&self) -> Self {
        match &self.data {
            ValueData::Scalar(Fraction {
                repr: FractionRepr::Small(n, d),
            }) => Self::leaf(ValueData::Scalar(Fraction {
                repr: FractionRepr::Small(*n, *d),
            })),
            _ => self.clone_fields(),
        }
    }
}

impl Value {
    #[inline(never)]
    fn clone_fields(&self) -> Self {
        Self {
            data: self.data.clone(),
            nesting: self.nesting,
        }
    }

    /// A present value that holds no other value — a number, a truth value,
    /// a String — whose nesting is therefore 0 without asking its data. The
    /// constructors of the scalar leaves the interpreter builds on every
    /// step come here, so building one is two stores.
    #[inline]
    pub(crate) fn leaf(data: ValueData) -> Self {
        debug_assert_eq!(data.nesting(), 0, "a leaf holds no other value");
        Self { data, nesting: 0 }
    }

    #[inline]
    /// A numeric leaf from a `Fraction`: a rational, or one of the two
    /// points over zero, which is a Scalar like any other.
    pub fn from_fraction(f: Fraction) -> Self {
        Self::leaf(ValueData::Scalar(f))
    }

    #[inline]
    pub fn from_int(n: i64) -> Self {
        Self::leaf(ValueData::Scalar(Fraction::from(n)))
    }

    #[inline]
    pub fn from_bool(b: bool) -> Self {
        Self::leaf(ValueData::Boolean(b))
    }

    /// The definite truth value carried by a Boolean data value, or `None`
    /// for any non-Boolean value. This is the data-plane truth accessor:
    /// unlike [`Value::is_truthy`] it never coerces a number, vector, or
    /// other shape into a truth value. UNKNOWN is a NIL, not a Boolean
    /// (LANG.VALUES.TRUTH), so it returns `None` here.
    #[inline]
    pub fn as_truth(&self) -> Option<bool> {
        match &self.data {
            ValueData::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Build a String value (LANG.VALUES.DISJOINT).
    ///
    /// The empty String is a String. It used to become
    /// `NilReason::EmptySequence`, which made `''` an absence rather than a
    /// value and forced every text Word to carry an empty special case; a
    /// domain with no empty element also cannot be closed under `TRIM` or
    /// `TOKENIZE`. NIL means "no value here", and `''` is a perfectly good
    /// value with no characters in it.
    pub fn from_string(s: &str) -> Self {
        Self::leaf(ValueData::Text(Arc::from(s)))
    }

    /// [`Self::from_string`] sharing text already held in an `Arc`: a String
    /// literal's token pushes its own text rather than a fresh copy of it,
    /// which was an allocation and a free on every push of one.
    #[inline]
    pub(crate) fn from_text(text: &Arc<str>) -> Self {
        Self::leaf(ValueData::Text(Arc::clone(text)))
    }

    /// The characters of a String value, or `None` for any other domain.
    ///
    /// This is the whole of stringhood now: no element inspection, no
    /// codepoint-range guessing. A Vector of codepoint Scalars is a
    /// Vector, and answers `None`.
    #[inline]
    pub fn as_text(&self) -> Option<&str> {
        match &self.data {
            ValueData::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Build a Record value (LANG.RECORDS.STRUCTURE).
    pub fn from_record(record: RecordData) -> Self {
        Self::new(ValueData::Record(Arc::new(record)), None)
    }

    /// The Record behind a Record value, or `None` for any other domain.
    #[inline]
    pub fn as_record(&self) -> Option<&RecordData> {
        match &self.data {
            ValueData::Record(record) => Some(record),
            _ => None,
        }
    }

    /// A bare Word reference — data until something executes it. See
    /// `ValueData::Symbol`'s doc comment.
    pub fn from_symbol(s: &str) -> Self {
        Self::new(ValueData::Symbol(Arc::from(s)), None)
    }

    #[inline]
    pub fn from_children(children: Vec<Value>) -> Self {
        Self::new(
            ValueData::Vector(Arc::new(unpack_truth_columns(children))),
            None,
        )
    }

    /// Build a Vector value (LANG.VALUES.VECTOR).
    ///
    /// The empty Vector is a Vector. It used to become
    /// `NilReason::EmptySequence`, which made `[ ]` an absence and put NIL to
    /// work as "empty collection" — a second job that collides with
    /// LANG.VALUES.NIL, where a reason is the *whole observable content* of an
    /// absence rather than a stand-in for a value. LANG.VALUES.VECTOR calls a
    /// Vector "an ordered finite collection of values" and makes "order and
    /// length" its whole observable structure; zero is a finite length.
    pub fn from_vector(values: Vec<Value>) -> Self {
        Self::new(
            ValueData::Vector(Arc::new(unpack_truth_columns(values))),
            None,
        )
    }

    #[inline]
    pub fn from_exact_real(er: crate::types::exact::ExactReal) -> Self {
        // A rational takes the `Fraction` path, so the two tiers hold one
        // value one way.
        if let Some(f) = er.as_rational() {
            return Self::from_fraction(f.clone());
        }
        Self::new(ValueData::ExactScalar(Box::new(er)), None)
    }

    #[inline]
    pub fn from_number(f: Fraction) -> Self {
        Self::from_fraction(f)
    }

    /// NIL test: `true` only for the operational absence node
    /// ([`ValueData::Nil(_)`]). UNKNOWN is a NIL read in truth position
    /// (LANG.VALUES.TRUTH), so it is one like any other.
    #[inline]
    pub fn is_nil(&self) -> bool {
        matches!(self.data, ValueData::Nil(_))
    }

    /// The value's domain, spelled as LANG.VALUES.DISJOINT spells it. Every
    /// error message names an operand's domain through this, so a reader
    /// meets one vocabulary for the seven domains and no other.
    pub fn domain_name(&self) -> &'static str {
        match &self.data {
            ValueData::Scalar(_) | ValueData::ExactScalar(_) => "Scalar",
            ValueData::Boolean(_) => "Boolean",
            ValueData::Text(_) => "String",
            ValueData::Vector(_) | ValueData::Tensor { .. } | ValueData::Truths(_) => "Vector",
            ValueData::Record(_) => "Record",
            ValueData::Nil(_) => "NIL",
            ValueData::Symbol(_) => "Symbol",
        }
    }

    #[inline]
    pub fn is_scalar(&self) -> bool {
        matches!(self.data, ValueData::Scalar(_) | ValueData::ExactScalar(_))
    }

    #[inline]
    pub fn is_vector(&self) -> bool {
        matches!(
            self.data,
            ValueData::Vector(_) | ValueData::Tensor { .. } | ValueData::Truths(_)
        )
    }

    /// Whether this value is a String (LANG.VALUES.DISJOINT).
    #[inline]
    pub fn is_text(&self) -> bool {
        matches!(self.data, ValueData::Text(_))
    }

    /// Borrow the children of an iterable `Value` as a `Cow<[Value]>`.
    /// `Vector` and `Record` borrow their backing slice directly; `Tensor`
    /// materializes its children once into an owned `Vec<Value>`. Non-iterable
    /// kinds (`Scalar`, `Nil`, and so on) return `None`.
    ///
    /// Use this in non-hot consumers (JSON serialization, sort, structural
    /// helpers) so they only need a single iteration path regardless of
    /// whether the value is `Vector` or `Tensor`.
    pub fn as_vector_view(&self) -> Option<std::borrow::Cow<'_, [Value]>> {
        match &self.data {
            ValueData::Vector(v) => Some(std::borrow::Cow::Borrowed(v.as_slice())),
            ValueData::Tensor { data, shape } => Some(std::borrow::Cow::Owned(
                tensor_to_nested_values(data, shape),
            )),
            ValueData::Truths(t) => Some(std::borrow::Cow::Owned(t.to_values())),
            ValueData::Boolean(_)
            | ValueData::Text(_)
            | ValueData::Scalar(_)
            | ValueData::ExactScalar(_)
            | ValueData::Nil(_)
            | ValueData::Symbol(_)
            | ValueData::Record(_) => None,
        }
    }

    /// Return a `Cow<Value>` that is guaranteed to use neither dense
    /// representation. `Tensor` and `Truths` values are converted into a
    /// nested `ValueData::Vector`; every other
    /// variant is borrowed in place.
    ///
    /// Useful at user-visible boundaries (PRINT, JSON-EXPORT, GUI hand-off,
    /// error message formatting) where the caller wants to operate on a
    /// uniform `Vector` shape without caring whether the producer happened to
    /// emit a dense `Tensor`.
    pub fn ensure_hydrated(&self) -> std::borrow::Cow<'_, Value> {
        match &self.data {
            ValueData::Tensor { data, shape } => {
                let children = tensor_to_nested_values(data, shape);
                std::borrow::Cow::Owned(Value::new(ValueData::Vector(Arc::new(children)), None))
            }
            ValueData::Truths(t) => std::borrow::Cow::Owned(t.to_vector()),
            _ => std::borrow::Cow::Borrowed(self),
        }
    }

    #[inline]
    pub fn is_truthy(&self) -> bool {
        match &self.data {
            ValueData::Boolean(b) => *b,
            // `is_truthy` is a total two-valued coercion. NIL — UNKNOWN in
            // truth position — collapses to `false`. Words that must honour
            // the third value read it before asking for a definite truth
            // (`SELECT`, `AND`/`NOT`), never here.
            ValueData::Nil(_) => false,
            // A String is not a truth value. LANG.VALUES.TRUTH is two-valued
            // over Booleans, and the logic Words reject anything else outright
            // (`nonTruthValue`); this total coercion survives only for legacy
            // internal callers, so a String collapses to `false` rather than
            // inventing an emptiness rule for a domain that has none.
            ValueData::Text(_) => false,
            ValueData::Scalar(f) => !f.is_zero(),
            // An ExactScalar is an irrational, never zero.
            ValueData::ExactScalar(_) => true,
            ValueData::Vector(v) => !v.is_empty() && !v.iter().all(|c| !c.is_truthy()),
            ValueData::Truths(t) => t.to_vector().is_truthy(),
            ValueData::Tensor { data, .. } => !data.is_empty() && !data.iter().all(|f| f.is_zero()),
            ValueData::Symbol(_) => true,
            // A Record is not a truth value either; like a String it collapses
            // to `false` here and is rejected outright by the logic Words.
            ValueData::Record(_) => false,
        }
    }
}

/// A truth column is held only where a Word left it, never inside another
/// value (`types::truth_column`): every reader of a container's children then
/// meets the Vectors they are. So a container built from values unpacks any
/// column among them.
#[inline]
pub(super) fn unpack_truth_columns(mut values: Vec<Value>) -> Vec<Value> {
    for value in values.iter_mut() {
        if let ValueData::Truths(column) = &value.data {
            *value = column.to_vector();
        }
    }
    values
}
