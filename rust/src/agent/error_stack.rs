//! What a report keeps when its stack is too large to send.
//!
//! An error report carries two different things, and they are not equally
//! important. The **error** is the answer: why the program stopped and which
//! ceiling it met. The **stack** is residual state: whatever
//! the program happened to be holding when it stopped. Serializing the residue
//! in full and letting the whole envelope exceed a host's response ceiling
//! trades the answer for the residue, which is backwards.
//!
//! It was not hypothetical. `1 21000 RANGE 1 [ MUL ] FOLD` is refused by the
//! work meter with `numericWork of 10000573 exceeds the limit of 10000000` —
//! precisely the error an agent needs — but the failing stack holds a
//! 21,000-element vector and an 81,649-digit partial product, so the envelope
//! came to 5,773,682 bytes of which 5,571,973 were the stack. Against the MCP
//! adapter's 1 MiB `responseBytes` ceiling that became
//! `hostError: responseTooLarge`, and the agent was told its *answer* was too
//! big rather than that it had exceeded the *work budget* — which points it at
//! shrinking output when the fix is to compute less. `numericWork` could not be
//! reclassified from `injectedLimit` to `boundary` for the same reason: a
//! control whose diagnosis does not survive the wire is not observable.
//!
//! So an error report elides the value payload of the slots it cannot afford,
//! and says that it did. A **successful** result under a host budget does the
//! same, for the opposite reason: a success *is* its stack, and a host that
//! refused the whole result as `responseTooLarge` told the caller nothing of
//! what was on it — not that a 100,000-element intermediate had been left
//! behind, not that the two small values beside it were fine. Elided, the
//! slots that fit arrive whole and the one that does not arrives as a record
//! of what it was, which is what lets the caller fix the program. Three rules
//! make either case honest:
//!
//! 1. **Values are dropped, never reasons.** `error`, `message` and `output`
//!    are never touched here.
//! 2. **Every slot stays in place.** An elided slot keeps its index, `type`,
//!    `semantics` (less an algebraic value's `exactTerms`), and gains an
//!    `elided` record naming what was dropped. Positions stay meaningful, so a
//!    reader that points at stack depth still points at the same thing.
//! 3. **The reason says which budget.** `errorStackBudget` is the fixed
//!    [`MAX_ERROR_STACK_BYTES`] every error report applies;
//!    `valueStackBudget` is the host-chosen one a success was sent under
//!    (`api::ComputeOptions::stack_budget_bytes`), and a host that sets none
//!    never sees it.
//!
//! This bounds what is *sent*, not what is built; the generative ceiling
//! (`maxMaterializedElements`) is what bounds the latter. And it is a
//! best-effort preservation, not a guarantee of delivery: a host whose response
//! ceiling is below the budget still refuses the result, and `responseBytes`
//! remains the hard gate.

use serde_json::{json, Map, Value as Json};

use crate::interpreter::Interpreter;
use crate::types::value_protocol::{value_to_protocol, ProtocolNode, ProtocolValue};
use crate::types::ValueData;

use super::report::{protocol_node_json, semantics_json};

/// Byte budget for an error report's `stack` and `stackDisplay` payload,
/// together.
///
/// One sixteenth of the 1 MiB `responseBytes` ceiling the MCP host profile
/// declares, which leaves the error and the adapter's own envelope fields room
/// to be pathological and still arrive. Ordinary errors are nowhere near it,
/// so nothing an agent normally sees changes by a byte.
pub(super) const MAX_ERROR_STACK_BYTES: usize = 64 * 1024;

/// The `stackElided.reason` / `elided.reason` of an error report's elision.
const ERROR_STACK_BUDGET: &str = "errorStackBudget";
/// The same, for a successful result sent under a host's stack budget.
const VALUE_STACK_BUDGET: &str = "valueStackBudget";

/// The stack of a run: slots the budget could afford, rendered in full; the
/// rest kept in place with their values dropped.
pub(super) struct ElidedStack {
    pub stack: Json,
    pub stack_display: Vec<String>,
    /// The envelope's `stackElided` record, or `None` when everything fit and
    /// the report is byte-for-byte what it always was.
    pub elided: Option<Json>,
}

/// The stack of a failed run, under the fixed error budget.
pub(super) fn elided_error_stack(interp: &Interpreter) -> ElidedStack {
    elided_stack(interp, MAX_ERROR_STACK_BYTES, ERROR_STACK_BUDGET)
}

/// The stack of a successful run, under the budget its host chose.
pub(super) fn elided_value_stack(interp: &Interpreter, budget: usize) -> ElidedStack {
    elided_stack(interp, budget, VALUE_STACK_BUDGET)
}

/// The stack of a successful run with no budget: every slot, whole.
pub(super) fn whole_stack(interp: &Interpreter) -> ElidedStack {
    ElidedStack {
        stack: super::report::stack_json(interp),
        stack_display: super::stack_display(interp),
        elided: None,
    }
}

