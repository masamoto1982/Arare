use super::higher_order::{execute_executable_code, extract_executable_code};
use crate::error::{AjisaiError, Result};
use crate::interpreter::Interpreter;
use crate::types::Stack;
use crate::types::Value;

/// What an accumulator walk answers with.
///
/// `FOLD` and `SCAN` are one walk: seed an accumulator, then for each element
/// run the caller's block over (accumulator, element) and take what it leaves
/// as the next accumulator. They differ only in which accumulators come back
/// — so the walk is written once and this says which.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    /// `FOLD`: the accumulator the last element left, alone.
    Last,
    /// `SCAN`: every accumulator the walk passed through, one per element, in
    /// visit order. The seed is not among them, so the answer has one lane per
    /// input lane — which is what lets a scan pair with the Vector it came
    /// from, under `ZIP`, a comparison, or `SELECT`'s truth operand.
    Every,
}

impl Answer {
    /// The answer for a walk with no elements to visit.
    ///
    /// The shape of the answer follows the shape of the question. `FOLD`
    /// reduces to one value, so with nothing to reduce it answers the seed it
    /// was given. `SCAN` answers one lane per lane, so with no lanes it
    /// answers an empty Vector, and the seed — which is not a lane — is not
    /// it.
    fn empty_walk(self, init: Value) -> Value {
        match self {
            Answer::Last => init,
            Answer::Every => Value::from_vector(Vec::new()),
        }
    }

    /// The answer for an absent target, by the same rule.
    ///
    /// `FOLD` of an absent Vector is the seed, because reducing nothing is the
    /// seed. `SCAN` of an absent Vector is that same absence, reason intact,
    /// because there is no lane count to answer with — the collection-shaped
    /// Words (`MAP`) answer an absent target the same way.
    fn absent_target(self, init: Value, target: Value) -> Value {
        match self {
            Answer::Last => init,
            Answer::Every => target,
        }
    }
}

pub fn op_fold(interp: &mut Interpreter) -> Result<()> {
    run_accumulator_walk(interp, "FOLD", Answer::Last)
}

/// `SCAN` — `FOLD` that keeps its working.
///
/// `[ vec ] [ init ] [ combine ] SCAN` answers the accumulator after each
/// element rather than only the last one, so `[ 1 2 3 4 ] 0 [ ADD ] SCAN` is
/// `[ 1/1 3/1 6/1 10/1 ]`. The seed's own shape carries through the walk
/// exactly as it does in `FOLD`: seeding with `[ 0 ]` instead makes every
/// accumulator a one-lane Vector, because that is what `ADD` answers when one
/// operand is one.
///
/// It is derivable — re-fold every prefix, which is what
/// `standard_operational_laws.rs` witnesses — and it is retained natively
/// because that derivation re-reads each prefix from the start and so does
/// quadratic work for a linear answer. In a language with no recursion and no
/// unbounded loop (`spec/termination.json`), a walk that carries state from
/// one element to the next is the only shape a running computation has, so
/// paying n² for it is paying it everywhere.
pub fn op_scan(interp: &mut Interpreter) -> Result<()> {
    run_accumulator_walk(interp, "SCAN", Answer::Every)
}

