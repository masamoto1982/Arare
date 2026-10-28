//! Shared helpers for the crate's in-tree behavioral probes.
//!
//! Every `*_tests` module used to carry its own copy of "run this source on a
//! fresh interpreter and read the answer" — one decision spelled a dozen
//! ways. It is written once here. A helper with a genuinely different
//! contract (a rendered rather than displayed stack, a different work
//! counter, a `Result` the probe inspects itself) stays with the tests that
//! need it.

use crate::error::{ErrorCategory, NilReason};
use crate::interpreter::{Interpreter, RuntimeLimits};
use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;
use crate::types::{Value, ValueData};

/// A fresh interpreter with `limits` in force.
pub(crate) fn with_limits(limits: RuntimeLimits) -> Interpreter {
    let mut interp = Interpreter::new();
    interp.set_runtime_limits(limits);
    interp
}

/// Run `code` on a fresh interpreter; a failure to run is the probe's own bug.
pub(crate) async fn run(code: &str) -> Interpreter {
    let mut interp = Interpreter::new();
    interp
        .execute(code)
        .await
        .unwrap_or_else(|e| panic!("`{code}` must not error: {e}"));
    interp
}

/// [`run`], with the materialization ceiling raised to ten million elements
/// so a probe over a wide collection is not refused by the host safety
/// control before it reaches the Word under test.
pub(crate) async fn run_with_wide_materialization(code: &str) -> Interpreter {
    let mut interp = Interpreter::new();
    let mut limits = *interp.runtime_limits();
    limits.max_materialized_elements = 10_000_000;
    interp.set_runtime_limits(limits);
    interp
        .execute(code)
        .await
        .unwrap_or_else(|e| panic!("`{code}` must compute, got: {e:?}"));
    interp
}

/// The stack `code` leaves, bottom to top.
pub(crate) async fn run_ok(code: &str) -> Vec<Value> {
    run(code).await.get_stack().to_vec()
}

/// The value `code` leaves on top.
pub(crate) async fn top_of(code: &str) -> Value {
    run(code)
        .await
        .stack
        .last()
        .cloned()
        .expect("an answer was pushed")
}

/// The whole stack `code` leaves, each value displayed, space-joined bottom
/// to top — the form most Word probes compare against.
pub(crate) async fn top(code: &str) -> String {
    run(code)
        .await
        .get_stack()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The value `code` leaves on top, displayed; `<empty>` for an empty stack.
pub(crate) async fn answer(code: &str) -> String {
    run(code)
        .await
        .get_stack()
        .last()
        .map(|v| format!("{v}"))
        .unwrap_or_else(|| "<empty>".to_string())
}

/// The protocol id of the error category `code` raises.
pub(crate) async fn error_of(code: &str) -> String {
    let mut interp = Interpreter::new();
    let err = interp.execute(code).await.expect_err("must raise an ERROR");
    ErrorCategory::from_error(&err)
        .expect("a program ERROR has a category")
        .as_protocol_str()
        .to_string()
}

/// The protocol id of the reason on the NIL `code` projects, or `None` when
/// the NIL carries no reason. Anything but a NIL is the probe's own bug.
pub(crate) async fn reason(code: &str) -> Option<String> {
    let answer = top_of(code).await;
    assert!(answer.is_nil(), "`{code}` must project NIL, got {answer:?}");
    answer
        .nil_reason()
        .map(|reason| reason.as_protocol_str().to_string())
}

/// The reason on the NIL at the top of `interp`'s stack, if the top is a
/// reasoned NIL.
pub(crate) fn top_nil_reason(interp: &Interpreter) -> Option<NilReason> {
    interp
        .get_stack()
        .last()
        .and_then(|v| v.nil_reason().cloned())
}

/// Numeric work charged by running `code`.
pub(crate) async fn charged_by(code: &str) -> u64 {
    run(code).await.numeric_work_used()
}

/// The truth value at the bottom of `interp`'s stack, read off its display.
pub(crate) fn bool_of(interp: &Interpreter) -> bool {
    let v = &interp.get_stack()[0];
    let s = format!("{}", v);
    match s.as_str() {
        "1" | "1/1" | "TRUE" => true,
        "0" | "0/1" | "FALSE" => false,
        other => panic!("expected boolean (0 or 1), got {}", other),
    }
}

/// Whether the top value `code` leaves is a dense `Tensor`.
pub(crate) async fn top_is_dense(code: &str) -> bool {
    let interp = run_with_wide_materialization(code).await;
    matches!(
        interp.get_stack().as_slice().last().map(|v| &v.data),
        Some(ValueData::Tensor { .. })
    )
}

/// What `left right EQ` answers, or `None` when it answers a non-truth value.
pub(crate) async fn equals(left: &str, right: &str) -> Option<bool> {
    let interp = run_with_wide_materialization(&format!("{left} {right} EQ")).await;
    interp.get_stack().as_slice().last()?.as_truth()
}

/// `n/d` as a `Fraction`.
pub(crate) fn frac(n: i64, d: i64) -> Fraction {
    Fraction::new(n.into(), d.into())
}

/// The value's `Hash`, through the standard library's default hasher — the
/// hasher `UNIQUE` / `GROUP` / `TALLY` bucket by.
pub(crate) fn hash_of(value: &Value) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// `√n` as a value, built without a parser.
pub(crate) fn sqrt_of(n: i64) -> Value {
    Value::from_exact_real(
        ExactReal::from_sqrt_rational(frac(n, 1)).expect("a non-negative radicand"),
    )
}

/// A numeric scalar as an exact real, whichever tier holds it.
pub(crate) fn as_exact(value: &Value) -> ExactReal {
    match &value.data {
        ValueData::ExactScalar(e) => (**e).clone(),
        ValueData::Scalar(f) => ExactReal::from_fraction(f.clone()),
        other => panic!("not an exact scalar: {other:?}"),
    }
}

/// `x MUL y` over the exact tier, for building `2·√3` without a parser.
pub(crate) fn exact_mul(left: &Value, right: &Value) -> Value {
    Value::from_exact_real(as_exact(left).mul(&as_exact(right)))
}

/// `x ADD y` over the exact tier.
pub(crate) fn exact_add(left: &Value, right: &Value) -> Value {
    Value::from_exact_real(as_exact(left).add(&as_exact(right)))
}

/// One `agent compute` run's JSON envelope under `options`.
#[cfg(feature = "std")]
pub(crate) async fn agent_json(
    source: &str,
    options: crate::agent::api::ComputeOptions,
) -> serde_json::Value {
    crate::agent::api::compute(source, options).await.to_json()
}

/// How `values` are held, for tests that hold two routes to one
/// representation: a dense Tensor and a Vector of the same numbers are told
/// apart, but a truth column reads as the Vector it stands for — it is the
/// only form a route may answer a mask in, and never nested (`truth_column`).
pub(crate) fn representation(values: &[Value]) -> String {
    let values: Vec<Value> = values
        .iter()
        .map(|value| match &value.data {
            ValueData::Truths(column) => column.to_vector(),
            _ => value.clone(),
        })
        .collect();
    format!("{values:?}")
}
