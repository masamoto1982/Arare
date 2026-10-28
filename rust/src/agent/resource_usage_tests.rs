//! A resource an agent plans against has to report the number it is judged by.
//!
//! `runtimeMetrics.executionSteps` was read from a `RuntimeMetrics` field that
//! no code ever wrote, sitting beside the `Interpreter::execution_step_count`
//! every limit check increments. Two counters for one fact, and the reported
//! one was the one that was always zero — so a 21,000-step fold and an empty
//! program described their work identically, and the ceiling an agent was
//! supposed to budget against could not be observed at all.

use crate::agent::api::ComputeOptions;
use crate::test_support::agent_json;
use serde_json::Value as Json;

/// The MCP profile's real `executionSteps` (100,000) is not
/// `Interpreter::new()`'s default any more — the playground and native-CLI
/// default was re-derived in the 2026-08-14 host-profile-derivation follow-up
/// and grew far past it (`runtime_limits::DEFAULT_MAX_EXECUTION_STEPS`). Every
/// real MCP call site threads its own 100,000 explicitly
/// (`tools/mcp-server/index.js` `LIMITS.executionSteps`), so these tests do the
/// same rather than relying on a default that no longer means "the MCP
/// profile".
const MCP_PROFILE: ComputeOptions = ComputeOptions::agent(Some(100_000));

fn steps(report: &Json) -> u64 {
    report["resourceUsage"]["executionSteps"]
        .as_u64()
        .expect("executionSteps is a number")
}

#[tokio::test]
async fn every_kind_of_execution_reports_the_steps_it_took() {
    // Each of these runs Words; none of them could ever report having done
    // so. A user-word call, a fold, and a fold through `MAP` cover the
    // dispatch routes of the engine.
    for source in [
        "2 3 DIV 1 3 DIV ADD",
        "[ [ 2 ] MUL ] 'DOUBLE' DEF [ 3 ] DOUBLE",
        "1 20 RANGE 1 [ MUL ] FOLD",
        "1 20 RANGE [ [ 2 ] MUL ] MAP",
    ] {
        let report = agent_json(source, MCP_PROFILE).await;
        assert!(
            steps(&report) > 0,
            "`{source}` executed Words and reported {} steps",
            steps(&report)
        );
    }
}

#[tokio::test]
async fn more_work_reports_more_steps() {
    // Not merely non-zero: the number has to move with the work, or it is
    // a constant dressed up as a measurement.
    let short = steps(&agent_json("1 10 RANGE 1 [ MUL ] FOLD", MCP_PROFILE).await);
    let long = steps(&agent_json("1 200 RANGE 1 [ MUL ] FOLD", MCP_PROFILE).await);
    assert!(
        long > short * 10,
        "twenty times the fold must cost far more steps, got {long} against {short}"
    );
}

#[tokio::test]
async fn the_reported_steps_are_the_ones_the_ceiling_judged() {
    // The point of a single counter: what a refusal says it observed and
    // what the report says was spent are the same reading. Nested so no
    // single `RANGE` call approaches `LOCAL_AGENT_RUNTIME_LIMITS`'
    // 100,000-element materialization ceiling (a different, smaller
    // budget that would otherwise fire first, as a cheap NIL, before the
    // step count this test means to exercise ever grows): 200 outer
    // iterations each folding 999 inner elements costs on the order of
    // 200,000 steps while every one `RANGE` ever produces stays at 999.
    // The outer block adds the element to the accumulator *before* the
    // inner fold, so it leaves the one value a block owes
    // (LANG.COLLECTIONS.HIGHER) rather than the accumulator and a sum.
    let report = agent_json(
        "1 200 RANGE 0 [ ADD 1 999 RANGE 0 [ ADD ] FOLD ADD ] FOLD",
        MCP_PROFILE,
    )
    .await;
    assert_eq!(report["status"], "error");
    let limit = MCP_PROFILE.step_limit.unwrap() as u64;
    assert_eq!(
        report["error"]["category"], "executionLimitExceeded",
        "the step ceiling must be what fired"
    );
    assert!(
        steps(&report) >= limit,
        "a run refused at {limit} steps must report having reached it, got {}",
        steps(&report)
    );
}