fn run_accumulator_walk(
    interp: &mut Interpreter,
    word: &'static str,
    answer: Answer,
) -> Result<()> {
    let code_val: Value = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    let executable = match extract_executable_code(interp, &code_val) {
        Ok(exec) => exec,
        Err(e) => {
            interp.stack.push(code_val);
            return Err(e);
        }
    };

    let init_val: Value = interp.stack.pop().ok_or_else(|| {
        interp.stack.push(code_val.clone());
        AjisaiError::stack_underflow()
    })?;
    let target_val: Value = interp.stack.pop().ok_or_else(|| {
        interp.stack.push(init_val.clone());
        interp.stack.push(code_val.clone());
        AjisaiError::stack_underflow()
    })?;

    if target_val.is_nil() {
        interp
            .stack
            .push(answer.absent_target(init_val, target_val));
        return Ok(());
    }

    if !target_val.is_vector() {
        let got = target_val.domain_name();
        interp.stack.push(target_val);
        interp.stack.push(init_val);
        interp.stack.push(code_val);
        return Err(AjisaiError::declared(
            "nonVector",
            format!("expected a Vector, got {got}"),
        ));
    }

    let n_elements: usize = target_val.len();
    if n_elements == 0 {
        interp.stack.push(answer.empty_walk(init_val));
        return Ok(());
    }

    let fused_walk = match answer {
        Answer::Last => crate::interpreter::fused_block::FusedWalk::Fold,
        Answer::Every => crate::interpreter::fused_block::FusedWalk::Scan,
    };
    if let Some(result) = executable
        .fused(interp, 2)
        .and_then(|block| block.run(interp, fused_walk, &target_val, Some(&init_val)))
    {
        interp.stack.push(result);
        return Ok(());
    }

    // The seed is kept apart from the running accumulator: a failure part way
    // through the walk puts the operands back as they were written, and the
    // seed is the operand, not whatever the walk had made of it by then.
    let mut accumulator: Value = init_val.clone();
    let mut visited: Vec<Value> = Vec::new();
    if answer == Answer::Every {
        visited.reserve(n_elements);
    }
    let mut saved_stack: Stack = Stack::new();
    std::mem::swap(&mut interp.stack, &mut saved_stack);
    let saved_no_change_check: bool = interp.disable_no_change_check;
    interp.disable_no_change_check = true;

    let mut error: Option<AjisaiError> = None;
    let mut children = target_val.children_reader();
    for i in 0..n_elements {
        let elem: Value = children
            .get(i)
            .unwrap_or_else(|| panic!("{word}: child index in 0..len must be valid"));
        interp.stack.clear();
        interp.stack.push(accumulator.clone());
        interp.stack.push(elem);
        match execute_executable_code(interp, &executable) {
            Ok(_) => match super::higher_order::block_result(interp) {
                Ok(result) => {
                    if answer == Answer::Every {
                        visited.push(result.clone());
                    }
                    accumulator = result;
                }
                Err(left) => {
                    error = Some(AjisaiError::declared(
                        "blockContractViolation",
                        super::higher_order::block_arity_message(
                            "the block to leave one value",
                            left,
                        ),
                    ));
                    break;
                }
            },
            Err(e) => {
                error = Some(e);
                break;
            }
        }
    }
    interp.disable_no_change_check = saved_no_change_check;
    interp.stack = saved_stack;

    if let Some(e) = error {
        interp.stack.push(target_val);
        interp.stack.push(init_val);
        interp.stack.push(code_val);
        return Err(e);
    }

    interp.stack.push(match answer {
        Answer::Last => accumulator,
        Answer::Every => Value::from_vector(visited),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Test suite for `crate::interpreter::higher_order_fold`.

    use crate::interpreter::Interpreter;

    fn top_scalar_i64(interp: &Interpreter) -> i64 {
        let top = interp.stack.last().expect("stack top");
        // A Boolean result reads as 1/0.
        if let Some(b) = top.as_truth() {
            return if b { 1 } else { 0 };
        }
        if let Some(f) = top.as_scalar() {
            return f.to_i64().expect("scalar i64");
        }
        let child = top.child(0).expect("vector[0]");
        if let Some(b) = child.as_truth() {
            return if b { 1 } else { 0 };
        }
        child
            .as_scalar()
            .and_then(|f| f.to_i64())
            .expect("expected scalar i64 on stack top")
    }

    #[tokio::test]
    async fn test_fold_basic() {
        let mut interp = Interpreter::new();
        let result = interp.execute("[ 1 2 3 4 ] [ 0 ] [ ADD ] FOLD").await;
        assert!(result.is_ok(), "FOLD should succeed: {:?}", result);
        assert_eq!(top_scalar_i64(&interp), 10);
    }

    #[tokio::test]
    async fn test_fold_of_an_absent_vector_is_that_absence() {
        // The Vector is data (LANG.FAILURE.PASSTHROUGH): an absent one is the
        // result, not the initial accumulator standing in for an empty fold.
        let mut interp = Interpreter::new();
        interp.execute("NIL [ 42 ] [ ADD ] FOLD").await.unwrap();
        assert!(interp.stack.last().is_some_and(|v| v.is_nil()));
    }
    /// `&` resolves to the same contract and executor as `AND`
    /// (LANG.SOURCE.NORMALIZE), including inside a predicate block.
    ///
    /// The operands are Booleans because `AND` is a truth-order connective, whose input
    /// domain is two-valued (LANG.VALUES.TRUTH). This case used to read
    /// `{ [ 0 ] [ 1 ] AND }`, where `[ 0 ]` and `[ 1 ]` are vector literals
    /// rather than the index lenses they resemble; it passed only because
    /// `AND` coerced the scalars `0` and `1` to truth values and `FILTER`
    /// accepted the resulting singleton Vector as a predicate result. Both
    /// coercions are gone, so the case now states its subject directly.
    #[tokio::test]
    async fn test_filter_reaches_and_by_name() {
        let mut and_interp = Interpreter::new();
        let and_result = and_interp
            .execute("[ TRUE FALSE TRUE ] [ TRUE AND ] FILTER")
            .await;
        assert!(
            and_result.is_ok(),
            "FILTER with AND failed: {:?}",
            and_result
        );

        // AND and NOT carry no symbol: `&` is an ordinary name the
        // dictionary does not have, inside a block body like anywhere else.
        let mut alias_interp = Interpreter::new();
        let alias_result = alias_interp
            .execute("[ TRUE FALSE TRUE ] [ TRUE & ] FILTER")
            .await;
        assert!(
            alias_result.is_err(),
            "`&` must not be a spelling of AND: {:?}",
            alias_result
        );
    }

    /// A truth-order connective raises its registered `nonTruthValue` ERROR on a
    /// scalar operand: the Boolean and Scalar domains are disjoint, so `1` is
    /// not TRUE and `0` is not FALSE (LANG.VALUES.DISJOINT).
    #[tokio::test]
    async fn test_logic_words_reject_scalar_operands() {
        for source in ["1 1 AND", "FALSE 0 AND", "5 NOT", "TRUE 1 AND"] {
            let mut interp = Interpreter::new();
            let result = interp.execute(source).await;
            assert!(
                result.is_err(),
                "`{}` must raise nonTruthValue, got {:?}",
                source,
                result
            );
        }
    }

    /// A predicate block must decide in the truth domain: a scalar and a
    /// singleton Vector are each a nonconforming predicate result rather than
    /// a truth value (LANG.VALUES.TRUTH).
    #[tokio::test]
    async fn test_higher_order_predicates_reject_non_boolean() {
        for source in ["[ 1 2 3 ] [ 1 ] FILTER", "[ 1 2 3 ] [ [ TRUE ] ] FILTER"] {
            let mut interp = Interpreter::new();
            let result = interp.execute(source).await;
            assert!(
                result.is_err(),
                "`{}` must reject a non-Boolean predicate result, got {:?}",
                source,
                result
            );
        }
    }

    /// A NIL is UNKNOWN in truth position (LANG.VALUES.TRUTH), for FILTER's
    /// predicate as for AND and SELECT: it is a truth value, so it raises
    /// nothing, and only a predicate that holds keeps its element.
    #[tokio::test]
    async fn test_filter_drops_an_unknown_predicate() {
        // `0/0` has no order (LANG.VALUES.EXACT), so its comparison projects
        // and is UNKNOWN there, and TRUE for the other two.
        let mut interp = Interpreter::new();
        interp
            .execute("[ 1 0/0 2 ] [ 'X' BIND X 1/3 GT ] FILTER")
            .await
            .expect("an UNKNOWN predicate is a truth value, not an ERROR");
        assert_eq!(
            crate::types::display::render_stack(interp.get_stack()),
            vec!["[ 1/1 2/1 ]".to_string()]
        );
    }
}
