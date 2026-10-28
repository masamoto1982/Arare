use crate::error::{AjisaiError, NilReason, Result};
use crate::interpreter::lane_lift::lift_lanes;
use crate::interpreter::truth_ops;
use crate::interpreter::Interpreter;
use crate::types::{Value, ValueData};

/// One of the three truth values of LANG.VALUES.TRUTH, read from a lane in
/// truth position.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Truth {
    True,
    False,
    /// A NIL read in truth position, whatever its reason.
    Unknown,
}

/// The truth value of a truth-order connective's operand.
///
/// TRUE and FALSE are the Boolean data values and NIL reads as UNKNOWN
/// (LANG.VALUES.TRUTH); nothing else is a truth value, so a scalar, a String
/// or a Vector standing in truth position is the `nonTruthValue` every Word
/// of the family registers. `0` and `1` are not truth values
/// (LANG.VALUES.DISJOINT): a caller who means a numeric test writes it,
/// `0 EQ NOT`.
///
/// A truth operand lifts (LANG.COLLECTIONS.LIFT), so this sees one lane at a
/// time: [`lift_lanes`] has already aligned the operands, and a Vector
/// reaching here is a Vector standing where a truth value belongs.
fn operand_truth(value: &Value) -> Result<Truth> {
    match &value.data {
        ValueData::Boolean(true) => Ok(Truth::True),
        ValueData::Boolean(false) => Ok(Truth::False),
        ValueData::Nil(_) => Ok(Truth::Unknown),
        _ => Err(AjisaiError::declared(
            "nonTruthValue",
            format!("expected a truth value, got {}", value.domain_name()),
        )),
    }
}

/// Conjunction under the strong Kleene table (LANG.VALUES.TRUTH), the meet
/// of the truth order FALSE < UNKNOWN < TRUE — SQL's `AND`. FALSE absorbs
/// everything and TRUE is the identity. Where the answer is an operand it is
/// that operand whole, so an UNKNOWN keeps its reason — the left one's, when
/// both are UNKNOWN.
fn compute_conjunction(a: &Value, b: &Value) -> Result<Value> {
    use Truth::*;
    Ok(match (operand_truth(a)?, operand_truth(b)?) {
        (False, _) | (_, False) => Value::from_bool(false),
        (True, _) | (Unknown, Unknown) => b.clone(),
        (Unknown, True) => a.clone(),
    })
}

/// `AND` over whole operands: the scalar law above, applied lane by lane
/// (LANG.COLLECTIONS.LIFT).
fn lifted_conjunction(a: &Value, b: &Value) -> Result<Value> {
    lift_lanes([a, b], |[x, y]| compute_conjunction(x, y))
}

/// `SELECT`'s scalar law: TRUE and FALSE choose one of the two values it was
/// handed, and UNKNOWN chooses neither.
///
/// Nothing is evaluated here. Both candidates are values the program already
/// built, so the work that produced them happened before `SELECT` ran, once
/// each and in the order they were written — which is why `SELECT` is `pure`
/// and `const` on the step axis where `COND` was `unbounded` on all three.
pub(crate) fn compute_selection(
    when_true: &Value,
    when_false: &Value,
    mask: &Value,
) -> Result<Value> {
    Ok(match operand_truth(mask)? {
        Truth::True => when_true.clone(),
        Truth::False => when_false.clone(),
        // UNKNOWN chooses neither: the answer is the absence the truth
        // operand carried, reason intact, so `NIL-REASON` can still say why.
        Truth::Unknown => mask.clone(),
    })
}

fn compute_inverted_value(val: &Value) -> Result<Value> {
    // NOT swaps TRUE and FALSE and leaves UNKNOWN where it is, flowing out
    // unchanged with its reason (LANG.VALUES.TRUTH's NOT row).
    Ok(match operand_truth(val)? {
        Truth::True => Value::from_bool(false),
        Truth::False => Value::from_bool(true),
        Truth::Unknown => val.clone(),
    })
}

