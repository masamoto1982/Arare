//! Ordering and grouping Words: `ORDER`, `UNIQUE`, `TALLY`, `GROUP`.
//! `TALLY` and `GROUP` answer Records (LANG.RECORDS.STRUCTURE).
//!
//! Every one of these is expressible in the existing vocabulary — and every
//! one of them, written that way, costs asymptotically more than the same work
//! done here. Ordering by a key is the clearest case: the reference's stable
//! ranking idiom is O(n²) interpreted steps, so `SORT` handled five thousand
//! elements natively while a k-nearest-neighbour classifier stopped at ninety.
//! LANG.AUTHORITY.IDENTITY keeps a Word out of Core when a user definition
//! *expresses* it; these are here because expressibility was never the whole
//! question.

use crate::error::{AjisaiError, Result};
use crate::interpreter::collection_meter::{charge_comparison_sort, ScanMeter};
use crate::interpreter::value_order::order_indices;
use crate::interpreter::Interpreter;
use crate::types::group_key::GroupKey;
use crate::types::{RecordData, Value, ValueData};
use std::collections::HashMap;

/// Take the Word's single operand off the stack.
pub(super) fn take_operand(interp: &mut Interpreter) -> Result<Value> {
    interp.stack.pop().ok_or(AjisaiError::stack_underflow())
}

/// Put an operand back when the Word failed after consuming it.
pub(super) fn restore(interp: &mut Interpreter, value: Value) {
    interp.stack.push(value);
}

/// The elements of a vector operand, or a structure error naming what came
/// instead. `expected` is a noun phrase: the diagnosis template supplies
/// "Structure error: expected _, got _" and the locus supplies the Word.
pub(super) fn elements_of(value: &Value, expected: &str) -> Result<Vec<Value>> {
    match value.as_vector_view() {
        Some(view) => Ok(view.into_owned()),
        // Every caller (ZIP, SUM, ORDER, UNIQUE, TALLY, GROUP) declares
        // `nonVector` in its own `errorWhen` — confirmed uniform across all
        // six, unlike Phase 2's shared-helper cases, so the remap is safe here.
        None => Err(AjisaiError::declared(
            "nonVector",
            format!("expected {}, got {}", expected, value.domain_name()),
        )),
    }
}

/// `ORDER ( [ vec ] -> [ permutation ] )`: the indices of `vec` in the order
/// that would sort it ascending, ties in original index order.
///
/// This is the Word the whole ordering family was missing. `SORT` answers
/// *what* the sorted values are and throws away *where* they came from, so
/// anything that has to carry a payload alongside the key — the k nearest
/// neighbours, the top-k scores, a median's position, a ranking, an argmin —
/// had to rebuild the ordering by counting, at quadratic cost. With the
/// permutation in hand each of those is `ORDER` and a `GET`.
///
/// Stability is part of the contract, not an accident of the sort used: two
/// equal keys keep their original order, which is what makes "the 3 nearest"
/// a well-defined set when two distances tie.
pub fn op_order(interp: &mut Interpreter) -> Result<()> {
    let value = take_operand(interp)?;
    if value.is_nil() {
        interp.stack.push(value);
        return Ok(());
    }

    let items = match elements_of(&value, "vector") {
        Ok(items) => items,
        Err(e) => {
            restore(interp, value);
            return Err(e);
        }
    };

    if let Err(e) = charge_comparison_sort(interp, &items) {
        restore(interp, value);
        return Err(e);
    }

    match order_indices(&items) {
        Ok(Some(perm)) => {
            let out: Vec<Value> = perm
                .into_iter()
                .map(|i| Value::from_int(i as i64))
                .collect();
            interp.stack.push(Value::from_vector(out));
            Ok(())
        }
        // A Vector holding `0/0` has no order to answer (LANG.VALUES.EXACT).
        Ok(None) => {
            interp
                .stack
                .push(crate::interpreter::comparison::unordered_projection());
            Ok(())
        }
        Err(e) => {
            restore(interp, value);
            Err(e)
        }
    }
}

