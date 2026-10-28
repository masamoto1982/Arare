//! The Record: a keyed correspondence, the seventh value domain
//! (LANG.RECORDS.STRUCTURE, LANG.VALUES.DISJOINT).
//!
//! A Record pairs a key sequence with a value sequence, position by position,
//! and no key appears twice. Its observable structure is exactly that pair of
//! sequences: `KEYS` and `VALUES` read them back in the order they were given,
//! and two Records are one value when their key sequences and their value
//! sequences are equal (LANG.VALUES.DENOTATION). Nothing here remembers how a
//! Record was built — `PUT` on an absent key appends, so a key's position is
//! part of the value, but the operations that led to that position are not.
//!
//! No literal spells a Record; `RECORD` is the only constructor
//! (`spec/grammar.json` is untouched by the domain), which is what keeps the
//! lexicon closed while the value space grows.
//!
//! Lookup is by hash, so `GET` on a Record answers in constant expected time where the
//! parallel-vector idiom it replaces (`INDEX-OF` then `GET`) scanned. The
//! index is built once per Record and shared through the `Arc` every
//! `ValueData::Record` holds, so copying a Record onto the stack costs a
//! pointer, as copying a Vector does.

use std::collections::HashMap;

use super::group_key::canonical_key;
use super::Value;

/// The keyed correspondence behind [`super::ValueData::Record`].
#[derive(Debug, Clone)]
pub struct RecordData {
    keys: Vec<Value>,
    values: Vec<Value>,
    /// Position of each key in `keys`, keyed by its canonical form: two keys
    /// are one key when grouping would make them one (LANG.VALUES.ORDER), so
    /// every NIL is one key whatever its reason. Rebuilt by every
    /// constructor; never observed directly (LANG.AUTHORITY.FREEDOM).
    index: HashMap<Value, usize>,
}

/// Why two sequences could not become a Record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordBuildError {
    /// The key and value sequences differ in length.
    LengthMismatch { keys: usize, values: usize },
    /// The key at `first` recurs at `second`.
    DuplicateKey { first: usize, second: usize },
}

impl RecordData {
    /// Pair `keys` with `values` position by position.
    pub fn new(keys: Vec<Value>, values: Vec<Value>) -> Result<Self, RecordBuildError> {
        let values = super::value_identity::unpack_truth_columns(values);
        if keys.len() != values.len() {
            return Err(RecordBuildError::LengthMismatch {
                keys: keys.len(),
                values: values.len(),
            });
        }
        let mut index = HashMap::with_capacity(keys.len());
        for (position, key) in keys.iter().enumerate() {
            if let Some(first) = index.insert(canonical_key(key).into_owned(), position) {
                return Err(RecordBuildError::DuplicateKey {
                    first,
                    second: position,
                });
            }
        }
        Ok(Self {
            keys,
            values,
            index,
        })
    }

    /// How many keys the Record holds.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The key sequence, in its observable order.
    pub fn keys(&self) -> &[Value] {
        &self.keys
    }

    /// The value sequence, aligned with [`RecordData::keys`].
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// The position of `key`, if present.
    pub fn position(&self, key: &Value) -> Option<usize> {
        self.index.get(canonical_key(key).as_ref()).copied()
    }

    /// The value under `key`, if present.
    pub fn get(&self, key: &Value) -> Option<&Value> {
        self.position(key).map(|position| &self.values[position])
    }

    pub fn has(&self, key: &Value) -> bool {
        self.position(key).is_some()
    }

    /// The pairs, in order.
    pub fn entries(&self) -> impl Iterator<Item = (&Value, &Value)> {
        self.keys.iter().zip(self.values.iter())
    }

    /// A copy with `key` set to `value`: replaced in place when the key is
    /// present, appended when it is not. A present key keeps the spelling it
    /// has — a NIL key keeps its first reason.
    pub fn with(&self, key: Value, value: Value) -> Self {
        let mut next = self.clone();
        match next.position(&key) {
            Some(position) => next.values[position] = value,
            None => {
                next.index
                    .insert(canonical_key(&key).into_owned(), next.keys.len());
                next.keys.push(key);
                next.values.push(value);
            }
        }
        next
    }

