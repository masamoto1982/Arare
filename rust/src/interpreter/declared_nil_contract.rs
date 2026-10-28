//! What a Word's declared operand roles oblige before its primitive runs.
//!
//! `spec/words.json` declares, per Word, what it does with each operand
//! (`stack.operands`, LANG.FAILURE.PASSTHROUGH): a `data` operand is read, so
//! an absent one makes the result that absence; an `element` is carried
//! without being read, so a NIL there is an ordinary value; a `control`
//! operand — a block, a name, a message — cannot be absent, so a NIL there is
//! malformed use; a `truth` operand reads NIL as UNKNOWN. This is the one
//! place that reads those roles, so no executor can quietly disagree with the
//! canon, and two Words that treat an operand alike treat a NIL there alike.

use crate::error::{AjisaiError, Result};
use crate::kernel::generated::{Arity, GeneratedWord, OperandRole};
use crate::types::Value;

use super::lane_lift::lift_lanes_dyn;
use super::ordering_ops::{restore, take_operand};
use super::Interpreter;

/// What the Word's declared operand roles require of the operands on the
/// stack, decided before its primitive is reached.
///
/// The passthrough arm names the projected NIL by its stack position rather
/// than carrying the value: the decision is made from a borrow of the stack,
/// and a position keeps the whole enum a couple of words wide.
enum NilContract {
    /// The declaration places no obligation here; run the primitive.
    Run,
    /// A NIL in a `control` position. `offset` is its position within the
    /// declared arity window, left to right in source order (0 = the
    /// first-pushed operand).
    Reject { offset: usize },
    /// A NIL in a `data` position is the Word's result; it flows through in
    /// place of running the primitive. `operands` is the declared operand
    /// window to unwind, `nil_index` the stack index of the NIL that becomes
    /// the result.
    PassThrough { operands: usize, nil_index: usize },
}

/// The declared condition a Word raises for a NIL in a `control` position:
/// the same condition its primitive raises for any other operand that is not
/// a block, a name or a message there, read from that Word's own `errorWhen`.
///
/// A table rather than a derivation because `errorWhen` lists every condition
/// a Word can raise, not which one belongs to which position (`DEF` declares
/// six, of which `invalidDefinitionBody` is its block's and `nonText` its
/// name's). `every_program_position_has_a_condition` pins the table against
/// the registry, so a new `control` position without an arm fails the build's
/// tests rather than panicking at runtime.
fn nil_rejection_error(word_name: &str, offset: usize) -> AjisaiError {
    const CODE: &str = "expected a Vector ([ ... ]) as the code operand, got NIL";
    match (word_name, offset) {
        ("EXEC", 0) | ("MAP", 1) | ("FILTER", 1) | ("FOLD", 2) | ("SCAN", 2) => {
            AjisaiError::declared("notExecutable", CODE)
        }
        ("CONTRACT", 0) => {
            AjisaiError::declared("notASymbol", "expected a Symbol naming a Word, got NIL")
        }
        ("FAIL", 0) => AjisaiError::declared("nonText", "expected a String message, got NIL"),
        ("ABSENT", 0) => AjisaiError::declared("nonText", "expected a String reason, got NIL"),
        ("BIND", 1) => AjisaiError::declared(
            "nonText",
            "expected a name (String) or a Vector of names, got NIL",
        ),
        ("DEF", 0) => AjisaiError::declared(
            "invalidDefinitionBody",
            "expected a Vector [ ... ] definition body, got NIL",
        ),
        ("DEF", 1) | ("DEL", 0) => {
            AjisaiError::declared("nonText", "expected a name (String), got NIL")
        }
        (word, offset) => unreachable!(
            "no declared condition registered for a NIL in {word}'s program operand {offset} — \
             add one to declared_nil_contract::nil_rejection_error"
        ),
    }
}

