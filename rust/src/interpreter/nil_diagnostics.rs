//! Diagnostic absence accessors (LANG.VALUES.NIL / LANG.OBSERVATION.DIAGNOSIS).
//!
//! `NIL?` and `NIL-REASON` let a program read what a reasoned NIL carries
//! (LANG.VALUES.NIL, `NilReason`) instead of collapsing every absence with a single
//! chosen fallback. They are the whole set: a NIL carries its reason and
//! nothing else to read.
//!
//! Two invariants hold for both:
//!
//!   * **They consume what they read**, like every Word
//!     (LANG.STACK.CONSUMPTION): the inspected value leaves the stack and the
//!     answer takes its place. A program that needs the value afterwards names
//!     it with `BIND`.
//!   * **Every absence, U included.** They key off
//!     [`Value::is_nil`], which is every `Nil` value. The logical
//!     Unknown (U) is a NIL read in truth position (LANG.VALUES.TRUTH), so it
//!     is an absence: `NIL?` answers TRUE for it and `NIL-REASON` reports the
//!     reason it arrived with. Excluding U here briefly made
//!     `'x' NUM TRUE AND NIL-REASON` answer that it had no reason while the
//!     protocol published `absence.reason = invalidEncoding` for the same
//!     value.
//!
//! Applied to a value that is not an operational NIL, `NIL?` yields `FALSE` —
//! a predicate answers its question — and `NIL-REASON` projects a NIL whose
//! reason is `domainMiss`: a well-formed operand outside the accessor's domain,
//! the reason `SQRT` gives a negative radicand (LANG.FAILURE.PROJECT), never an
//! error.

use crate::error::{AjisaiError, NilReason, Result};
use crate::interpreter::Interpreter;
use crate::semantic::AbsenceMetadata;
use crate::types::Value;

/// The operational-NIL metadata of a value, or `None` when it is not an
/// operational NIL.
fn operational_absence(value: &Value) -> Option<&AbsenceMetadata> {
    if !value.is_nil() {
        return None;
    }
    // Every operational NIL has metadata; `absence_metadata` is `Some` for a
    // reasoned NIL and the literal-NIL constructor. Fall back defensively.
    value.absence_metadata()
}

/// A protocol-string Text result, or a `domainMiss` NIL when the accessor
/// found no value.
///
/// The projected NIL is *reasoned*. It used to be `Value::nil()`, a bare
/// literal NIL, which left `NIL-REASON`'s declared projection reason
/// unobservable: `5 NIL-REASON NIL-REASON` answered NIL rather
/// than the registered reason. `LANG.FAILURE.PROJECT` says a projection
/// produces "NIL with the reason its contract registers", and
/// `LANG.VALUES.NIL` makes the reason a NIL's entire observable content — a
/// reasonless projection would have no content to observe.
fn push_protocol_string_or_nil(interp: &mut Interpreter, value: Option<&str>) {
    match value {
        Some(protocol) => interp.stack.push(Value::from_string(protocol)),
        None => interp
            .stack
            .push(Value::nil_with_reason(NilReason::DomainMiss)),
    }
}

/// `NIL?` — consume the value and push `TRUE` when it was an operational NIL,
/// `FALSE` otherwise. It checks absence only and never branches on the reason
/// (LANG.VALUES.NIL).
pub fn op_nil_check(interp: &mut Interpreter) -> Result<()> {
    let value = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    interp.stack.push(Value::from_bool(value.is_nil()));
    Ok(())
}

/// `NIL-REASON` — the direct reason as a lowerCamelCase protocol-string Text,
/// or a `domainMiss` NIL when the value is not an operational NIL.
///
/// A `userDeclared` NIL (one `ABSENT` made) answers the text it was declared
/// with rather than the reason id: that text is the reason's parameter and,
/// under `LANG.VALUES.NIL`, the NIL's entire observable content.
pub fn op_nil_reason(interp: &mut Interpreter) -> Result<()> {
    let value = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;
    let protocol: Option<String> = operational_absence(&value).and_then(|absence| {
        let reason = absence.reason.as_ref()?;
        Some(match (reason, absence.detail_text()) {
            (NilReason::UserDeclared, Some(detail)) => detail.to_string(),
            _ => reason.as_protocol_str().to_string(),
        })
    });
    push_protocol_string_or_nil(interp, protocol.as_deref());
    Ok(())
}
