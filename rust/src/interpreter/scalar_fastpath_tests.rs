//! Differential coverage for the D1 scalar-scalar arithmetic/comparison fast
//! path. The fast path is observational only: with it enabled or disabled, the
//! stack values, rendered forms, and per-value hints must be identical.

use crate::agent::block_on;
use crate::interpreter::{arithmetic, comparison, Interpreter};
use crate::types::Value;

fn run(src: &str, enabled: bool) -> Interpreter {
    let mut interp = Interpreter::new();
    interp.set_scalar_fastpath_enabled(enabled);
    block_on(interp.execute(src)).unwrap();
    interp
}

fn run_direct(
    stack: Vec<Value>,
    enabled: bool,
    op: fn(&mut Interpreter) -> crate::error::Result<()>,
) -> Interpreter {
    let mut interp = Interpreter::new();
    interp.set_scalar_fastpath_enabled(enabled);
    interp.update_stack(stack);
    op(&mut interp).unwrap();
    interp
}

fn singleton_vector(n: i64) -> Value {
    Value::from_vector(vec![Value::from_int(n)])
}

fn one_char_string(n: i64) -> Value {
    Value::from_string(
        &char::from_u32(n as u32)
            .expect("test codepoint")
            .to_string(),
    )
}

fn rendered_stack(interp: &Interpreter) -> Vec<String> {
    interp
        .get_stack()
        .iter()
        .map(|value| format!("{value}"))
        .collect()
}

/// `f`'s result and how many scalar fast-path answers it gave.
fn counting<T>(f: impl FnOnce() -> T) -> (T, u64) {
    let before = super::arithmetic::scalar_fastpath_hits_on_this_thread();
    let value = f();
    (
        value,
        super::arithmetic::scalar_fastpath_hits_on_this_thread() - before,
    )
}

fn assert_on_equals_off(src: &str) -> (Interpreter, Interpreter, u64, u64) {
    let (on, on_hits) = counting(|| run(src, true));
    let (off, off_hits) = counting(|| run(src, false));
    assert_eq!(
        format!("{:?}", on.get_stack()),
        format!("{:?}", off.get_stack()),
        "fast path ON vs OFF stack diverged for: {src}"
    );
    assert_eq!(
        rendered_stack(&on),
        rendered_stack(&off),
        "fast path ON vs OFF render diverged for: {src}"
    );
    (on, off, on_hits, off_hits)
}

fn assert_direct_on_equals_off(
    stack: Vec<Value>,
    op: fn(&mut Interpreter) -> crate::error::Result<()>,
) -> (Interpreter, Interpreter, u64, u64) {
    let (on, on_hits) = counting(|| run_direct(stack.clone(), true, op));
    let (off, off_hits) = counting(|| run_direct(stack, false, op));
    assert_eq!(
        format!("{:?}", on.get_stack()),
        format!("{:?}", off.get_stack()),
        "direct fast path ON vs OFF stack diverged"
    );
    assert_eq!(
        rendered_stack(&on),
        rendered_stack(&off),
        "direct fast path ON vs OFF render diverged"
    );
    (on, off, on_hits, off_hits)
}

#[test]
fn arithmetic_fast_path_matches_baseline_for_bare_scalars_and_singleton_tensors() {
    for src in [
        "2 3 ADD",
        "7 4 SUB",
        "6 5 MUL",
        "6 4 DIV",
        "[ 1 ] [ 2 ] ADD",
        "[ 7 ] [ 4 ] SUB",
        "[ 6 ] [ 5 ] MUL",
        "[ 6 ] [ 4 ] DIV",
    ] {
        let (_on, _off, on_hits, off_hits) = assert_on_equals_off(src);
        assert!(on_hits >= 1, "expected scalar fast path to fire for: {src}");
        assert_eq!(
            off_hits, 0,
            "disabled scalar fast path should not count for: {src}"
        );
    }
}