impl Interpreter {
    /// What the Word's declared operand roles dictate for the operands
    /// currently on the stack.
    ///
    /// A NIL in a `control` position is refused first, wherever it sits: a
    /// block, name or message that is absent is malformed use whatever else
    /// is absent beside it (LANG.FAILURE.TRICHOTOMY puts an ERROR ahead of a
    /// NIL). Otherwise the leftmost NIL in a `data` position is the result,
    /// matching left-to-right evaluation order. `element` and `truth`
    /// positions leave the NIL to the primitive, which treats it as a value
    /// or as UNKNOWN. A Word of data-dependent arity carries no fixed operand
    /// window, so it is left to its executor.
    fn declared_nil_contract(&self, word: &GeneratedWord) -> NilContract {
        let Arity::Fixed(arity) = word.stack_inputs else {
            return NilContract::Run;
        };
        let arity = arity as usize;
        let operands = self.stack.as_slice();

        // Refusing to run touches nothing, so a short stack can be judged on
        // what it holds: the window is clamped rather than required.
        let available = arity.min(operands.len());
        let window = &operands[operands.len() - available..];
        let roles = &word.operand_roles[arity - available..];
        if let Some(offset) = window
            .iter()
            .zip(roles)
            .position(|(operand, role)| *role == OperandRole::Control && operand.is_nil())
        {
            return NilContract::Reject {
                offset: offset + (arity - available),
            };
        }

        // Passing a NIL through unwinds the operands and synthesises a
        // result, which needs the whole window present; a short stack is an
        // arity fault, left to the executor to report as underflow.
        if available < arity {
            return NilContract::Run;
        }
        match window.iter().zip(roles).position(|(operand, role)| {
            matches!(role, OperandRole::Data | OperandRole::Leaf) && operand.is_nil()
        }) {
            Some(offset) => NilContract::PassThrough {
                operands: arity,
                nil_index: operands.len() - arity + offset,
            },
            None => NilContract::Run,
        }
    }

    /// Yield the NIL at stack index `nil_index` as the Word's result without
    /// running its primitive, removing the declared operand window. The NIL is
    /// copied out before the unwind, since the unwind is what removes it.
    fn pass_nil_through(&mut self, operands: usize, nil_index: usize) {
        let result = Value::nil_inheriting_absence_from(&self.stack[nil_index]);
        let remaining = self.stack.len() - operands;
        self.stack.drain(remaining..);
        self.stack.push(result);
    }

    /// Settle the Word's declared NIL contract against the current stack.
    ///
    /// `None` means the declaration places no obligation here and the primitive
    /// must run; `Some(result)` is the Word's outcome, decided without running
    /// it. Every dispatch path must consult this — a path that skips it is a
    /// path on which the declaration is decorative again.
    pub(super) fn apply_declared_nil_contract(
        &mut self,
        word: &GeneratedWord,
    ) -> Option<Result<()>> {
        match self.declared_nil_contract(word) {
            NilContract::Run => None,
            // A NIL where a block, name or message belongs is refused with
            // the same declared condition the primitive raises for any other
            // operand that is not one, so the two mistakes read alike.
            NilContract::Reject { offset } => Some(Err(nil_rejection_error(word.name, offset))),
            NilContract::PassThrough {
                operands,
                nil_index,
            } => {
                self.pass_nil_through(operands, nil_index);
                Some(Ok(()))
            }
        }
    }
}

// Lifting a Word over the Vectors and Records in its `leaf` and `truth`
// operands (LANG.COLLECTIONS.LIFT).
//
// A `leaf` operand is read as one Scalar, String or Boolean, so a container
// there means "apply the Word to each element". Every Word with a lifted
// operand is lifted here, through `lift_lanes_dyn`, except the few whose
// primitive already runs the same lift faster itself ([`lifts_natively`]);
// one rule decides how `[ 'a' 'b' ] UPPER`,
// `[ 1 2 ] 10 ADD` and `R [ 'x' 'y' ] GET` combine their elements. Each
// element runs through the full dispatcher, so the NIL roles, the declared
// conditions and the cost charges are exactly the Word's own.
/// The primitives that lift their own `leaf` and `truth` operands: the tensor
/// path of exact arithmetic, and `lane_lift` for comparison and logic. This is
/// an implementation choice, not part of any contract — the specification says
/// only which operands lift — and `rust/tests/lifting_laws.rs` holds these to
/// the same answers the dispatcher's lift gives every other Word.
///
/// Asked of the Word's identity rather than its name: every Word that is not
/// one of these asks on every call, and comparing its name against each of
/// these was a string compare apiece.
fn lifts_natively(word: &GeneratedWord) -> bool {
    use crate::kernel::generated::WordId::*;
    matches!(
        word.id,
        Add | Sub
            | Mul
            | Div
            | Floor
            | Round
            | Min
            | Max
            | Sqrt
            | Pow
            | Gcd
            | Ratio
            | Lt
            | Gt
            | And
            | Not
            | Select
    )
}