#[tokio::test]
async fn numeric_work_is_reported_beside_the_steps() {
    let report = agent_json("1 20 RANGE 1 [ MUL ] FOLD", MCP_PROFILE).await;
    assert!(
        report["resourceUsage"]["numericWork"]
            .as_u64()
            .expect("numericWork is a number")
            > 0,
        "a fold of twenty multiplications charged the meter and must say so"
    );
}

#[tokio::test]
async fn collection_work_is_reported_and_tracks_the_data_not_the_length() {
    // The property the whole collection meter exists for: the same Word
    // over the same number of elements charges for the *data*, not the
    // count read off the vector's length. Before the de-quadraticization
    // follow-up this meant 16,000 distinct values (a linear scan against
    // every distinct value found so far) cost 50x what 16,000 repeats of
    // one value cost, and was refused outright. `Value: Hash` replaced
    // that scan with an O(n)-average `HashMap` lookup, so distinctness no
    // longer dominates the price the way it did — both succeed now, and
    // the gap between them is only the cost of copying every new value
    // into the result rather than an O(n×distinct) amplification.
    let uniform = agent_json("0 15999 RANGE [ 0 MUL ] MAP UNIQUE LENGTH", MCP_PROFILE).await;
    assert_eq!(uniform["status"], "ok");
    let uniform_work = uniform["resourceUsage"]["collectionWork"]
        .as_u64()
        .expect("collectionWork is a number");
    assert!(
        uniform_work > 0,
        "a scan of 16,000 elements charged nothing"
    );

    let distinct = agent_json("0 15999 RANGE UNIQUE LENGTH", MCP_PROFILE).await;
    assert_eq!(distinct["status"], "ok");
    let distinct_work = distinct["resourceUsage"]["collectionWork"]
        .as_u64()
        .expect("collectionWork is a number");
    assert!(
        distinct_work > uniform_work,
        "16,000 distinct values still costs more than 16,000 repeats — \
         every one of them is new and gets copied into the result — but \
         {distinct_work} against {uniform_work} must at least move with \
         the data"
    );
    assert!(
        distinct_work < uniform_work * 3,
        "distinctness must no longer dominate the price the way the \
         O(n×distinct) linear scan did: {distinct_work} against {uniform_work}"
    );
}

#[tokio::test]
async fn a_program_that_does_nothing_reports_nothing() {
    // The other half of "the number moves with the work": zero has to be
    // reachable, or non-zero proves nothing.
    let report = agent_json("42", MCP_PROFILE).await;
    assert_eq!(report["status"], "ok");
    assert_eq!(steps(&report), 0, "a bare literal executes no Word");
    assert_eq!(report["resourceUsage"]["numericWork"], 0);
    assert_eq!(report["resourceUsage"]["collectionWork"], 0);
}

#[tokio::test]
async fn every_reported_usage_names_a_declared_ceiling() {
    // `resourceUsage` earns its separation from `runtimeMetrics` by this
    // property: each key is a budget the host declares, so an agent can
    // subtract. A key here with no ceiling behind it would be an optimizer
    // counter in the wrong object.
    let report = agent_json("1 20 RANGE 1 [ MUL ] FOLD", MCP_PROFILE).await;
    let usage = report["resourceUsage"]
        .as_object()
        .expect("resourceUsage is an object");
    for key in usage.keys() {
        assert!(
            matches!(
                key.as_str(),
                "executionSteps" | "numericWork" | "collectionWork"
            ),
            "`{key}` is reported as a resource but names no ceiling the \
             interpreter enforces; peaks for `bigintBits` and \
             `algebraicTerms` are checked per result and never accumulated, \
             so there is nothing to report for them yet"
        );
    }
}

#[tokio::test]
async fn a_budget_is_reported_once() {
    // `executionSteps` is a budget, so it lives in `resourceUsage` alone;
    // `runtimeMetrics` is optimizer counters and carries no second copy.
    let report = agent_json("1 20 RANGE 1 [ MUL ] FOLD", MCP_PROFILE).await;
    assert!(report["runtimeMetrics"].get("executionSteps").is_none());
    assert_eq!(report["resourceUsage"]["executionSteps"], 22);
}
