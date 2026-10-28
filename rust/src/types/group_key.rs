//! The key `UNIQUE`, `TALLY` and `GROUP` bundle by, and the key a Record is
//! indexed by (LANG.VALUES.ORDER, LANG.RECORDS.STRUCTURE).
//!
//! Two keys are one key when they are one value, as `EQ` decides — with the
//! one exception SQL's `DISTINCT` and `GROUP BY` make too: every NIL is the
//! same key whatever its reason, at any depth. A null a source stored and a
//! key a record lacked are one group, and one key of a Record, where `EQ`
//! tells them apart by their reasons (LANG.VALUES.NIL). The answer keeps the
//! first NIL it met, so the reason survives in it.
//!
//! Two forms of the one relation: [`GroupKey`] borrows an element for a scan
//! that keeps nothing (`UNIQUE`, `TALLY`, `GROUP`), and [`canonical_key`] owns
//! one for an index that outlives the scan (a Record's). `canonical_key`
//! replaces every NIL with one fixed NIL, so `Value`'s own equality and hash
//! over the results are this relation; `group_key_tests` holds the two forms
//! to the same answers.
//!
//! The wrapper decides once, when it is built, whether a key holds a NIL at
//! all. One that does not — every scalar, text and NIL-free container, which
//! is nearly every key — hashes and compares exactly as `Value` does, so the
//! common case pays one `match` for the exception.

use super::record::RecordData;
use super::{Value, ValueData};
use crate::error::NilReason;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};

/// A grouping key: a borrowed element, and whether a NIL lies anywhere in it.
pub(crate) struct GroupKey<'a> {
    value: &'a Value,
    holds_nil: bool,
}

impl<'a> GroupKey<'a> {
    pub(crate) fn of(value: &'a Value) -> Self {
        GroupKey {
            value,
            holds_nil: holds_nil(value),
        }
    }
}

impl PartialEq for GroupKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        if self.holds_nil != other.holds_nil {
            return false;
        }
        if !self.holds_nil {
            return self.value == other.value;
        }
        same_group(self.value, other.value)
    }
}

impl Eq for GroupKey<'_> {}

impl Hash for GroupKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.holds_nil {
            hash_group(self.value, state);
        } else {
            self.value.hash(state);
        }
    }
}

/// Whether a NIL lies anywhere in `value`. A dense Tensor holds numbers and
/// nothing else, so it never does.
pub(crate) fn holds_nil(value: &Value) -> bool {
    match &value.data {
        ValueData::Nil(_) => true,
        ValueData::Vector(children) => children.iter().any(holds_nil),
        ValueData::Truths(column) => column.unknown().is_some(),
        ValueData::Record(record) => {
            record.keys().iter().any(holds_nil) || record.values().iter().any(holds_nil)
        }
        ValueData::Boolean(_)
        | ValueData::Scalar(_)
        | ValueData::ExactScalar(_)
        | ValueData::Tensor { .. }
        | ValueData::Symbol(_)
        | ValueData::Text(_) => false,
    }
}

/// The elements of a Vector held either way: boxed, or as a truth column.
fn sequence(value: &Value) -> Option<std::borrow::Cow<'_, [Value]>> {
    match &value.data {
        ValueData::Vector(children) => Some(std::borrow::Cow::Borrowed(children.as_slice())),
        ValueData::Truths(column) => Some(std::borrow::Cow::Owned(column.to_values())),
        _ => None,
    }
}

/// `EQ`, with every NIL one value. Reached only for a key that holds a NIL;
/// a NIL-free part of it compares as `Value` does.
fn same_group(a: &Value, b: &Value) -> bool {
    if a.is_nil() || b.is_nil() {
        return a.is_nil() && b.is_nil();
    }
    if let (Some(left), Some(right)) = (sequence(a), sequence(b)) {
        return left.len() == right.len()
            && left.iter().zip(right.iter()).all(|(x, y)| same_group(x, y));
    }
    if let (ValueData::Record(left), ValueData::Record(right)) = (&a.data, &b.data) {
        let pairwise = |x: &[Value], y: &[Value]| {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_group(p, q))
        };
        return pairwise(left.keys(), right.keys()) && pairwise(left.values(), right.values());
    }
    a == b
}

const GROUP_TAG_NIL: u8 = 0xA0;
const GROUP_TAG_SEQUENCE: u8 = 0xA1;
const GROUP_TAG_RECORD: u8 = 0xA2;

/// The hash `same_group` agrees with: a NIL hashes as one tag, whatever its
/// reason, and a container holding one hashes its parts the same way. A
/// NIL-free part hashes as `Value` does, so it meets the equal part of
/// another key — a Tensor and its boxed Vector alike — in one bucket.
fn hash_group<H: Hasher>(value: &Value, state: &mut H) {
    if value.is_nil() {
        state.write_u8(GROUP_TAG_NIL);
        return;
    }
    if !holds_nil(value) {
        value.hash(state);
        return;
    }
    if let Some(children) = sequence(value) {
        state.write_u8(GROUP_TAG_SEQUENCE);
        state.write_usize(children.len());
        for child in children.iter() {
            hash_group(child, state);
        }
        return;
    }
    if let ValueData::Record(record) = &value.data {
        state.write_u8(GROUP_TAG_RECORD);
        state.write_usize(record.keys().len());
        for part in record.keys().iter().chain(record.values()) {
            hash_group(part, state);
        }
    }
}

