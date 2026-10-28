//! Property-based observation-foundation laws (Phase 1).
//!
//! These encode the algebraic content of the observation function and the
//! renderer (Phase 1): `observe(p) = (render(π_Stack ⟦p⟧ σ₀), π_Eff)` with
//! `render : value → display` a **pure** function of the value
//! (LANG.VALUES.DENOTATION), observed through the LANG.OBSERVATION.FIREWALL
//! semantic axes only.
//!
//! Unlike `algebraic_laws.rs` — which observes through whole-stack
//! `Value::to_string()` (a *display* surface, non-canonical per LANG.OBSERVATION.FIREWALL) — this
//! file observes through protocol axes and treats `render` as an explicit
//! function of the value. It is the firewall-clean basis later phases reuse
//! by adding domain generators (`test_support::generators`).
//!
//! Every law below was checked against the reference implementation with a
//! throwaway probe before being written (roadmap §1.2-(T) discipline).

mod test_support;

use proptest::prelude::*;
use test_support::generators::*;
use test_support::observe::{observe_axes, render, run, run_one};

// ─────────────────────────── concrete-witness laws ───────────────────────────

/// Finding B at the observation layer: a truth value is observably **not** a
/// number. `TRUE` carries the `truthValue` axis; the scalar `1` does not, and
/// they render differently.
#[test]
fn truth_value_is_observably_not_a_number() {
    let t = observe_axes(&run_one("TRUE"));
    let one = observe_axes(&run_one("1"));
    assert_eq!(t.truth_value, Some("true"));
    assert_eq!(one.truth_value, None);
    assert_ne!(render(&run_one("TRUE")), render(&run_one("1")));
}
/// Every observed protocol string is canonical lower-camelCase (LANG.OBSERVATION.FIREWALL):
/// nonempty, lowercase first letter, ASCII-alphanumeric only (no `_`, no `-`).
#[test]
fn protocol_strings_are_lower_camel_case() {
    fn ok(s: &str) -> bool {
        let mut chars = s.chars();
        matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
            && s.chars().all(|c| c.is_ascii_alphanumeric())
    }
    for src in [
        "5",
        "TRUE",
        "FALSE",
        "1 0 DIV",
        "[ 1 2 3 ]",
        "[ 1 ADD ]",
        "2 SQRT",
    ] {
        for v in run(src) {
            let o = observe_axes(&v);
            if let Some(absence) = &o.absence {
                if let Some(reason) = absence.reason {
                    assert!(ok(reason), "reason {reason:?}");
                }
            }
            if let Some(tv) = o.truth_value {
                assert!(ok(tv), "truthValue {tv:?}");
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    // ───────────────────────── render is a pure function ─────────────────────

    /// **Totality + determinism**: `render` is defined for every well-formed
    /// value and is a deterministic pure function — two calls agree.
    #[test]
    fn render_total_and_deterministic(src in any_value_src()) {
        let v = run_one(&src);
        prop_assert_eq!(render(&v), render(&v));
    }

    // ───────────────────────── semantic firewall on the axes ─────────────────

    /// **The truth axis is the Boolean domain** (LANG.VALUES.TRUTH): a value
    /// reports `truthValue` exactly when it is a Boolean. UNKNOWN is a NIL and
    /// reports none.
    #[test]
    fn truth_axis_is_present_exactly_on_booleans(src in any_value_src()) {
        let v = run_one(&src);
        let is_boolean = matches!(v.data, ajisai_core::types::ValueData::Boolean(_));
        prop_assert_eq!(v.truth_value().is_some(), is_boolean);
    }
}
