//! Tests for the typed, source-only agent API (`crate::agent::api`).

use crate::agent::api::*;
use crate::interpreter::RuntimeLimits;

#[tokio::test]
async fn compute_is_source_only_and_returns_the_cli_envelope() {
    let response = compute("[ 2 ] SQRT", ComputeOptions::default()).await;
    let json = response.to_json();
    assert_eq!(response.exit_code(), 0);
    assert_eq!(json["status"], "ok");
    assert_eq!(
        json["stack"][0]["value"][0]["semantics"]["exactTerms"][0]["radicand"],
        "2"
    );
}

#[tokio::test]
async fn compute_preserves_structured_language_errors() {
    let response = compute("FROBNICATE", ComputeOptions::default()).await;
    let json = response.to_json();
    assert_eq!(response.exit_code(), 1);
    assert_eq!(json["status"], "error");
    assert_eq!(json["error"]["category"], "unknownWord");
    assert_eq!(json["error"]["word"], "FROBNICATE");
    assert_eq!(json["error"]["line"], 1);
}

#[tokio::test]
async fn compute_applies_injected_internal_cost_limits() {
    let response = compute(
        "0 11 RANGE",
        ComputeOptions {
            runtime_limits: Some(RuntimeLimits {
                max_materialized_elements: 10,
                ..RuntimeLimits::default()
            }),
            ..ComputeOptions::default()
        },
    )
    .await;
    let json = response.to_json();
    assert_eq!(json["status"], "error");
    assert_eq!(json["outcome"], "error:resourceLimitExceeded");
    assert_eq!(json["error"]["category"], "resourceLimitExceeded");
    assert_eq!(json["error"]["resource"], "materializedElements");
}

/// `outcome` names a run in `outcomes`' own vocabulary — a value, a NIL
/// by its reason, an error by its category — so a prediction and a run
/// compare by membership. `check` never runs, so it names none.
#[tokio::test]
async fn compute_names_the_outcome_id_it_produced() {
    for (source, expected) in [
        ("1 2 ADD", "value"),
        ("", "value"),
        ("-1 SQRT", "nil:domainMiss"),
        ("1 0 DIV", "value"),
        ("FROBNICATE", "error:unknownWord"),
        ("[ 1 2", "error:malformedSource"),
    ] {
        let json = compute(source, ComputeOptions::default()).await.to_json();
        assert_eq!(json["outcome"], expected, "{source}");
    }
    assert!(check("1 2 ADD").to_json().get("outcome").is_none());
}

#[test]
fn check_is_execution_free_and_structured() {
    let response = check("[ [ 1 ] ADD ] 'INC' DEF 'must-not-print' PRINT");
    let json = response.to_json();
    assert_eq!(response.exit_code(), 0);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["output"], serde_json::json!([]));
}

#[test]
fn infer_contracts_returns_a_common_agent_envelope() {
    let response = infer_contracts("[ [ 1 ] ADD ] 'INC' DEF").to_json();
    assert_eq!(response["status"], "ok");
    assert_eq!(response["contracts"][0]["name"], "INC");
}

/// Source that does not read is `malformedSource` to every execution-free
/// operation, exactly as `check` reports it — not an empty success.
#[test]
fn infer_contracts_reports_malformed_source_as_check_does() {
    for source in ["[ 1 2", "1 2 ]", "'unterminated"] {
        let inferred = infer_contracts(source);
        let checked = check(source);
        let inferred_json = inferred.to_json();
        assert_eq!(inferred_json["status"], "error", "{source}");
        assert_eq!(
            inferred_json["error"]["category"], "malformedSource",
            "{source}"
        );
        assert_eq!(inferred_json, checked.to_json(), "{source}");
        assert_eq!(inferred.exit_code(), checked.exit_code(), "{source}");
    }
}

/// A body naming a Word nothing defines has no arity the walk can state.
#[test]
fn an_unresolved_word_leaves_the_arity_variable() {
    let response = infer_contracts("[ FOO ] 'W' DEF").to_json();
    let contract = &response["contracts"][0];
    assert_eq!(contract["name"], "W");
    assert_eq!(contract["inputs"], "variable");
    let resolved = infer_contracts("[ 1 ADD ] 'W' DEF").to_json();
    assert_eq!(resolved["contracts"][0]["inputs"], 1);
    assert_eq!(
        resolved["contracts"][0]["suggested"],
        "#:contract W inputs=1 outputs=1 field=closed"
    );
}