/// `value` with every NIL in it replaced by one fixed NIL, so that two keys
/// are one key exactly when their canonical forms are one `Value`. Borrowed,
/// at the cost of one `match`, for a key that holds no NIL — nearly every key.
pub(crate) fn canonical_key(value: &Value) -> Cow<'_, Value> {
    if !holds_nil(value) {
        return Cow::Borrowed(value);
    }
    Cow::Owned(canonical_owned(value))
}

fn canonical_owned(value: &Value) -> Value {
    if value.is_nil() {
        return Value::nil_with_reason(NilReason::Literal);
    }
    if !holds_nil(value) {
        return value.clone();
    }
    if let Some(children) = sequence(value) {
        return Value::from_vector(children.iter().map(canonical_owned).collect());
    }
    if let ValueData::Record(record) = &value.data {
        let keys = record.keys().iter().map(canonical_owned).collect();
        let values = record.values().iter().map(canonical_owned).collect();
        // A Record's keys are already distinct under this relation, so their
        // canonical forms are distinct `Value`s.
        let rebuilt =
            RecordData::new(keys, values).expect("a Record's keys are distinct as grouping keys");
        return Value::from_record(rebuilt);
    }
    value.clone()
}

#[cfg(test)]
mod group_key_tests {
    //! The borrowed and the owned form of the grouping key are one relation:
    //! for every pair of keys, `GroupKey` equality is `canonical_key`
    //! equality, and equal keys hash alike in both forms.

    use super::{canonical_key, GroupKey};
    use crate::error::NilReason;
    use crate::types::record::RecordData;
    use crate::types::Value;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn hash_of(value: &impl Hash) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn keys() -> Vec<Value> {
        let nil = || Value::nil_with_reason(NilReason::Literal);
        let missing = || Value::nil_with_reason(NilReason::NotFound);
        let declared = || Value::nil_user_declared("why");
        let record = |keys: Vec<Value>, values: Vec<Value>| {
            Value::from_record(RecordData::new(keys, values).expect("distinct keys"))
        };
        vec![
            nil(),
            missing(),
            declared(),
            Value::from_int(1),
            Value::from_string("a"),
            Value::from_bool(true),
            Value::from_vector(vec![Value::from_int(1), nil()]),
            Value::from_vector(vec![Value::from_int(1), missing()]),
            Value::from_vector(vec![Value::from_int(1), Value::from_int(2)]),
            Value::from_int_tensor(vec![1, 2]),
            Value::from_vector(vec![Value::from_bool(true), nil()]),
            Value::from_vector(vec![Value::from_bool(true), declared()]),
            Value::from_vector(vec![Value::from_int_tensor(vec![1, 2]), missing()]),
            Value::from_vector(vec![
                Value::from_vector(vec![Value::from_int(1), Value::from_int(2)]),
                nil(),
            ]),
            Value::from_vector(vec![nil()]),
            Value::from_vector(vec![]),
            record(vec![Value::from_string("k")], vec![nil()]),
            record(vec![Value::from_string("k")], vec![missing()]),
            record(vec![missing()], vec![Value::from_int(1)]),
            record(vec![nil()], vec![Value::from_int(1)]),
        ]
    }

    #[test]
    fn the_two_forms_are_one_relation() {
        let keys = keys();
        for a in &keys {
            for b in &keys {
                let borrowed = GroupKey::of(a) == GroupKey::of(b);
                let owned = canonical_key(a) == canonical_key(b);
                assert_eq!(borrowed, owned, "{a:?} against {b:?}");
                if borrowed {
                    assert_eq!(
                        hash_of(&GroupKey::of(a)),
                        hash_of(&GroupKey::of(b)),
                        "equal grouping keys must hash alike: {a:?}, {b:?}"
                    );
                    assert_eq!(
                        hash_of(canonical_key(a).as_ref()),
                        hash_of(canonical_key(b).as_ref())
                    );
                }
            }
        }
    }

    #[test]
    fn every_nil_is_one_key_and_nothing_else_merges() {
        let keys = keys();
        assert!(GroupKey::of(&keys[0]) == GroupKey::of(&keys[1]));
        assert!(GroupKey::of(&keys[1]) == GroupKey::of(&keys[2]));
        assert!(GroupKey::of(&keys[6]) == GroupKey::of(&keys[7]));
        assert!(GroupKey::of(&keys[10]) == GroupKey::of(&keys[11]));
        assert!(GroupKey::of(&keys[16]) == GroupKey::of(&keys[17]));
        assert!(GroupKey::of(&keys[18]) == GroupKey::of(&keys[19]));
        // A NIL is not a number, and a Vector with a NIL is not one without.
        assert!(GroupKey::of(&keys[0]) != GroupKey::of(&keys[3]));
        assert!(GroupKey::of(&keys[6]) != GroupKey::of(&keys[8]));
        assert!(GroupKey::of(&keys[8]) == GroupKey::of(&keys[9]));
        assert!(GroupKey::of(&keys[14]) != GroupKey::of(&keys[15]));
    }
}