fn elided_stack(interp: &Interpreter, budget: usize, reason: &'static str) -> ElidedStack {
    // The protocol node is built for every slot, because it is what says which
    // domain the value belonged to and an elided slot still reports that. What
    // is *not* built for a slot the budget cannot afford is its JSON and its
    // display string — which is where the bytes are. Deciding from the node
    // instead of from the serialized text is the difference between throwing
    // away 9 MB and never building it: `0 99999 RANGE LENGHT` spent 1.5 s
    // rendering a stack it was about to discard, close enough to `wallTimeMs`
    // that a slow host would have seen a timeout instead of its typo.
    let slots: Vec<ProtocolNode> = interp.get_stack().iter().map(value_to_protocol).collect();
    let costs: Vec<usize> = slots.iter().map(node_wire_bytes).collect();

    // Fill from the top down. The operands a failure names — and the answer a
    // success leaves — are the ones nearest the top, so when the budget cannot
    // hold everything it is the top that is worth holding. Lower slots that
    // still fit in what remains are kept, so a single enormous slot does not
    // cost the small ones below it.
    let mut remaining = budget;
    let mut keep = vec![false; slots.len()];
    for index in (0..slots.len()).rev() {
        if costs[index] <= remaining {
            remaining -= costs[index];
            keep[index] = true;
        }
    }
    let all_kept = keep.iter().all(|kept| *kept);

    let mut stack = Vec::with_capacity(slots.len());
    let mut stack_display = Vec::with_capacity(slots.len());
    let mut elided_slots = Vec::new();
    for (index, node) in slots.iter().enumerate() {
        if keep[index] {
            stack.push(protocol_node_json(node));
            stack_display.push(render_slot(interp, index));
            continue;
        }
        let elements = element_count(node);
        stack.push(elided_node_json(node, costs[index], elements, reason));
        // A text-only client reads `stackDisplay` and nothing else, so the
        // marker has to carry the same facts the structured record does.
        stack_display.push(match elements {
            Some(elements) => format!(
                "<elided {} of {} elements, ~{} bytes>",
                node.type_str, elements, costs[index]
            ),
            None => format!("<elided {}, ~{} bytes>", node.type_str, costs[index]),
        });
        let mut record = Map::new();
        record.insert("index".into(), json!(index));
        record.insert("approxBytes".into(), json!(costs[index]));
        if let Some(elements) = elements {
            record.insert("elements".into(), json!(elements));
        }
        elided_slots.push(Json::Object(record));
    }

    ElidedStack {
        stack: Json::Array(stack),
        stack_display,
        elided: (!all_kept).then(|| {
            json!({
                "reason": reason,
                "budgetBytes": budget,
                "slots": elided_slots,
            })
        }),
    }
}

/// The display string for one slot, rendered only when the slot is kept.
fn render_slot(interp: &Interpreter, index: usize) -> String {
    interp
        .get_stack()
        .get(index)
        .map(crate::types::Value::to_string)
        .unwrap_or_default()
}

/// Serialized size of a slot — its `stack` node plus its `stackDisplay`
/// string, since a slot is sent as both and the budget covers both — estimated
/// from the node rather than from the text, so a slot about to be discarded is
/// never serialized to find out how big it was.
///
/// Calibrated against `protocol_node_json` and `types::display`: a vector of
/// small integers measures 80 bytes per element in `stack` and 7 in
/// `stackDisplay`, and this estimates 83 and 7. The first version of this
/// estimate charged every interior node a flat 128-byte envelope and counted
/// the value twice for the display, which put `0 99999 RANGE` at "~33 MB" for
/// a stack that serializes to 8.6 MB — a safe direction for an error budget,
/// and a wrong number in a record a caller is meant to act on.
pub(super) fn node_wire_bytes(node: &ProtocolNode) -> usize {
    node_json_bytes(node) + display_bytes(node) + 4
}

/// `{"semantics":…,"type":"…","value":…}` plus the comma that separates it
/// from a sibling: the punctuation and keys are 34 bytes, the rest is content.
fn node_json_bytes(node: &ProtocolNode) -> usize {
    34 + node.type_str.len() + semantics_bytes(node) + value_json_bytes(node)
}

/// The `semantics` block. Empty (`{}`) for every plain value, so the common
/// case costs nothing to estimate; the three kinds that carry one are
/// measured or rendered.
fn semantics_bytes(node: &ProtocolNode) -> usize {
    match &node.semantics.data {
        // `{"approximate":true,"exactTerms":[…]}`, about 64 bytes a term.
        ValueData::ExactScalar(exact) => 38 + exact.algebraic_term_count() * EXACT_TERM_BYTES,
        _ if node.semantics.normalized_absence_metadata().is_some() => {
            // An absence block carries a reason and perhaps a user's text,
            // and there is one per NIL rather than one per element, so
            // rendering it is cheap and guessing it is not.
            serde_json::to_string(&semantics_json(&node.semantics)).map_or(128, |text| text.len())
        }
        // `{"truthValue":"true"}`.
        _ if node.semantics.truth_value().is_some() => 22,
        _ => 2,
    }
}