/// `RECONCILE`'s scalar law: what two sources agree on (LANG.VALUES.TRUTH).
///
/// It is the join of the information order — an absence knows least, a
/// conflict most. A conflict absorbs, an absence yields to the other source,
/// equal values agree, and any two different values — two truth values
/// included — project NIL(conflict). Making the conflict absorbing
/// rather than one more absence that yields is what keeps the Word
/// associative: `1 2 RECONCILE 3 RECONCILE` and `1 2 3 RECONCILE RECONCILE`
/// are both the conflict.
///
/// Among numbers `0/0` knows least, as UNKNOWN does among truth values: its
/// zero shape is the bottom of the shapes (LANG.VALUES.SHAPE), so it yields to the
/// other number as an absence yields to any source. It yields only to a
/// number — a String or a truth value tells nothing about which number is
/// meant — so the order stays a join semilattice and the Word associative.
pub(crate) fn reconcile(a: &Value, b: &Value) -> Value {
    let is_conflict = |v: &Value| v.nil_reason() == Some(&NilReason::Conflict);
    if is_conflict(a) || b.is_nil() && !is_conflict(b) {
        return a.clone();
    }
    if is_conflict(b) || a.is_nil() || a == b {
        return b.clone();
    }
    let is_number = |v: &Value| matches!(v.data, ValueData::Scalar(_) | ValueData::ExactScalar(_));
    let is_nullity = |v: &Value| matches!(&v.data, ValueData::Scalar(f) if f.is_nullity());
    if is_nullity(b) && is_number(a) {
        return a.clone();
    }
    if is_nullity(a) && is_number(b) {
        return b.clone();
    }
    Value::nil_with_reason(NilReason::Conflict)
}

/// `RECONCILE` — what two sources agree on. Like `EQ` it reads its
/// operands whole, so two Vectors agree when they are one value.
pub fn op_reconcile(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 2 {
        return Err(AjisaiError::stack_underflow());
    }
    let b_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    let a_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    interp.stack.push(reconcile(&a_val, &b_val));
    Ok(())
}

pub fn op_not(interp: &mut Interpreter) -> Result<()> {
    // A column, or a Vector of truth values, negated as codes in place.
    if let Some(top) = interp.stack.len().checked_sub(1) {
        if truth_ops::negate(&mut interp.stack[top]) {
            return Ok(());
        }
    }
    let val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    let result = match lift_lanes([&val], |[x]| compute_inverted_value(x)) {
        Ok(v) => v,
        Err(e) => {
            interp.stack.push(val);
            return Err(e);
        }
    };

    interp.stack.push(result);
    Ok(())
}

pub fn op_and(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 2 {
        return Err(AjisaiError::stack_underflow());
    }

    let b_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    let a_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    if let Some(answer) = truth_ops::conjoin(&a_val, &b_val) {
        interp.stack.push(answer);
        return Ok(());
    }

    let result = match lifted_conjunction(&a_val, &b_val) {
        Ok(v) => v,
        Err(e) => {
            interp.stack.push(a_val);
            interp.stack.push(b_val);
            return Err(e);
        }
    };
    interp.stack.push(result);
    Ok(())
}

/// `SELECT` — the conditional.
///
/// `[ whenTrue ] [ whenFalse ] [ mask ] SELECT` answers one of the two
/// candidates per lane. The truth operand comes last because that is where
/// every Word puts the operand that decides what it does, and because
/// `NIL?` leaves its answer exactly there: `[ 0 ] X X NIL? SELECT` reads as
/// "0 if X is absent, else X" with nothing moved on the stack.
///
/// Unlike the `COND` this replaces, `SELECT` evaluates nothing and holds no
/// frame: its operands are ordinary values, aligned by the one lifting rule
/// (LANG.COLLECTIONS.LIFT), so branching is no longer a construct with laws
/// of its own. There is no else-clause to reach and no clause set to exhaust
/// — two candidates and a truth are total by construction.
pub fn op_select(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 3 {
        return Err(AjisaiError::stack_underflow());
    }

    let mask = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    let when_false = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    let when_true = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    if let Some(answer) = truth_ops::select(&when_true, &when_false, &mask) {
        interp.stack.push(answer);
        return Ok(());
    }

    let result = match lift_lanes([&when_true, &when_false, &mask], |[t, f, m]| {
        compute_selection(t, f, m)
    }) {
        Ok(v) => v,
        Err(e) => {
            interp.stack.push(when_true);
            interp.stack.push(when_false);
            interp.stack.push(mask);
            return Err(e);
        }
    };

    interp.stack.push(result);
    Ok(())
}

#[cfg(test)]
#[path = "truth_ops_tests.rs"]
mod truth_ops_tests;