fn is_container(value: &Value) -> bool {
    value.is_vector() || value.as_record().is_some()
}

impl Interpreter {
    /// Lift the Word over its operands when a `leaf` or `truth` operand holds
    /// a Vector or Record and the Word does not lift natively. `None` means
    /// there is nothing to lift here and the primitive runs.
    pub(super) fn apply_declared_lift(
        &mut self,
        word: &'static GeneratedWord,
    ) -> Option<Result<()>> {
        if lifts_natively(word) {
            return None;
        }
        let Arity::Fixed(arity) = word.stack_inputs else {
            return None;
        };
        let arity = arity as usize;
        if arity == 0 || self.stack.len() < arity {
            return None;
        }
        let lifts = |role: &OperandRole| matches!(role, OperandRole::Leaf | OperandRole::Truth);
        let window = &self.stack.as_slice()[self.stack.len() - arity..];
        // Decided from the declared roles in place: the per-operand flags
        // are only built once there is something to lift.
        if !window
            .iter()
            .zip(word.operand_roles)
            .any(|(operand, role)| lifts(role) && is_container(operand))
        {
            return None;
        }
        let lifted: Vec<bool> = word.operand_roles.iter().map(lifts).collect();

        // The lift loops over every lane in one step and builds a container
        // of the results: a copy of the widest lifted operand, charged before
        // any lane runs. Each lane's own work is the Word's, charged by it.
        let lanes = window
            .iter()
            .zip(&lifted)
            .filter(|(operand, lifts)| **lifts && is_container(operand))
            .map(|(operand, _)| {
                super::collection_meter::element_cost(operand).copies(operand.len())
            })
            .max()
            .unwrap_or(0);
        if let Err(e) = super::collection_meter::charge(self, lanes) {
            return Some(Err(e));
        }

        let start = self.stack.len() - arity;
        let operands: Vec<Value> = self.stack.drain(start..).collect();
        let refs: Vec<&Value> = operands.iter().collect();
        let mut element = |lane: &[&Value]| -> Result<Value> {
            let base = self.stack.len();
            for operand in lane {
                self.stack.push((*operand).clone());
            }
            match self.execute_generated_word(word) {
                Ok(()) => {
                    // Every lifted Word answers exactly one value
                    // (word-schema:check), so the lane leaves one result.
                    assert_eq!(
                        self.stack.len(),
                        base + 1,
                        "{} answers one value",
                        word.name
                    );
                    Ok(self.stack.pop().expect("the result just asserted"))
                }
                Err(e) => {
                    self.stack.truncate(base);
                    Err(e)
                }
            }
        };
        let outcome = lift_lanes_dyn(&refs, &lifted, &mut element);
        Some(match outcome {
            Ok(result) => {
                self.stack.push(result);
                Ok(())
            }
            Err(e) => {
                self.stack.extend(operands);
                Err(e)
            }
        })
    }
}

// `ABSENT` and `FAIL`: the trichotomy, stated by the program (LANG.FAILURE.TRICHOTOMY).
//
// A Core Word's contract says which of the three outcomes each of its inputs
// meets; until these two Words a user Word could say neither of the failing
// two — it answered a bare literal NIL, or let some inner Word raise for it.
// `ABSENT` is a reasoned absence whose reason the program states, recovered
// like any other; `FAIL` is an ERROR the program raises, propagating like any
// other. Neither evaluates anything and neither can catch anything.
/// `ABSENT ( [ 'reason' ] -> [ NIL ] )`.
pub fn op_absent(interp: &mut Interpreter) -> Result<()> {
    let operand = take_operand(interp)?;
    let Some(text) = operand.as_text() else {
        let got = operand.domain_name();
        restore(interp, operand);
        return Err(AjisaiError::declared(
            "nonText",
            format!("expected a String reason, got {got}"),
        ));
    };
    let absence = Value::nil_user_declared(text);
    interp.stack.push(absence);
    Ok(())
}

