//! A flat Vector of truth values held one byte a lane.
//!
//! A mask — what `GT` answers over a Vector of numbers, what `AND`, `NOT`
//! and `SELECT` read — is the commonest Vector a program builds, and as
//! `Value`s it is four words a lane to say one of three things. Held here it
//! is one byte: FALSE, TRUE or UNKNOWN, where every UNKNOWN lane is
//! the same NIL, kept once beside the bytes. It is a representation and
//! nothing more (LANG.AUTHORITY.FREEDOM): it is equal to, hashes as and
//! displays as the Vector of `Value`s it stands for, and anything that does
//! not read it directly reads that Vector (`TruthColumn::to_values`).

use super::{Value, ValueData};
use std::sync::Arc;

impl Value {
    /// The Vector `column` stands for, held as the column.
    pub(crate) fn from_truth_column(column: TruthColumn) -> Self {
        Self::new(ValueData::Truths(Arc::new(column)), None)
    }
}

/// The byte a lane holds.
pub(crate) const FALSE: u8 = 0;
pub(crate) const TRUE: u8 = 1;
/// The column's one NIL, whatever its reason.
pub(crate) const UNKNOWN: u8 = 2;

/// The lanes of a flat truth Vector, and the NIL its UNKNOWN lanes all are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruthColumn {
    codes: Vec<u8>,
    /// Present exactly when some lane is UNKNOWN.
    unknown: Option<Value>,
}

impl TruthColumn {
    /// A column of FALSE and TRUE lanes. Every code must be one of those
    /// two: a lane that is UNKNOWN needs the NIL it stands for.
    pub(crate) fn known(codes: Vec<u8>) -> Self {
        debug_assert!(codes.iter().all(|&c| c < UNKNOWN));
        Self {
            codes,
            unknown: None,
        }
    }

    /// A column whose UNKNOWN lanes are `unknown`, a NIL. `None` when a lane
    /// is UNKNOWN and no NIL is given.
    pub(crate) fn with_unknown(codes: Vec<u8>, unknown: Option<Value>) -> Option<Self> {
        debug_assert!(unknown.as_ref().is_none_or(Value::is_nil));
        let any_unknown = codes.contains(&UNKNOWN);
        if any_unknown && unknown.is_none() {
            return None;
        }
        Some(Self {
            codes,
            unknown: unknown.filter(|_| any_unknown),
        })
    }

    /// The column `lanes` are, when each is a Boolean or one NIL
    /// shared by every NIL among them — the same reason and detail.
    pub(crate) fn pack(lanes: &[Value]) -> Option<Self> {
        let (codes, first_nil) = pack_codes(lanes.iter().map(|lane| match &lane.data {
            ValueData::Boolean(truth) => PackLane::Truth(*truth),
            ValueData::Nil(_) => PackLane::Nil(lane.absence_metadata()),
            _ => PackLane::Other,
        }))?;
        Some(Self {
            codes,
            unknown: first_nil.map(|index| lanes[index].clone()),
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.codes.len()
    }

    pub(crate) fn codes(&self) -> &[u8] {
        &self.codes
    }

    pub(crate) fn codes_mut(&mut self) -> &mut [u8] {
        &mut self.codes
    }

    /// The NIL the UNKNOWN lanes are, when there are any.
    pub(crate) fn unknown(&self) -> Option<&Value> {
        self.unknown.as_ref()
    }

    /// Lane `index` as the `Value` it stands for.
    pub(crate) fn lane(&self, index: usize) -> Value {
        match self.codes[index] {
            FALSE => Value::from_bool(false),
            TRUE => Value::from_bool(true),
            _ => self.unknown.clone().expect("an UNKNOWN lane has its NIL"),
        }
    }

    /// Every lane as the `Value` it stands for.
    pub(crate) fn to_values(&self) -> Vec<Value> {
        (0..self.len()).map(|i| self.lane(i)).collect()
    }

    /// The Vector of `Value`s this column stands for: what any reader that
    /// does not read the bytes reads instead.
    pub(crate) fn to_vector(&self) -> Value {
        Value::from_vector(self.to_values())
    }
}

/// One lane as [`TruthColumn::pack`] reads it: a truth value, a NIL with what
/// tells it apart from another NIL, or anything else.
enum PackLane<K> {
    Truth(bool),
    Nil(K),
    Other,
}

/// The codes of `lanes` and the index of the first NIL among them, when every
/// lane is a truth value or a NIL and every NIL has the first one's key.
fn pack_codes<K: PartialEq>(
    lanes: impl ExactSizeIterator<Item = PackLane<K>>,
) -> Option<(Vec<u8>, Option<usize>)> {
    let mut first_nil: Option<(usize, K)> = None;
    let mut codes = Vec::with_capacity(lanes.len());
    for (index, lane) in lanes.enumerate() {
        codes.push(match lane {
            PackLane::Truth(false) => FALSE,
            PackLane::Truth(true) => TRUE,
            PackLane::Nil(key) => {
                match &first_nil {
                    None => first_nil = Some((index, key)),
                    Some((_, first)) if *first == key => {}
                    Some(_) => return None,
                }
                UNKNOWN
            }
            PackLane::Other => return None,
        });
    }
    Some((codes, first_nil.map(|(index, _)| index)))
}

#[cfg(kani)]
#[path = "truth_column_proofs.rs"]
mod proofs;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NilReason;

    fn nil(reason: NilReason) -> Value {
        Value::nil_with_reason(reason)
    }

    #[test]
    fn packing_round_trips_and_refuses_what_it_cannot_hold() {
        let lanes = vec![
            Value::from_bool(true),
            Value::from_bool(false),
            nil(NilReason::DomainMiss),
            nil(NilReason::DomainMiss),
        ];
        let column = TruthColumn::pack(&lanes).expect("one shared NIL");
        assert_eq!(column.to_values(), lanes);
        assert_eq!(column.codes(), &[TRUE, FALSE, UNKNOWN, UNKNOWN]);

        let mixed = [nil(NilReason::DomainMiss), nil(NilReason::NotFound)];
        assert!(TruthColumn::pack(&mixed).is_none(), "two reasons");
        assert!(
            TruthColumn::pack(&[Value::from_int(1)]).is_none(),
            "a number"
        );
        assert!(TruthColumn::with_unknown(vec![UNKNOWN], None).is_none());
        let known = TruthColumn::with_unknown(vec![TRUE], Some(nil(NilReason::NotFound)))
            .expect("no lane needs the NIL");
        assert_eq!(
            known,
            TruthColumn::known(vec![TRUE]),
            "an unused NIL is dropped"
        );
    }
}
