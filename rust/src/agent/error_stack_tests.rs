//! An error report must deliver its error, whatever the failing stack was
//! holding.
//!
//! The case these exist for: the work meter refuses
//! `1 21000 RANGE 1 [ MUL ] FOLD` and names `numericWork`, but the stack at
//! that moment holds a 21,000-element vector and an 81,649-digit partial
//! product. Serialized in full that is 5.7 MB, which a host response ceiling
//! turns into "your answer was too big" — the opposite of what the program
//! actually did wrong.

use crate::agent::api::ComputeOptions;
use crate::test_support::agent_json;
use serde_json::Value as Json;

fn compact_len(report: &Json) -> usize {
    serde_json::to_string(report)
        .expect("a report serializes")
        .len()
}

/// The source whose refusal used to be unreportable.
const RUNAWAY_FOLD: &str = "1 21000 RANGE 1 [ MUL ] FOLD";

#[tokio::test]
async fn a_refusal_reports_its_resource_rather_than_its_residue() {
    let report = agent_json(RUNAWAY_FOLD, ComputeOptions::agent(None)).await;
    assert_eq!(report["status"], "error");
    assert_eq!(
        report["error"]["resource"], "numericWork",
        "the ceiling that fired must survive to the wire"
    );
    assert!(
        compact_len(&report) < 64 * 1024,
        "a refusal must be deliverable; got {} bytes",
        compact_len(&report)
    );
}

#[tokio::test]
async fn an_elided_slot_says_what_it_dropped() {
    let report = agent_json(RUNAWAY_FOLD, ComputeOptions::agent(None)).await;
    let elided = &report["stackElided"];
    assert_eq!(elided["reason"], "errorStackBudget");
    assert_eq!(
        elided["slots"][0]["index"], 0,
        "the record names which slots were dropped"
    );
    assert_eq!(
        elided["slots"][0]["elements"], 21000,
        "and how much was in them"
    );
    assert!(
        elided["slots"][0]["approxBytes"].as_u64().unwrap() > 1_000_000,
        "and roughly what they would have cost"
    );
}

#[tokio::test]
async fn an_elided_slot_keeps_its_position_and_its_kind() {
    let report = agent_json(RUNAWAY_FOLD, ComputeOptions::agent(None)).await;
    let stack = report["stack"].as_array().expect("an array");
    assert_eq!(
        stack.len(),
        report["stackDisplay"].as_array().unwrap().len(),
        "the two views of the stack stay aligned"
    );
    // Dropping the slot outright would renumber everything above it and
    // silently move whatever a diagnosis points at.
    assert_eq!(stack[0]["type"], "vector", "the domain is still named");
    assert!(stack[0]["value"].is_null(), "the value is what goes");
    assert_eq!(stack[0]["elided"]["reason"], "errorStackBudget");
    // The block that was applied is small, so it is still there in full:
    // the budget drops what it must, not everything.
    assert_eq!(
        report["stackDisplay"][2], "[ MUL ]",
        "an affordable slot is untouched"
    );
}

#[tokio::test]
async fn a_text_only_client_still_learns_what_was_there() {
    let report = agent_json(RUNAWAY_FOLD, ComputeOptions::agent(None)).await;
    let marker = report["stackDisplay"][0].as_str().expect("a string");
    assert!(
        marker.contains("elided") && marker.contains("21000"),
        "`stackDisplay` is the whole result for a text-only client, got: {marker}"
    );
}

#[tokio::test]
async fn an_ordinary_error_is_not_elided_at_all() {
    // The budget must be invisible to the errors an agent actually meets.
    for source in [
        "[ 1 2 3 ] LENGHT",
        "1 0 DIV 2 UNKNOWNWORD",
        "[ 1 2 ] [ 1 2 3 ] ADD",
    ] {
        let report = agent_json(source, ComputeOptions::agent(None)).await;
        assert_eq!(report["status"], "error", "`{source}` must fail");
        assert!(
            report["stackElided"].is_null(),
            "`{source}` must report no elision"
        );
        for node in report["stack"].as_array().expect("an array") {
            assert!(
                node.get("elided").is_none(),
                "`{source}` must carry its stack in full"
            );
        }
    }
}