/// The distinct elements of `items` in first-occurrence order, paired with how
/// many times each occurred. One pass shared by `UNIQUE` and `TALLY` so the
/// two can never disagree about either the order or the counts.
///
/// Equality is value equality, so this works over the domains `SORT` does not:
/// class labels are usually text, and a grouping key is whatever the data
/// says it is. Every NIL is one key whatever its reason ([`GroupKey`],
/// LANG.VALUES.ORDER), as every NULL is one group to SQL.
///
/// A `HashMap<Value, usize>` keyed on the output index makes this one pass
/// over `items` rather than a scan of the accumulated result for every
/// element (`n × distinct` in the worst case, when nothing repeats). `Value`
/// hashes the same representation-independent way it compares — a rational,
/// text, boolean, or rectangular numeric vector/tensor hashes cheaply; an
/// algebraic element pays a bounded interval-refinement pass to find its
/// canonical bucket (`Algebraic::hash`) — so first-occurrence order still
/// costs one hash-and-lookup per element rather than a probe against every
/// distinct value seen so far. See `collection_meter::ScanMeter`.
fn distinct_with_counts(interp: &mut Interpreter, items: &[Value]) -> Result<Vec<(Value, usize)>> {
    use std::collections::hash_map::Entry;

    let meter = ScanMeter::new(items);
    // Indices into `items`, not clones: the element itself is only cloned
    // once, when it turns out to be one of the survivors, instead of once to
    // probe the map and again to hold the result.
    let mut order: Vec<usize> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();
    let mut index: HashMap<GroupKey<'_>, usize> = HashMap::with_capacity(items.len());
    for (completed, item) in items.iter().enumerate() {
        meter.charge_scan_of(interp, completed)?;
        match index.entry(GroupKey::of(item)) {
            Entry::Occupied(e) => counts[*e.get()] += 1,
            Entry::Vacant(e) => {
                meter.charge_retained(interp, completed)?;
                e.insert(order.len());
                order.push(completed);
                counts.push(1);
            }
        }
    }
    Ok(order
        .into_iter()
        .zip(counts)
        .map(|(idx, count)| (items[idx].clone(), count))
        .collect())
}

/// [`distinct_with_counts`] over a flat pure-integer dense buffer, reading its
/// numerator column.
///
/// `Some(pairs)` when this route ran — each distinct lane with its count, in
/// first-occurrence order, exactly as the boxed scan reports them. `None` when
/// the value is any other shape and that scan must handle it. An `Err` is the
/// scan's own refusal, raised at the same element and with the same progress.
///
/// The boxed scan materializes a `Tensor` into one 96-byte `Value` per lane and
/// then keys a `HashMap<&Value, usize>` on it, hashing through `ValueData`'s
/// discriminant and `Fraction`'s reduced pair to ask a question about one
/// machine integer. Keying the `i64` instead is the same question: every lane of
/// a pure-integer buffer has denominator 1, so numerator equality *is* value
/// equality, and `Fraction`'s cross-multiplying `eq` agrees lane for lane.
///
/// **The charge is the scan's own, element for element.** `ScanMeter::for_vector`
/// reaches the same `ElementCost` without materializing (see its note), and the
/// sequence of charges depends on nothing else: `charge_scan_of` once per lane in
/// order, and `charge_retained` only where a lane is newly distinct — which for
/// integers is decided identically either way. So a ceiling refuses at the same
/// lane, with the same `completed` progress, and the choice of route stays
/// unobservable (LANG.AUTHORITY.FREEDOM).
fn dense_integer_distinct_with_counts(
    interp: &mut Interpreter,
    value: &Value,
) -> Result<Option<Vec<(i64, usize)>>> {
    let ValueData::Tensor { data, shape } = &value.data else {
        return Ok(None);
    };
    // A rational lane is not identified by its numerator, and rank above 1
    // reports distinct *rows*.
    if shape.len() != 1 || !data.is_pure_integer {
        return Ok(None);
    }

    let meter = ScanMeter::for_vector(value);
    let mut first_seen: HashMap<i64, usize> = HashMap::with_capacity(data.len());
    let mut distinct: Vec<(i64, usize)> = Vec::new();
    for (completed, lane) in data.numerators.iter().enumerate() {
        meter.charge_scan_of(interp, completed)?;
        match first_seen.entry(*lane) {
            std::collections::hash_map::Entry::Occupied(slot) => distinct[*slot.get()].1 += 1,
            std::collections::hash_map::Entry::Vacant(slot) => {
                meter.charge_retained(interp, completed)?;
                slot.insert(distinct.len());
                distinct.push((*lane, 1));
            }
        }
    }
    Ok(Some(distinct))
}

/// `UNIQUE ( [ vec ] -> [ distinct ] )`: the distinct elements in
/// first-occurrence order.
///
/// First-occurrence rather than sorted order, because it has to work on values
/// that have no order at all — the labels of a classifier, the tokens of a
/// vocabulary, the keys of a grouping.
pub fn op_unique(interp: &mut Interpreter) -> Result<()> {
    let value = take_operand(interp)?;
    if value.is_nil() {
        interp.stack.push(value);
        return Ok(());
    }
    match dense_integer_distinct_with_counts(interp, &value) {
        Ok(Some(distinct)) => {
            let lanes: Vec<i64> = distinct.into_iter().map(|(lane, _)| lane).collect();
            interp.stack.push(Value::from_int_tensor(lanes));
            return Ok(());
        }
        Ok(None) => {}
        Err(e) => {
            restore(interp, value);
            return Err(e);
        }
    }

    let items = match elements_of(&value, "vector") {
        Ok(items) => items,
        Err(e) => {
            restore(interp, value);
            return Err(e);
        }
    };
    match distinct_with_counts(interp, &items) {
        Ok(distinct) => {
            let out: Vec<Value> = distinct.into_iter().map(|(value, _)| value).collect();
            interp.stack.push(Value::from_vector(out));
            Ok(())
        }
        Err(e) => {
            restore(interp, value);
            Err(e)
        }
    }
}