/// `FAIL ( [ 'message' ] -> [ ] )`: raises `declaredFailure`. The operand is
/// put back first, as every Word's operands are on an ERROR.
pub fn op_fail(interp: &mut Interpreter) -> Result<()> {
    let operand = take_operand(interp)?;
    let Some(text) = operand.as_text() else {
        let got = operand.domain_name();
        restore(interp, operand);
        return Err(AjisaiError::declared(
            "nonText",
            format!("expected a String message, got {got}"),
        ));
    };
    let message = text.to_string();
    restore(interp, operand);
    Err(AjisaiError::declared("declaredFailure", message))
}

#[cfg(test)]
mod declared_nil_contract_tests {
    use super::nil_rejection_error;
    use crate::kernel::generated::{OperandRole, GENERATED_WORDS};

    /// Every `control` position has a registered condition for a NIL, and
    /// each is one its Word declares — `nil_rejection_error` panics on a
    /// missing arm, so this is what catches a new `control` operand before
    /// it ships.
    #[test]
    fn every_program_position_has_a_condition() {
        for word in GENERATED_WORDS {
            for (offset, role) in word.operand_roles.iter().enumerate() {
                if *role != OperandRole::Control {
                    continue;
                }
                let crate::error::AjisaiError::DeclaredCondition { condition, .. } =
                    nil_rejection_error(word.name, offset)
                else {
                    panic!("{} operand {offset}: not a declared condition", word.name);
                };
                assert!(
                    word.error_when.contains(&condition),
                    "{} operand {offset}: {condition} is not in its errorWhen",
                    word.name
                );
            }
        }
    }
}

#[cfg(test)]
mod declared_outcomes_tests {
    //! Behavioral probes for `ABSENT` and `FAIL`: the reason a program states is
    //! the reason the value carries, observably and as part of its identity.

    use crate::interpreter::Interpreter;
    use crate::test_support::{run, top};
    use crate::types::Value;

    #[tokio::test]
    async fn absent_carries_the_reason_the_program_states() {
        assert_eq!(
            top("'rate not quoted' ABSENT NIL-REASON").await,
            "'rate not quoted'"
        );
        assert_eq!(top("'why' ABSENT NIL?").await, "TRUE");
        assert_eq!(top("'why' ABSENT 'S' BIND 0 S S NIL? SELECT").await, "0/1");
        let interp = run("'why' ABSENT").await;
        let value = interp.stack.last().cloned().expect("an answer");
        assert_eq!(
            value.nil_reason().map(|r| r.as_protocol_str()),
            Some("userDeclared")
        );
        assert_eq!(value.absence_detail(), Some("why"));
    }

    /// LANG.VALUES.NIL: the reason is the value's entire observable content,
    /// and the text is the reason, so it decides identity.
    #[tokio::test]
    async fn the_text_is_part_of_the_value() {
        assert_eq!(
            top("'a' ABSENT 1 COLLECT 'b' ABSENT 1 COLLECT EQ").await,
            "FALSE"
        );
        assert_eq!(
            top("'a' ABSENT 1 COLLECT 'a' ABSENT 1 COLLECT EQ").await,
            "TRUE"
        );
        assert_ne!(Value::nil_user_declared("a"), Value::nil_user_declared("b"));
        assert_eq!(Value::nil_user_declared("a"), Value::nil_user_declared("a"));
    }

    /// The detail survives the two places a value can be stored other than
    /// the stack: a dense lane, and the persistence codec.
    #[tokio::test]
    async fn the_detail_survives_a_dense_lane() {
        assert_eq!(
            top("1 'why' ABSENT 2 COLLECT 1 GET NIL-REASON").await,
            "'why'"
        );
    }

    #[tokio::test]
    async fn fail_raises_the_declared_category_with_the_message() {
        let mut interp = Interpreter::new();
        let error = interp
            .execute("1 'width must be positive' FAIL 2")
            .await
            .expect_err("FAIL must raise");
        let text = error.to_string();
        assert!(text.contains("width must be positive"), "got: {text}");
        // The operand is restored, and nothing after FAIL ran.
        assert_eq!(
            interp
                .get_stack()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["1/1", "'width must be positive'"]
        );
    }

    #[tokio::test]
    async fn a_non_text_operand_is_the_program_being_wrong() {
        for code in ["1 ABSENT", "1 FAIL", "NIL ABSENT"] {
            let mut interp = Interpreter::new();
            let text = interp
                .execute(code)
                .await
                .expect_err(&format!("`{code}` must raise"))
                .to_string();
            assert!(
                text.contains("String"),
                "`{code}` must name the String it expected, got: {text}"
            );
        }
    }
}