/// A success under the agent profile is sent whole while it fits the
/// profile's stack budget, and elided — never refused — past it. The
/// values that fit arrive in full; the one that does not arrives as a
/// record of what it was, so the caller learns it left a 20,001-element
/// intermediate behind instead of learning only that its answer was large.
#[tokio::test]
async fn an_oversized_success_is_elided_rather_than_refused() {
    let report = agent_json("0 20000 RANGE 1 2 ADD", ComputeOptions::agent(None)).await;
    assert_eq!(report["status"], "ok");
    assert_eq!(report["outcome"], "value");
    assert_eq!(report["stackElided"]["reason"], "valueStackBudget");
    assert_eq!(
        report["stackElided"]["budgetBytes"],
        crate::agent::api::AGENT_STACK_BUDGET_BYTES
    );
    assert_eq!(report["stackElided"]["slots"][0]["index"], 0);
    assert_eq!(report["stackElided"]["slots"][0]["elements"], 20001);
    assert_eq!(report["stack"][0]["type"], "vector");
    assert!(report["stack"][0]["value"].is_null());
    assert_eq!(report["stack"][0]["elided"]["reason"], "valueStackBudget");
    assert_eq!(
        report["stackDisplay"][1], "3/1",
        "the answer that fits is whole"
    );
    assert!(
        compact_len(&report) < crate::agent::api::AGENT_STACK_BUDGET_BYTES + 8 * 1024,
        "an elided success fits the budget it was sent under; got {} bytes",
        compact_len(&report)
    );
}

/// The budget is the agent profile's. The trusted profile — `ajisai run`,
/// a host with no response ceiling — sends every success whole.
#[tokio::test]
async fn a_success_without_a_budget_is_never_elided() {
    let report = agent_json("0 20000 RANGE", ComputeOptions::default()).await;
    assert_eq!(report["status"], "ok");
    assert!(report["stackElided"].is_null());
    assert!(compact_len(&report) > 1_000_000);
}

/// A success that fits the budget is byte-for-byte what it always was:
/// a 5,000-element vector of small integers is 433 KB and still whole.
#[tokio::test]
async fn a_success_within_the_budget_is_untouched() {
    let report = agent_json("0 5000 RANGE", ComputeOptions::agent(None)).await;
    assert_eq!(report["status"], "ok");
    assert!(report["stackElided"].is_null());
    assert_eq!(report["stack"][0]["value"].as_array().unwrap().len(), 5001);
}

/// The byte estimate a record carries is the size the slot would have
/// serialized to, within a few percent — not a safe multiple of it.
#[tokio::test]
async fn the_estimate_tracks_the_real_size() {
    let whole = agent_json("0 99999 RANGE", ComputeOptions::default()).await;
    let real = serde_json::to_string(&whole["stack"][0]).unwrap().len()
        + whole["stackDisplay"][0].as_str().unwrap().len();
    let elided = agent_json("0 99999 RANGE", ComputeOptions::agent(None)).await;
    let estimate = elided["stackElided"]["slots"][0]["approxBytes"]
        .as_u64()
        .unwrap() as usize;
    assert!(
        estimate >= real && estimate < real + real / 10,
        "estimated {estimate} bytes for a slot that serializes to {real}"
    );
}

/// Nineteen 256-term algebraic values, refused by the work meter at the
/// twentieth (each costs 524,560 of the profile's 10,000,000 units).
const REPEATED_CASCADE: &str = "[ 2 SQRT 3 SQRT ADD 5 SQRT 7 SQRT ADD MUL 11 SQRT 13 SQRT ADD MUL \
17 SQRT 19 SQRT ADD MUL 23 SQRT 29 SQRT ADD MUL 31 SQRT 37 SQRT ADD MUL 41 SQRT 43 SQRT ADD MUL \
47 SQRT 53 SQRT ADD MUL ] 'C' DEF C C C C C C C C C C C C C C C C C C C C";

#[tokio::test]
async fn eliding_an_algebraic_value_drops_the_part_that_is_large() {
    // For an algebraic value the number lives in `semantics.exactTerms`,
    // while `value` is only the marked approximation — so dropping `value`
    // and keeping `semantics` dropped the cheap half and kept the expensive
    // one. Eighteen 256-term values came to 388 KB with seventeen of them
    // reported as elided.
    let report = agent_json(REPEATED_CASCADE, ComputeOptions::agent(None)).await;
    assert_eq!(report["status"], "error");
    assert!(
        compact_len(&report) < 128 * 1024,
        "an elided algebraic stack must actually shrink; got {} bytes",
        compact_len(&report)
    );
    let elided = report["stack"]
        .as_array()
        .expect("an array")
        .iter()
        .find(|node| node.get("elided").is_some())
        .expect("something was elided");
    assert!(
        elided["semantics"].get("exactTerms").is_none(),
        "the exact form is the value, and an elided slot carries no value"
    );
    assert_eq!(
        elided["elided"]["algebraicTerms"], 256,
        "how much was there is said in the record instead, so nothing goes \
         missing silently"
    );
    assert_eq!(
        elided["type"], "number",
        "what kind of value it was still survives"
    );
}
