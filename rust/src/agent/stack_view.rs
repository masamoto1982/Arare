//! The bounded view of a stack that the playground draws.
//!
//! The playground draws at most `MAX_RENDERED_ELEMENTS_PER_COLLECTION` (100)
//! leading elements of any collection and states how many it left out
//! (`src/gui/output-display-renderer.ts`). It used to be handed every element
//! anyway, as one protocol node each: `0 999999 RANGE` became a million JS
//! objects, built at about 7 µs apiece, so a run that computed in a few
//! milliseconds spent seconds in the conversion before drawing a hundred of
//! them.
//!
//! This view is the protocol node (`agent::report::protocol_node_json`, the
//! same `{ semantics, type, value }` every host reads) with each Vector cut to
//! its first `VIEW_ELEMENTS_PER_COLLECTION` elements. A cut Vector says what
//! it left out in a `truncated` field (`spec/host-protocol.schema.json`), and
//! says it about the whole value, so a
//! host that reads only the view still decides what it decides from the whole
//! value:
//!
//! - `length`: the Vector's element count, which the elision marker states;
//! - `holdsNil` / `holdsRecord`: whether a NIL or a Record sits anywhere in
//!   the left-out elements, which decide "did this run leave a NIL" and
//!   whether the Vector is drawn as a literal or as a `COLLECT` phrase;
//! - `digest`: a hash of the left-out elements under value equality, so two
//!   views are equal exactly when the values are (to 64 bits), which is what
//!   decides whether a run changed the stack.
//!
//! Nothing below the view changes: it is presentation, read by a host, and
//! the value on the stack stays whole. A Vector of up to
//! `VIEW_ELEMENTS_PER_COLLECTION` elements is rendered exactly as
//! `protocol_node_json` renders it, which `stack_view_tests` pins.

use super::report::{protocol_node_json, semantics_json};
use crate::types::value_protocol::value_to_protocol;
use crate::types::{Value, ValueData};
use serde_json::{json, Map, Value as Json};
use std::hash::{Hash, Hasher};

/// Elements of one Vector the view keeps. At least the playground's
/// `MAX_RENDERED_ELEMENTS_PER_COLLECTION`, so the view always holds every
/// element the playground draws.
pub(crate) const VIEW_ELEMENTS_PER_COLLECTION: usize = 100;

/// The bounded view of every value on `stack`, bottom first.
pub(crate) fn stack_view_json(stack: &[Value]) -> Json {
    Json::Array(stack.iter().map(value_view_json).collect())
}

/// The bounded view of one value: its protocol node, with every Vector at
/// any depth cut to its first `VIEW_ELEMENTS_PER_COLLECTION` elements.
pub(crate) fn value_view_json(value: &Value) -> Json {
    match &value.data {
        ValueData::Vector(_) | ValueData::Tensor { .. } => vector_view_json(value),
        ValueData::Record(record) => {
            let mut obj = Map::new();
            obj.insert("semantics".into(), semantics_json(value));
            obj.insert("type".into(), json!("record"));
            obj.insert(
                "value".into(),
                json!({
                    "keys": record.keys().iter().map(value_view_json).collect::<Vec<_>>(),
                    "values": record.values().iter().map(value_view_json).collect::<Vec<_>>(),
                }),
            );
            Json::Object(obj)
        }
        _ => protocol_node_json(&value_to_protocol(value)),
    }
}

fn vector_view_json(value: &Value) -> Json {
    let length = value.len();
    let shown = length.min(VIEW_ELEMENTS_PER_COLLECTION);
    let children: Vec<Json> = (0..shown)
        .filter_map(|index| value.child(index))
        .map(|child| value_view_json(&child))
        .collect();
    let mut obj = Map::new();
    obj.insert("semantics".into(), semantics_json(value));
    obj.insert("type".into(), json!("vector"));
    obj.insert("value".into(), Json::Array(children));
    if length > shown {
        obj.insert("truncated".into(), truncation_json(value, shown, length));
    }
    Json::Object(obj)
}

/// What a cut Vector left out: elements `shown..length` of `value`.
fn truncation_json(value: &Value, shown: usize, length: usize) -> Json {
    let tail = value.children_range(shown, length);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    tail.hash(&mut hasher);
    json!({
        "length": length,
        "holdsNil": holds(&tail, &|v| v.is_nil()),
        "holdsRecord": holds(&tail, &|v| matches!(v.data, ValueData::Record(_))),
        "digest": format!("{:016x}", hasher.finish()),
    })
}

/// Whether `value` or anything inside it satisfies `test`. A dense Tensor
/// holds numbers only, so only boxed Vectors and Records are descended into.
fn holds(value: &Value, test: &dyn Fn(&Value) -> bool) -> bool {
    if test(value) {
        return true;
    }
    match &value.data {
        ValueData::Vector(children) => children.iter().any(|child| holds(child, test)),
        ValueData::Record(record) => record
            .keys()
            .iter()
            .chain(record.values().iter())
            .any(|child| holds(child, test)),
        _ => false,
    }
}