    /// A copy without `key`, or `None` when the key is absent.
    pub fn without(&self, key: &Value) -> Option<Self> {
        let position = self.position(key)?;
        let mut keys = self.keys.clone();
        let mut values = self.values.clone();
        keys.remove(position);
        values.remove(position);
        Some(Self::new(keys, values).expect("removing a key keeps the keys distinct"))
    }

    /// The right-biased union: this Record's keys in their order with `other`'s
    /// values where the keys coincide, then `other`'s remaining keys appended
    /// in their order.
    ///
    /// One copy of this Record, then one index probe per key of `other`: the
    /// cost is linear in the two Records. It was a [`RecordData::with`] per
    /// key, and each `with` copies the whole Record it extends, so merging
    /// two Records of `n` keys copied `n` Records — quadratic, a fifth of a
    /// second at two thousand keys.
    pub fn merge(&self, other: &Self) -> Self {
        let mut merged = self.clone();
        merged.keys.reserve(other.len());
        merged.values.reserve(other.len());
        for (key, value) in other.entries() {
            match merged.position(key) {
                Some(position) => merged.values[position] = value.clone(),
                None => {
                    merged
                        .index
                        .insert(canonical_key(key).into_owned(), merged.keys.len());
                    merged.keys.push(key.clone());
                    merged.values.push(value.clone());
                }
            }
        }
        merged
    }

    /// The same keys over `f` applied to each value.
    pub fn map_values<E>(&self, mut f: impl FnMut(&Value) -> Result<Value, E>) -> Result<Self, E> {
        let values = self
            .values
            .iter()
            .map(&mut f)
            .collect::<Result<Vec<_>, E>>()?;
        Ok(Self {
            keys: self.keys.clone(),
            values,
            index: self.index.clone(),
        })
    }

    /// Whether `other` has the same key sequence, key for key as the index
    /// compares them, so that the two pair position by position
    /// (LANG.COLLECTIONS.LIFT over Records).
    pub fn same_keys(&self, other: &Self) -> bool {
        self.keys.len() == other.keys.len()
            && self
                .keys
                .iter()
                .zip(&other.keys)
                .all(|(a, b)| canonical_key(a) == canonical_key(b))
    }
}

/// Identity is the two sequences (LANG.VALUES.DENOTATION); the index is a
/// derived view and does not take part.
impl PartialEq for RecordData {
    fn eq(&self, other: &Self) -> bool {
        self.keys == other.keys && self.values == other.values
    }
}

impl Eq for RecordData {}

impl std::hash::Hash for RecordData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.keys.hash(state);
        self.values.hash(state);
    }
}

#[cfg(test)]
mod merge_tests {
    use super::RecordData;
    use crate::error::NilReason;
    use crate::types::Value;

    /// The one-pass union answers what a `with` per key of the right Record
    /// answers — the definition it replaced — key order, values and index
    /// alike, over overlapping, disjoint and NIL keys.
    #[test]
    fn merge_is_a_with_per_right_key() {
        let key = |i: i64| match i {
            -1 => Value::nil_with_reason(NilReason::Literal),
            -2 => Value::nil_with_reason(NilReason::NotFound),
            i if i % 3 == 0 => Value::from_string(&format!("k{i}")),
            i => Value::from_int(i),
        };
        let record = |keys: &[i64], base: i64| {
            RecordData::new(
                keys.iter().map(|&i| key(i)).collect(),
                keys.iter().map(|&i| Value::from_int(base + i)).collect(),
            )
            .expect("distinct keys")
        };
        let cases: [(&[i64], &[i64]); 6] = [
            (&[], &[]),
            (&[1, 2, 3], &[]),
            (&[], &[4, 5]),
            (&[1, 2, 3, 4], &[3, 9, 1, 12]),
            (&[-1, 6, 7], &[-2, 7, 8]),
            (&[0, 1, 2, 3, 4, 5, 6, 7], &[7, 6, 5, 4, 3, 2, 1, 0, 10, 11]),
        ];
        for (left, right) in cases {
            let (left, right) = (record(left, 100), record(right, 200));
            let expected = right
                .entries()
                .fold(left.clone(), |acc, (k, v)| acc.with(k.clone(), v.clone()));
            let merged = left.merge(&right);
            assert_eq!(merged, expected);
            for k in merged.keys() {
                assert_eq!(merged.get(k), expected.get(k));
            }
            assert_eq!(merged.len(), expected.len());
        }
    }
}
