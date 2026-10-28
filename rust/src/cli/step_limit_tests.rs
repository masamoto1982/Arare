//! Native tests for `ajisai run --step-limit <N>`: the host-configurable
//! execution step budget (water level, LANG.MACHINE.LIMITS). The budget
//! is a runtime safety control, not language semantics, so these tests only
//! assert *whether* `ExecutionLimitExceeded` is raised — raising the budget
//! lets a legitimately large `FOLD` run to completion, lowering it sandboxes
//! even a short program — plus the CLI usage-error contract (exit 2 for zero
//! / non-numeric values). Deliberately not part of the conformance suite:
//! conformance must not depend on any budget value.

use crate::error::AjisaiError;
use crate::interpreter::{Interpreter, DEFAULT_MAX_EXECUTION_STEPS};

/// A `FOLD` over a generated range: `RANGE` materializes its elements
/// internally and so counts as a single step regardless of length (SPEC
/// LANG.MACHINE.LIMITS), while `FOLD` dispatches its body once per element — a bounded,
/// non-recursive way to spend a large, roughly-known number of steps. Kept
/// as the fixed probe for the "raised limit" direction below, where the
/// point is that an explicit `--step-limit` still works, not that it is
/// large.
const FOLD_PROBE: &str = "1 200000 RANGE 0 [ ADD ] FOLD";

/// Write `source` to a unique temp file and return its path.
fn write_program(name: &str, source: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ajisai-step-limit-{}-{}.ajisai",
        std::process::id(),
        name
    ));
    std::fs::write(&path, source).expect("temp program must be writable");
    path
}

fn run_cli(args: &[&str]) -> i32 {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    super::run(&args)
}

/// `ajisai run` with no `--step-limit` must actually apply the derived
/// default, through the CLI, not merely have a constant that says so.
///
/// This replaced an assertion that compared `Interpreter::new()
/// .max_execution_steps()` to `DEFAULT_MAX_EXECUTION_STEPS` — which the
/// constructor initializes that field *from* (`interpreter_core.rs`), so it
/// could never fail, and which never invoked the CLI at all despite saying it
/// checked `ajisai run`.
///
/// `FOLD_PROBE` costs on the order of 200,000 steps: comfortably past the
/// 100,000 the default used to be, and a rounding error against what it is
/// now. So a clean exit here is a real end-to-end statement — the CLI
/// reached the interpreter default, and that default is at least the old
/// budget — while staying a fast test. What it deliberately does *not* claim
/// is that the budget is enforced at its exact declared value; only
/// `down_probe_exceeds_the_real_default_budget_without_step_limit` below can
/// say that, and it costs real wall time by construction, so it is
/// `#[ignore]`d.
#[test]
fn down_probe_runs_under_the_default_budget_without_step_limit() {
    let path = write_program("default", FOLD_PROBE);
    let code = run_cli(&["run", path.to_str().unwrap()]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        code, 0,
        "folding 200000 elements costs on the order of 200,000 steps and must \
         complete under the derived default budget of \
         {DEFAULT_MAX_EXECUTION_STEPS}, with no --step-limit given"
    );
}

#[test]
fn down_probe_succeeds_with_an_explicit_step_limit() {
    // 1,000,000 is no longer a *raised* limit — `DEFAULT_MAX_EXECUTION_STEPS`
    // is far larger now — but the point of this test was always that an
    // explicit `--step-limit` is honored, not that the number is big.
    let path = write_program("explicit", FOLD_PROBE);
    let code = run_cli(&["run", path.to_str().unwrap(), "--step-limit", "1000000"]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        code, 0,
        "folding 200000 elements must complete under an explicit --step-limit 1000000"
    );
}

/// The real, end-to-end version of the test above: a `FOLD` over just past
/// `DEFAULT_MAX_EXECUTION_STEPS` elements, with no `--step-limit`, must still
/// exceed the real default. At the measured floor of ~773-890 steps/ms
/// (release; far slower in the debug build this normally runs under) this
/// takes on the order of a minute, so it is `#[ignore]`d rather than run on
/// every `cargo test`. Run deliberately with `cargo test -- --ignored` after
/// touching `DEFAULT_MAX_EXECUTION_STEPS` or the interpreter's dispatch path.
#[test]
#[ignore = "exhausting the real default costs real wall time by construction; run explicitly"]
fn down_probe_exceeds_the_real_default_budget_without_step_limit() {
    // Generous margin over one step per element: `RANGE` itself is a single
    // step regardless of length, and `FOLD` costs at least one dispatch per
    // element, so `elements + elements / 10` comfortably clears the real
    // default even if per-element overhead is a small multiple of one step.
    let elements = DEFAULT_MAX_EXECUTION_STEPS + DEFAULT_MAX_EXECUTION_STEPS / 10;
    let probe = format!("1 {elements} RANGE 0 [ ADD ] FOLD");
    let path = write_program("real-default", &probe);
    let code = run_cli(&["run", path.to_str().unwrap()]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        code, 1,
        "folding {elements} elements must exceed the real default budget of \
         {DEFAULT_MAX_EXECUTION_STEPS} steps"
    );
}

/// Twelve word executions (a step counts a *word* execution, not a literal),
/// so this trips a 10-step budget but is far below the default either way.
const SIMPLE_PROGRAM: &str = "[ 1 ] [ 1 ] ADD [ 1 ] ADD [ 1 ] ADD [ 1 ] ADD [ 1 ] ADD [ 1 ] ADD \
     [ 1 ] ADD [ 1 ] ADD [ 1 ] ADD [ 1 ] ADD [ 1 ] ADD [ 1 ] ADD";

#[test]
fn lowered_step_limit_sandboxes_a_simple_program() {
    let path = write_program("lowered", SIMPLE_PROGRAM);
    let code = run_cli(&["run", path.to_str().unwrap(), "--step-limit", "10"]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 1, "a lowered budget must stop even a simple program");
}

/// The lowered budget must fail with `ExecutionLimitExceeded` specifically
/// (the sandbox use case), not some other error the CLI exit code would mask.
#[test]
fn lowered_budget_raises_execution_limit_exceeded() {
    let mut interp = Interpreter::new();
    interp.set_max_execution_steps(10);
    let err = crate::agent::block_on(interp.execute(SIMPLE_PROGRAM))
        .expect_err("a 10-step budget must stop this program");
    assert!(
        matches!(err, AjisaiError::ExecutionLimitExceeded { limit: 10 }),
        "expected ExecutionLimitExceeded {{ limit: 10 }}, got: {err}"
    );
}

#[test]
fn step_limit_zero_is_a_usage_error() {
    let path = write_program("zero", "[ 1 ]");
    let code = run_cli(&["run", path.to_str().unwrap(), "--step-limit", "0"]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 2);
}

#[test]
fn step_limit_non_numeric_is_a_usage_error() {
    let path = write_program("nonnum", "[ 1 ]");
    let code = run_cli(&["run", path.to_str().unwrap(), "--step-limit", "many"]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 2);
}

#[test]
fn step_limit_missing_value_is_a_usage_error() {
    let path = write_program("missing", "[ 1 ]");
    let code = run_cli(&["run", path.to_str().unwrap(), "--step-limit"]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 2);
}
