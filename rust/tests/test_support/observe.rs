//! Firewall-clean observation (Phase 1).
//!
//! The formalization §8 defines the observation function
//! `observe(p) = (render(π_Stack ⟦p⟧ σ₀), π_Eff ⟦p⟧ σ₀)`. This module gives the
//! data-plane half of that: it reads a value **only** through the LANG.OBSERVATION.FIREWALL
//! semantic axes (`semanticKind`, `shape`, `capabilities`, `truthValue`,
//! `origin`, `absence`) and through the pure renderer `render : value →
//! display`. It never branches on a Rust enum name, `Debug` string, or display
//! text — the semantic-firewall discipline the roadmap §1.2-3 mandates.
//!
//! Later laws also use `observe_program` from this module to compare the
//! effect trace and error category alongside the data-plane axes, keeping
//! surface/canonical equivalence checks structured rather than string-fragment
//! based.
//!
//! Shared test scaffolding: each law binary observes a different subset of the
//! axes, so any single binary sees the rest as dead code. Allow it module-wide
//! rather than annotating every accessor.
#![allow(dead_code)]

use ajisai_core::interpreter::Interpreter;
use ajisai_core::types::Value;
use ajisai_core::ErrorCategory;

/// The pure renderer `render : value → display` (LANG.OBSERVATION.PROTOCOL,
/// LANG.VALUES.DENOTATION). Exposed as a named function so laws read as
/// equations over `render`.
pub fn render(v: &Value) -> String {
    v.to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsenceObservation {
    pub reason: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisObservation {
    pub truth_value: Option<&'static str>,
    pub absence: Option<AbsenceObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueObservation {
    pub render: String,
    pub axes: AxisObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramObservation {
    pub stack: Vec<ValueObservation>,
    pub effects: Vec<(String, String)>,
    pub error_category: Option<&'static str>,
}

/// Observe a value through the semantic axes only (firewall-clean).
pub fn observe_axes(v: &Value) -> AxisObservation {
    let absence = v.absence_metadata().map(|absence| AbsenceObservation {
        reason: absence
            .reason
            .as_ref()
            .map(|reason| reason.as_protocol_str()),
    });
    AxisObservation {
        truth_value: v.truth_value(),
        absence,
    }
}

/// Observe a value as the structured stack payload used by law tests: stable
/// render text plus protocol-level semantic axes, including absence
/// metadata when present.
pub fn observe_value(v: &Value) -> ValueObservation {
    ValueObservation {
        render: render(v),
        axes: observe_axes(v),
    }
}

/// Run a program and capture the structured observation fields needed by
/// surface/canonical equivalence laws: stack values, effect trace, and error
/// category. Human-readable error messages are deliberately not observed.
pub fn observe_program(src: &str) -> ProgramObservation {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio current-thread runtime");
    rt.block_on(async {
        let mut interp = Interpreter::new();
        let error_category = match interp.execute(src).await {
            Ok(()) => None,
            Err(err) => ErrorCategory::from_error(&err).map(|category| category.as_protocol_str()),
        };
        ProgramObservation {
            stack: interp.get_stack().iter().map(observe_value).collect(),
            effects: interp
                .host_effects()
                .iter()
                .map(|effect| (effect.kind().to_string(), effect.payload().to_string()))
                .collect(),
            error_category,
        }
    })
}

/// Run an Ajisai program and return the final stack. Panics on execution error
/// so a malformed law program is loud rather than silently skipped (mirrors the
/// existing `algebraic_laws.rs` harness).
/// Run a program expected to fail, returning the error text. Laws that pin a
/// rejection (a domain a Word does not accept) need the message, not a panic.
pub fn run_err(src: &str) -> String {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio current-thread runtime");
    rt.block_on(async {
        let mut interp = Interpreter::new();
        match interp.execute(src).await {
            Ok(()) => panic!("program was expected to fail: {src:?}"),
            Err(e) => e.to_string(),
        }
    })
}

pub fn run(src: &str) -> Vec<Value> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio current-thread runtime");
    rt.block_on(async {
        let mut interp = Interpreter::new();
        let program = src;
        interp
            .execute(program)
            .await
            .unwrap_or_else(|e| panic!("program failed: {src:?}: {e}"));
        interp.get_stack().to_vec()
    })
}

/// Run a program expected to leave exactly one value, returning that value.
pub fn run_one(src: &str) -> Value {
    let stack = run(src);
    assert_eq!(
        stack.len(),
        1,
        "generator program {src:?} must leave exactly one value, got {}",
        stack.len()
    );
    stack.into_iter().next().unwrap()
}
