use super::{entries, to_json, LIMIT_PROFILE_CEILINGS};
use crate::interpreter::RuntimeLimits;

fn sample() -> (RuntimeLimits, usize) {
    (
        RuntimeLimits {
            max_materialized_elements: 2,
            max_source_bytes: 3,
            max_numeric_literal_digits: 4,
            max_numeric_work: 5,
            max_collection_work: 6,
            max_bigint_bits: 7,
            max_algebraic_terms: 8,
            max_nesting_depth: 9,
        },
        1,
    )
}

/// The three published forms are one list now, so this only has to pin the
/// list. `entries` is exhaustive over `RuntimeLimits`, so a new ceiling
/// cannot reach any consumer without passing through here.
#[test]
fn every_ceiling_is_named_exactly_once() {
    let (limits, steps) = sample();
    let names: Vec<&str> = entries(&limits, steps).iter().map(|(n, _)| *n).collect();
    assert_eq!(names.len(), LIMIT_PROFILE_CEILINGS);
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "duplicate name in {names:?}");
}

#[test]
fn the_json_form_carries_exactly_the_entries() {
    let (limits, steps) = sample();
    let json = to_json(&limits, steps);
    let object = json.as_object().expect("limit profile is an object");
    assert_eq!(object.len(), LIMIT_PROFILE_CEILINGS);
    for (name, value) in entries(&limits, steps) {
        assert_eq!(object[name], serde_json::json!(value), "{name}");
    }
}