/// Bytes one algebraic term costs in `semantics.exactTerms`: a numerator, a
/// denominator and a radicand, quoted and keyed. Measured at 62 on a
/// 512-term value.
const EXACT_TERM_BYTES: usize = 64;

/// JSON characters a string's content adds when quoted: its own bytes, two
/// quotes, and an escape for every quote or backslash it contains.
fn quoted_bytes(text: &str) -> usize {
    text.len() + 2 + text.bytes().filter(|b| matches!(b, b'"' | b'\\')).count()
}

fn value_json_bytes(node: &ProtocolNode) -> usize {
    match &node.value {
        ProtocolValue::Null => 4,
        ProtocolValue::Bool(_) => 5,
        ProtocolValue::Text(text) => quoted_bytes(text),
        // `{"denominator":"…","numerator":"…"}`.
        ProtocolValue::Number {
            numerator,
            denominator,
        } => 34 + numerator.len() + denominator.len(),
        ProtocolValue::Children(children) => {
            2 + children.iter().map(node_json_bytes).sum::<usize>()
        }
        // `{"keys":[…],"values":[…]}`.
        ProtocolValue::Record { keys, values } => {
            24 + keys
                .iter()
                .chain(values.iter())
                .map(node_json_bytes)
                .sum::<usize>()
        }
    }
}

/// The slot's `stackDisplay` string, estimated the way `types::display` writes
/// it: a number as `n/d`, a string quoted, a vector as its elements between
/// brackets with one space around each.
fn display_bytes(node: &ProtocolNode) -> usize {
    match &node.value {
        ProtocolValue::Null => 3,
        ProtocolValue::Bool(_) => 5,
        ProtocolValue::Text(text) => quoted_bytes(text),
        ProtocolValue::Number {
            numerator,
            denominator,
        } => match &node.semantics.data {
            // Written from its terms: `1/2*sqrt(2)+sqrt(3)`, about 16 a term.
            ValueData::ExactScalar(exact) => exact.algebraic_term_count() * 16,
            _ => numerator.len() + denominator.len() + 1,
        },
        ProtocolValue::Children(children) => {
            4 + children
                .iter()
                .map(|child| display_bytes(child) + 1)
                .sum::<usize>()
        }
        ProtocolValue::Record { keys, values } => {
            16 + keys
                .iter()
                .chain(values.iter())
                .map(|child| display_bytes(child) + 1)
                .sum::<usize>()
        }
    }
}

/// Terms an algebraic value carries, for the record of what was dropped: with
/// `exactTerms` gone from an elided slot, this is what says how much there was.
fn algebraic_term_count(node: &ProtocolNode) -> Option<usize> {
    match &node.semantics.data {
        ValueData::ExactScalar(exact) => Some(exact.algebraic_term_count()),
        _ => None,
    }
}

/// An elided slot: everything the full node said about *what kind of value*
/// this was, and nothing of the value itself.
///
/// `value` is `null` rather than the node being dropped, because dropping it
/// would renumber every slot above it and silently move what an error points
/// at. A reader tells this apart from a genuine `NIL` by `type` (which still
/// names the real domain) and by the presence of `elided`.
fn elided_node_json(
    node: &ProtocolNode,
    approx_bytes: usize,
    elements: Option<usize>,
    reason: &'static str,
) -> Json {
    let mut obj = Map::new();
    // For an algebraic value the number itself lives in
    // `semantics.exactTerms`, not in `value` — `value` is only the marked
    // approximation. Eliding `value` and keeping `semantics` therefore
    // dropped the cheap half and kept the expensive one: eighteen 256-term
    // values still came to 388 KB with seventeen of them "elided". What a
    // reader needs from a dropped slot is what kind of value it was, which
    // is everything in the block except the exact form; the term count goes
    // into the `elided` record instead, so nothing is silently missing.
    let mut semantics = semantics_json(&node.semantics);
    if let Some(object) = semantics.as_object_mut() {
        object.remove("exactTerms");
    }
    obj.insert("semantics".into(), semantics);
    obj.insert("type".into(), json!(node.type_str));
    obj.insert("value".into(), Json::Null);
    let mut record = Map::new();
    record.insert("reason".into(), json!(reason));
    record.insert("approxBytes".into(), json!(approx_bytes));
    if let Some(elements) = elements {
        record.insert("elements".into(), json!(elements));
    }
    if let Some(terms) = algebraic_term_count(node) {
        record.insert("algebraicTerms".into(), json!(terms));
    }
    obj.insert("elided".into(), Json::Object(record));
    Json::Object(obj)
}

/// Direct children of a composite node, or `None` for a leaf — the one fact
/// about a dropped collection that its `semantics` block does not carry.
fn element_count(node: &ProtocolNode) -> Option<usize> {
    match &node.value {
        ProtocolValue::Children(children) => Some(children.len()),
        _ => None,
    }
}
