//! `NOT`, `AND` and `SELECT` over truth columns (`types::truth_column`).
//!
//! Each law of LANG.VALUES.TRUTH is a table over the three lane codes, so a
//! column is answered a byte at a time with no `Value` built. A flat Vector
//! of truth values is packed first and answered the same way; anything the
//! tables cannot say — a lane that is not a truth value, a nested lane,
//! extents that disagree, two different NILs that could both reach the
//! answer — is left to the lifted law in `logic`, which words the ERROR.

use std::borrow::Cow;
use std::sync::Arc;

use crate::interpreter::lane_lift::{common_extent, vector_extent};
use crate::interpreter::logic::compute_selection;
use crate::types::truth_column::{FALSE, TRUE, UNKNOWN};
use crate::types::{TruthColumn, Value, ValueData};

/// One operand of a connective, read as codes.
enum Truths<'a> {
    /// A column, borrowed or packed from a Vector.
    Column(Cow<'a, TruthColumn>),
    /// One truth value, combined with every lane; its NIL when UNKNOWN.
    Lane(u8, Option<&'a Value>),
}

impl<'a> Truths<'a> {
    fn of(value: &'a Value) -> Option<Self> {
        Some(match &value.data {
            ValueData::Truths(column) => Truths::Column(Cow::Borrowed(column)),
            ValueData::Vector(lanes) => Truths::Column(Cow::Owned(TruthColumn::pack(lanes)?)),
            ValueData::Boolean(false) => Truths::Lane(FALSE, None),
            ValueData::Boolean(true) => Truths::Lane(TRUE, None),
            ValueData::Nil(_) => Truths::Lane(UNKNOWN, Some(value)),
            _ => return None,
        })
    }

    /// The extent this operand sets, `None` for one that sets none: a single
    /// truth value, or a column one lane long (LANG.COLLECTIONS.LIFT).
    fn extent(&self) -> Option<usize> {
        match self {
            Truths::Column(column) => vector_extent(column.len()),
            Truths::Lane(..) => None,
        }
    }

    fn code(&self, index: usize) -> u8 {
        match self {
            Truths::Column(column) if column.len() == 1 => column.codes()[0],
            Truths::Column(column) => column.codes()[index],
            Truths::Lane(code, _) => *code,
        }
    }

    fn unknown(&self) -> Option<&Value> {
        match self {
            Truths::Column(column) => column.unknown(),
            Truths::Lane(_, nil) => *nil,
        }
    }
}

/// `NOT` on a column: TRUE and FALSE swap, UNKNOWN stays
/// (LANG.VALUES.TRUTH). Rewritten in place when the column is the only
/// holder of its bytes.
pub(crate) fn negate(value: &mut Value) -> bool {
    if let ValueData::Vector(lanes) = &value.data {
        match TruthColumn::pack(lanes) {
            Some(column) => *value = Value::from_truth_column(column),
            None => return false,
        }
    }
    let ValueData::Truths(column) = &mut value.data else {
        return false;
    };
    for code in Arc::make_mut(column).codes_mut() {
        *code = negate_code(*code);
    }
    true
}

/// One lane of `NOT`. FALSE (0) and TRUE (1) differ in the low bit alone.
#[inline]
fn negate_code(code: u8) -> u8 {
    if code < UNKNOWN {
        code ^ 1
    } else {
        code
    }
}

/// The strong Kleene conjunction, `CONJUNCTION[3 * a + b]`. Where the law
/// answers an operand whole the code is that operand's, so an UNKNOWN keeps
/// its NIL: the right one's from `T ∧ U` and `U ∧ U`, the left one's from
/// `U ∧ T`.
const CONJUNCTION: [u8; 9] = [
    FALSE, FALSE, FALSE, // FALSE ∧ _
    FALSE, TRUE, UNKNOWN, // TRUE ∧ b = b
    FALSE, UNKNOWN, UNKNOWN, // UNKNOWN ∧ _
];

/// `AND` when either operand is a column and both read as codes.
pub(crate) fn conjoin(a: &Value, b: &Value) -> Option<Value> {
    if !matches!(a.data, ValueData::Truths(_) | ValueData::Vector(_))
        && !matches!(b.data, ValueData::Truths(_) | ValueData::Vector(_))
    {
        return None;
    }
    let (a, b) = (Truths::of(a)?, Truths::of(b)?);
    // Two different NILs could both reach the answer, and a column holds
    // one: the lifted law keeps each lane's own.
    let unknown = match (a.unknown(), b.unknown()) {
        (Some(x), Some(y)) if x.absence_metadata() != y.absence_metadata() => return None,
        (x, y) => x.or(y).cloned(),
    };
    let width = common_extent([a.extent(), b.extent()]).ok()?.unwrap_or(1);
    let codes = (0..width)
        .map(|i| CONJUNCTION[usize::from(3 * a.code(i) + b.code(i))])
        .collect();
    let column = TruthColumn::with_unknown(codes, unknown).expect("an UNKNOWN lane has its NIL");
    Some(Value::from_truth_column(column))
}

/// A `SELECT` candidate: a leaf, or a Vector or column of leaves.
enum Candidate<'a> {
    Leaf(&'a Value),
    Lanes(&'a [Value]),
    Column(&'a TruthColumn),
}

impl<'a> Candidate<'a> {
    fn of(value: &'a Value) -> Option<Self> {
        Some(match &value.data {
            ValueData::Truths(column) => Candidate::Column(column),
            ValueData::Vector(lanes) if lanes.iter().all(|lane| !is_container(lane)) => {
                Candidate::Lanes(lanes)
            }
            _ if !is_container(value) => Candidate::Leaf(value),
            _ => return None,
        })
    }

    fn extent(&self) -> Option<usize> {
        match self {
            Candidate::Lanes(lanes) => vector_extent(lanes.len()),
            Candidate::Column(column) => vector_extent(column.len()),
            Candidate::Leaf(_) => None,
        }
    }

    fn lane(&self, index: usize) -> Cow<'a, Value> {
        match self {
            Candidate::Leaf(value) => Cow::Borrowed(value),
            Candidate::Lanes(lanes) => {
                Cow::Borrowed(&lanes[if lanes.len() == 1 { 0 } else { index }])
            }
            Candidate::Column(column) => {
                Cow::Owned(column.lane(if column.len() == 1 { 0 } else { index }))
            }
        }
    }
}

/// `SELECT` when the mask reads as codes and is longer than one lane, and the
/// candidates are leaves or flat: each lane is the candidate its code chooses,
/// the mask's NIL for UNKNOWN.
pub(crate) fn select(when_true: &Value, when_false: &Value, mask: &Value) -> Option<Value> {
    if !matches!(mask.data, ValueData::Truths(_) | ValueData::Vector(_)) {
        return None;
    }
    let Truths::Column(column) = Truths::of(mask)? else {
        return None;
    };
    let (t, f) = (Candidate::of(when_true)?, Candidate::of(when_false)?);
    let width = column.len();
    // A one-lane mask is reused across the candidates' extent; the lifted
    // law sizes that answer, and answers an empty mask.
    if width < 2 || common_extent([Some(width), t.extent(), f.extent()]).ok()? != Some(width) {
        return None;
    }
    if let Some(dense) = select_numbers(&t, &f, column.codes()) {
        return Some(dense);
    }
    let lanes = column
        .codes()
        .iter()
        .enumerate()
        .map(|(i, &code)| match code {
            TRUE => t.lane(i).into_owned(),
            FALSE => f.lane(i).into_owned(),
            _ => compute_selection(&t.lane(i), &f.lane(i), &column.lane(i))
                .expect("an UNKNOWN lane is a truth value"),
        })
        .collect();
    Some(Value::from_vector(lanes))
}

/// `SELECT` between two machine-word numbers under a mask of TRUE and FALSE
/// alone: every lane is one of the two pairs, so the answer is written
/// straight into a dense Tensor's columns — the Vector of those numbers, held
/// as `Value::from_vector_promoted` would hold it. An UNKNOWN lane answers a
/// NIL, which no column holds.
fn select_numbers(t: &Candidate, f: &Candidate, codes: &[u8]) -> Option<Value> {
    let pair = |c: &Candidate| match c {
        Candidate::Leaf(value) => value.as_scalar()?.extract_i64_pair(),
        _ => None,
    };
    let ((tn, td), (fn_, fd)) = (pair(t)?, pair(f)?);
    if codes.iter().any(|&code| code > TRUE) {
        return None;
    }
    let (mut nums, mut dens) = (
        Vec::with_capacity(codes.len()),
        Vec::with_capacity(codes.len()),
    );
    for &code in codes {
        let on = code == TRUE;
        nums.push(if on { tn } else { fn_ });
        dens.push(if on { td } else { fd });
    }
    let width = codes.len();
    let pure = td == 1 && fd == 1;
    let data = crate::types::DenseTensor::from_columns(nums, dens, [width], pure);
    Some(Value::from_dense_tensor(data, vec![width]))
}

fn is_container(value: &Value) -> bool {
    matches!(
        value.data,
        ValueData::Vector(_)
            | ValueData::Tensor { .. }
            | ValueData::Record(_)
            | ValueData::Truths(_)
    )
}

#[cfg(kani)]
#[path = "truth_ops_proofs.rs"]
mod proofs;
