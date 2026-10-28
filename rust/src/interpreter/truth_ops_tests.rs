//! The column route of `NOT`, `AND` and `SELECT` (`truth_ops`) against the
//! lifted law it stands in for: every operand drawn as a column, as a Vector
//! of `Value`s or as one truth value, and the two answers compared lane by
//! lane — each NIL's whole absence included, not only its reason.

use super::*;
use crate::error::NilReason;
use crate::types::TruthColumn;
use proptest::prelude::*;

/// The lane every index names: the two Booleans and two different NILs.
fn lane(index: u8) -> Value {
    let unknown = |reason| Value::nil_with_reason(reason);
    match index % 4 {
        0 => Value::from_bool(false),
        1 => Value::from_bool(true),
        2 => unknown(NilReason::DomainMiss),
        _ => unknown(NilReason::NotFound),
    }
}

/// An operand: one truth value, or lanes held as a Vector or as a column.
fn operand(lanes: &[u8], form: u8) -> Value {
    let values: Vec<Value> = lanes.iter().map(|&i| lane(i)).collect();
    match (form % 3, lanes) {
        (0, [_one]) => values[0].clone(),
        (1, _) => match TruthColumn::pack(&values) {
            Some(column) => Value::from_truth_column(column),
            None => Value::from_vector(values),
        },
        _ => Value::from_vector(values),
    }
}

/// `value`, every lane of it, with each NIL's absence spelled out.
fn exactly(value: &Value) -> String {
    let lanes = match value.as_vector_view() {
        Some(lanes) => lanes.into_owned(),
        None => vec![value.clone()],
    };
    let lanes: Vec<String> = lanes
        .iter()
        .map(|l| format!("{:?}/{:?}", l.ensure_hydrated(), l.absence_metadata()))
        .collect();
    format!("vector={} {lanes:?}", value.is_vector())
}

fn run(
    stack: Vec<Value>,
    op: fn(&mut Interpreter) -> Result<()>,
) -> std::result::Result<String, String> {
    let mut interp = Interpreter::new();
    interp.stack.extend(stack);
    match op(&mut interp) {
        Ok(()) => Ok(interp.stack.as_slice().iter().map(exactly).collect()),
        Err(e) => Err(format!("{e:?} / {}", interp.stack.len())),
    }
}

/// The lifted law on the Vectors the operands stand for.
fn lifted<const N: usize>(
    operands: [&Value; N],
    law: impl Fn([&Value; N]) -> Result<Value>,
) -> std::result::Result<String, String> {
    let hydrated: Vec<Value> = operands
        .iter()
        .map(|v| v.ensure_hydrated().into_owned())
        .collect();
    let refs: [&Value; N] = std::array::from_fn(|i| &hydrated[i]);
    lift_lanes(refs, law)
        .map(|v| exactly(&v))
        .map_err(|e| format!("{e:?} / {N}"))
}

fn lanes_strategy() -> impl Strategy<Value = (Vec<u8>, u8)> {
    (proptest::collection::vec(0u8..5, 0..6), 0u8..3)
}

proptest! {
    #[test]
    fn not_answers_the_lifted_law((lanes, form) in lanes_strategy()) {
        let value = operand(&lanes, form);
        let expected = lifted([&value], |[x]| compute_inverted_value(x));
        prop_assert_eq!(run(vec![value], op_not), expected);
    }

    #[test]
    fn and_answers_the_lifted_law(
        (left, left_form) in lanes_strategy(),
        (right, right_form) in lanes_strategy(),
    ) {
        let (a, b) = (operand(&left, left_form), operand(&right, right_form));
        let expected = lifted([&a, &b], |[x, y]| compute_conjunction(x, y));
        // An ERROR restores both operands, so the counts agree too.
        prop_assert_eq!(run(vec![a, b], op_and), expected);
    }

    #[test]
    fn select_answers_the_lifted_law(
        (mask, mask_form) in lanes_strategy(),
        candidates in (0u8..6, 0u8..6),
        width_seed in 0usize..3,
    ) {
        let mask = operand(&mask, mask_form);
        let width = mask.len().max(1);
        let candidate = |kind: u8, k: i64| match kind {
            0 => Value::from_int(k),
            1 => Value::from_fraction(crate::types::fraction::Fraction::new(k.into(), 3.into())),
            2 => Value::from_vector((0..width as i64).map(|i| Value::from_int(i * k)).collect()),
            3 => Value::from_vector(vec![Value::from_int(k)]),
            4 => lane(k as u8),
            _ => Value::from_vector((0..[width, 1, width + 1][width_seed]).map(|i| lane(i as u8)).collect()),
        };
        let (t, f) = (candidate(candidates.0, 7), candidate(candidates.1, -2));
        let expected = lifted([&t, &f, &mask], |[t, f, m]| compute_selection(t, f, m));
        prop_assert_eq!(run(vec![t, f, mask], op_select), expected);
    }
}