#[test]
fn fast_path_preserves_tensor_wrapping() {
    let (on, _, _, _) = assert_on_equals_off("[ 1 ] [ 2 ] ADD");
    let rendered = rendered_stack(&on);
    assert_eq!(rendered, vec!["[ 3/1 ]"]);
}

#[test]
fn unsupported_or_semantically_sensitive_shapes_fall_back() {
    for src in ["2 [ 3 ] ADD", "[ 2 ] 3 ADD", "NIL 3 ADD", "3 NIL GT"] {
        let (_on, _off, on_hits, off_hits) = assert_on_equals_off(src);
        assert_eq!(on_hits, 0, "fast path should fall back for: {src}");
        assert_eq!(off_hits, 0);
    }
}

#[test]
fn direct_singleton_vector_fast_path_matches_baseline() {
    // Arithmetic only. The comparison family no longer has a singleton fast
    // path: a one-element Vector is not its element (LANG.VALUES.DISJOINT), so
    // `[ 6 ] [ 3 ] LT` lifts element-wise to `[ FALSE ]` rather than deciding
    // as a scalar. See `comparison_on_singletons_lifts_instead_of_fast_pathing`.
    for op in [
        arithmetic::op_add as fn(&mut Interpreter) -> crate::error::Result<()>,
        arithmetic::op_sub,
        arithmetic::op_mul,
        arithmetic::op_div,
    ] {
        let (_on, _off, on_hits, off_hits) =
            assert_direct_on_equals_off(vec![singleton_vector(6), singleton_vector(3)], op);
        assert!(on_hits >= 1, "expected singleton Vector fast path to fire");
        assert_eq!(off_hits, 0);
    }
}

#[test]
fn string_operand_stays_on_baseline_path() {
    // A String has no numeric lane at all now, so the fast path cannot see a
    // singleton to project — and arithmetic rejects it outright rather than
    // adding 1 to a codepoint. It used to be excluded by a hint check, and
    // `'A' 1 ADD` used to answer `[ 66/1 ]`.
    for enabled in [true, false] {
        let mut interp = Interpreter::new();
        interp.set_scalar_fastpath_enabled(enabled);
        interp.update_stack(vec![one_char_string(65), one_char_string(66)]);
        let (rejected, hits) = counting(|| arithmetic::op_add(&mut interp).is_err());
        assert!(
            rejected,
            "ADD must reject String operands (fast path {enabled})"
        );
        assert_eq!(
            hits, 0,
            "a String must not take the numeric scalar fast path"
        );
    }
}

#[test]
fn division_by_zero_matches_baseline() {
    let (_, _, on_hits, _) = assert_on_equals_off("6 0 DIV");
    assert!(
        on_hits >= 1,
        "division by zero still uses the scalar fast path to produce the same projected NIL"
    );
}

#[test]
fn comparison_on_singletons_lifts_instead_of_fast_pathing() {
    // The fast path used to project a one-element Vector to its element and
    // answer a bare Boolean, so `[ 6 ] [ 3 ] LT` was `FALSE` — a collapse
    // LANG.COLLECTIONS.LIFT forbids. It now lifts, which is a shape the fast
    // path deliberately declines.
    for enabled in [true, false] {
        let mut interp = Interpreter::new();
        interp.set_scalar_fastpath_enabled(enabled);
        interp.update_stack(vec![singleton_vector(6), singleton_vector(3)]);
        let (lifted, hits) = counting(|| comparison::op_lt(&mut interp));
        lifted.expect("LT lifts over singletons");
        assert_eq!(
            hits, 0,
            "a shaped operand must not take the scalar fast path (enabled={enabled})"
        );
        let result = interp.get_stack().last().expect("a result").clone();
        assert_eq!(result.len(), 1, "the lift preserves the operand shape");
        assert_eq!(
            result.child(0).and_then(|c| c.as_truth()),
            Some(false),
            "6 < 3 is false in the single lane"
        );
    }
}
