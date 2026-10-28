//! Tests for `nil_diagnostics`: the diagnostic absence accessors
//! (LANG.VALUES.NIL / LANG.OBSERVATION.DIAGNOSIS) `NIL?` and `NIL-REASON`
//! here.
//!
//! Coverage follows the §15 discipline: success paths, the non-NIL path, the
//! reason-present vs reason-absent split, protocol-string (not Rust `Debug`)
//! output, the U firewall, operand consumption, and MC/DC over the two governing
//! decisions (`is_nil` and reason `Some`/`None`).

use crate::error::NilReason;
use crate::interpreter::value_extraction_helpers::value_as_string;
use crate::interpreter::Interpreter;
use crate::test_support::run;

/// The top-of-stack value, as a protocol string, or `None` when it is NIL.
fn top_text(interp: &Interpreter) -> Option<String> {
    let top = interp.get_stack().last().expect("stack must be non-empty");
    if top.is_nil() {
        return None;
    }
    Some(value_as_string(top).expect("top must be a Text value"))
}

fn top_is_nil(interp: &Interpreter) -> bool {
    interp
        .get_stack()
        .last()
        .map(|v| v.is_nil())
        .unwrap_or(false)
}

fn top_is_true(interp: &Interpreter) -> bool {
    interp
        .get_stack()
        .last()
        .and_then(|v| v.as_truth())
        .unwrap_or(false)
}
// ── NIL? ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn nil_check_is_true_for_operational_nil_and_consumes_it() {
    let interp = run("-1 SQRT NIL?").await;
    let stack = interp.get_stack();
    assert_eq!(stack.len(), 1, "NIL? consumes the inspected value");
    assert!(top_is_true(&interp), "NIL? on an operational NIL is TRUE");
}

#[tokio::test]
async fn nil_check_is_false_for_present_value() {
    let interp = run("5 NIL?").await;
    let stack = interp.get_stack();
    assert_eq!(stack.len(), 1, "NIL? consumes the inspected value");
    assert_eq!(
        stack[0].as_truth(),
        Some(false),
        "NIL? on a present value is FALSE"
    );
}

/// The logical Unknown (U) is a NIL read in truth position
/// (LANG.VALUES.TRUTH), so it is an absence and `NIL?` answers TRUE — the
/// same answer a chosen fallback acts on, and the same story the host protocol
/// tells about it (`type: "nil"`, a published `absence.reason`).
///
/// `TRUE NIL AND` is the strong-Kleene UNKNOWN row (neither operand absorbs
/// the other), so it produces a genuine U — `AND`/`NOT` are what makes
/// U reachable from source at all.
#[tokio::test]
async fn nil_check_is_true_for_logical_unknown() {
    let interp = run("TRUE NIL AND NIL?").await;
    assert_eq!(
        interp.get_stack()[0].as_truth(),
        Some(true),
        "NIL? on the logical Unknown must be TRUE: U is an absence"
    );
}

/// A reason survives being read in truth position. `AND` used to swallow it:
/// `'x' NUM TRUE AND NIL-REASON` answered that it had no reason while the
/// protocol still published `absence.reason = invalidEncoding` for that value,
/// so the language contradicted its own boundary and LANG.VALUES.NIL ("the
/// reason is the entire observable content of a NIL").
#[tokio::test]
async fn nil_reason_survives_a_kleene_word() {
    let interp = run("'x' NUM TRUE AND NIL-REASON").await;
    assert_eq!(
        top_text(&interp).as_deref(),
        Some("invalidEncoding"),
        "an UNKNOWN must keep the reason it arrived with"
    );
}

// ── NIL-REASON ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn nil_reason_reports_domain_miss_protocol_string() {
    let interp = run("-1 SQRT NIL-REASON").await;
    let stack = interp.get_stack();
    assert_eq!(stack.len(), 1, "NIL-REASON consumes the inspected value");
    assert_eq!(
        top_text(&interp).as_deref(),
        Some("domainMiss"),
        "NIL-REASON must be the lowerCamelCase protocol string, not a Debug name"
    );
}

/// The output must be the protocol string, never the Rust `Debug` rendering of
/// the `NilReason` enum (`DomainMiss`).
#[tokio::test]
async fn nil_reason_is_protocol_string_not_debug_name() {
    let interp = run("-1 SQRT NIL-REASON").await;
    let text = top_text(&interp).expect("reason must be Text");
    assert_eq!(text, "domainMiss");
    assert_ne!(text, format!("{:?}", NilReason::DomainMiss));
}

#[tokio::test]
async fn nil_reason_reports_index_out_of_bounds() {
    let interp = run("[ 1 2 3 ] 9 GET NIL-REASON").await;
    assert_eq!(top_text(&interp).as_deref(), Some("indexOutOfBounds"));
}

/// A written NIL reads back as `literal`. Every NIL carries a reason now, so
/// the reason-`None` branch of `NIL-REASON` is no longer reachable through a
/// literal; the branch itself is covered by `nil_reason_is_nil_for_present_value`
/// below, where the subject is not an operational NIL at all.
#[tokio::test]
async fn nil_reason_of_a_written_nil_is_literal() {
    let interp = run("NIL NIL-REASON").await;
    assert_eq!(top_text(&interp).as_deref(), Some("literal"));
}

/// The non-NIL path: `NIL-REASON` on a present value yields NIL, not an error.
#[tokio::test]
async fn nil_reason_is_nil_for_present_value() {
    let interp = run("5 NIL-REASON").await;
    assert!(top_is_nil(&interp));
    assert_eq!(interp.get_stack().len(), 1, "the 5 is consumed");
}

/// `NIL-REASON` on the result of an exact-arithmetic comparison must yield
/// NIL, never a reason — same as `nil_reason_is_nil_for_present_value`
/// above. Exact comparisons always decide (LANG.VALUES.EXACT), so `2 SQRT 1
/// ADD` compared against itself resolves to a definite `TRUE`.
#[tokio::test]
async fn nil_reason_is_nil_for_a_decidable_exact_comparison() {
    let interp = run("2 SQRT 1 ADD 2 SQRT 1 ADD SUB 0 EQ NIL-REASON").await;
    assert!(
        top_is_nil(&interp),
        "NIL-REASON on a non-NIL value must be NIL, never a reason string"
    );
}
// ── Domain miss (LANG.FAILURE.PROJECT: "SQRT of a negative rational is a well-formed
//    domain miss") ────────────────────────────────────────────────────────────
/// A quotient by zero is a number, not an absence (LANG.VALUES.EXACT), so
/// `NIL-REASON` asked of it is asked of a value outside its domain and
/// projects `domainMiss` like any other non-NIL operand.
#[tokio::test]
async fn a_quotient_by_zero_is_a_number_not_an_absence() {
    let interp = run("1 0 DIV NIL?").await;
    assert!(!top_is_true(&interp), "`1 0 DIV` is the number 1/0");
    let interp = run("1 0 DIV NIL-REASON NIL-REASON").await;
    assert_eq!(top_text(&interp).as_deref(), Some("domainMiss"));
}