/// `TALLY ( [ vec ] -> [ record ] )`: a Record from each distinct element to
/// how many times it occurs, keys in the order `UNIQUE` reports them.
///
/// It answered a bare count Vector aligned with `UNIQUE` while a map "would
/// be a new value domain for one Word to own"; the Record domain exists now
/// (LANG.RECORDS.STRUCTURE), so the counts carry what they count. `VALUES`
/// recovers the old answer exactly, and `KEYS` is `UNIQUE`.
pub fn op_tally(interp: &mut Interpreter) -> Result<()> {
    let value = take_operand(interp)?;
    if value.is_nil() {
        interp.stack.push(value);
        return Ok(());
    }
    match dense_integer_distinct_with_counts(interp, &value) {
        Ok(Some(distinct)) => {
            let (keys, counts): (Vec<Value>, Vec<Value>) = distinct
                .into_iter()
                .map(|(lane, count)| (Value::from_int(lane), Value::from_int(count as i64)))
                .unzip();
            push_tally(interp, keys, counts);
            return Ok(());
        }
        Ok(None) => {}
        Err(e) => {
            restore(interp, value);
            return Err(e);
        }
    }

    let items = match elements_of(&value, "vector") {
        Ok(items) => items,
        Err(e) => {
            restore(interp, value);
            return Err(e);
        }
    };
    match distinct_with_counts(interp, &items) {
        Ok(distinct) => {
            let (keys, counts): (Vec<Value>, Vec<Value>) = distinct
                .into_iter()
                .map(|(key, count)| (key, Value::from_int(count as i64)))
                .unzip();
            push_tally(interp, keys, counts);
            Ok(())
        }
        Err(e) => {
            restore(interp, value);
            Err(e)
        }
    }
}

/// The Record `TALLY` answers. The keys are distinct by construction — they
/// are the survivors of one hash-keyed scan — so building the Record cannot
/// fail and charges nothing the scan has not already charged.
fn push_tally(interp: &mut Interpreter, keys: Vec<Value>, counts: Vec<Value>) {
    let record = RecordData::new(keys, counts).expect("distinct_with_counts yields distinct keys");
    interp.stack.push(Value::from_record(record));
}

/// `GROUP ( [ keys ] [ values ] -> [ record ] )`: a Record from each key to
/// the Vector of the `values` at its positions, keys in the order
/// `UNIQUE keys` reports them. Keys come first, as they do for `RECORD`.
///
/// The core of a centroid update, a decision-tree split, a per-class tally and
/// a stratified partition. Written out, each of those was a nested scan over
/// the keys with the index plumbing done by hand.
pub fn op_group(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 2 {
        return Err(AjisaiError::stack_underflow());
    }
    let values_value = interp.stack.pop().expect("checked by len()");
    let keys_value = interp.stack.pop().expect("checked by len()");

    let put_back = |interp: &mut Interpreter, values: &Value, keys: &Value| {
        interp.stack.push(keys.clone());
        interp.stack.push(values.clone());
    };

    let values = match elements_of(&values_value, "vector as second operand") {
        Ok(items) => items,
        Err(e) => {
            put_back(interp, &values_value, &keys_value);
            return Err(e);
        }
    };
    let keys = match elements_of(&keys_value, "vector as first operand") {
        Ok(items) => items,
        Err(e) => {
            put_back(interp, &values_value, &keys_value);
            return Err(e);
        }
    };

    if values.len() != keys.len() {
        put_back(interp, &values_value, &keys_value);
        return Err(AjisaiError::length_mismatch(keys.len(), values.len()));
    }

    // Two prices, because `GROUP` does two things: it hashes each key to find
    // its bucket (`n`, one hash-and-lookup per key — see `distinct_with_counts`)
    // and it copies every value into a bucket (`n`, charged as each one lands).
    let key_meter = ScanMeter::new(&keys);
    let value_meter = ScanMeter::new(&values);
    let mut groups: Vec<(Value, Vec<Value>)> = Vec::new();
    let mut group_index: HashMap<GroupKey<'_>, usize> = HashMap::with_capacity(keys.len());
    for (completed, (value, key)) in values.iter().zip(keys.iter()).enumerate() {
        let charged = key_meter
            .charge_scan_of(interp, completed)
            .and_then(|()| value_meter.charge_retained(interp, completed));
        if let Err(e) = charged {
            put_back(interp, &values_value, &keys_value);
            return Err(e);
        }
        match group_index.entry(GroupKey::of(key)) {
            std::collections::hash_map::Entry::Occupied(e) => {
                groups[*e.get()].1.push(value.clone())
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                if let Err(err) = key_meter.charge_retained(interp, completed) {
                    put_back(interp, &values_value, &keys_value);
                    return Err(err);
                }
                e.insert(groups.len());
                groups.push((key.clone(), vec![value.clone()]));
            }
        }
    }

    let (keys, buckets): (Vec<Value>, Vec<Value>) = groups
        .into_iter()
        .map(|(key, bucket)| (key, Value::from_vector(bucket)))
        .unzip();
    let record = RecordData::new(keys, buckets).expect("group keys are distinct by construction");
    interp.stack.push(Value::from_record(record));
    Ok(())
}
