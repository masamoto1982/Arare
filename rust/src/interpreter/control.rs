use crate::error::{AjisaiError, Result};
use crate::interpreter::Interpreter;
use crate::types::Value;

/// `EXEC` — evaluate a Vector's elements as instructions.
///
/// Every Vector is executable: code and data share one Vector domain, written
/// in `[ ]`, so `EXEC` runs any Vector (`[ 1 2 ADD ]`). The elements are
/// bridged back to tokens (`value_as_code.rs`) and run through the existing
/// token-based execution loop unchanged.
///
/// `as_vector_view` (not `as_vector`) matters here: a fully-numeric-
/// rectangular literal like `[ 1 2 ]` or `{ 1 2 }` silently promotes to
/// `ValueData::Tensor` (a storage optimization that predates this
/// unification), and `as_vector` alone excludes it.
///
/// `EXEC` used to render *any* value back to Ajisai source text and re-tokenize
/// it. That made a value's printed form decide its meaning, and the rendering
/// is not a right inverse of the reader: String is encoded as a Vector of
/// codepoints, which renders as those numbers, so `[ 1 2 ADD ] EXEC` pushed
/// `1 2 [ 65 68 68 ]` — the word `ADD` came back as its own codepoints instead
/// of being applied. Bridging the elements directly needs no such round-trip.
pub(crate) fn op_exec(interp: &mut Interpreter) -> Result<()> {
    exec_block(interp)
}

fn exec_block(interp: &mut Interpreter) -> Result<()> {
    let target: Value = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    let Some(elements) = target.as_vector_view() else {
        let got = target.domain_name();
        interp.stack.push(target);
        return Err(AjisaiError::declared(
            "notExecutable",
            format!("expected a Vector ([ ... ]) as the code operand, got {got}"),
        ));
    };
    // Every refusal before the block starts puts the operand back (the ERROR
    // discipline of every Core Word, LANG.STACK.CONSUMPTION). Once the block
    // runs, the frame is the whole stack and what it leaves is the state.
    //
    // The elements are values already built, not lexemes: the numeric-literal
    // ceiling (LANG.MACHINE.LIMITS) bounds what a lexeme may denote at the
    // entry points that read the numeric grammar, and this is not one of
    // them. A Scalar the program computed within its `bigintBits` used to be
    // refused here as a "literal" of too many digits, while the same block
    // applied by `MAP` ran it — the outcome depended on the route.
    let tokens = match crate::interpreter::value_as_code::value_elements_to_tokens(&elements)
        .and_then(|tokens| {
            crate::tokenizer::validate_code_tokens(&tokens)
                .map_err(AjisaiError::MalformedSource)
                .map(|()| tokens)
        }) {
        Ok(t) => t,
        Err(e) => {
            interp.stack.push(target);
            return Err(e);
        }
    };
    // The block `EXEC` runs is its own token stream and is never the enclosing
    // word's tail position — see `Interpreter::execute_nested_block`.
    interp.execute_nested_block(&tokens)
}

#[cfg(test)]
mod tests {
    //! Test suite for `crate::interpreter::control` (EXEC/EVAL).

    use crate::interpreter::Interpreter;
    #[tokio::test]
    async fn test_exec_empty_stack_error() {
        let mut interp = Interpreter::new();

        let result = interp.execute("EXEC").await;

        assert!(result.is_err(), "EXEC on empty stack should fail");
    }

    #[tokio::test]
    async fn test_eval_empty_stack_error() {
        let mut interp = Interpreter::new();

        let result = interp.execute("EVAL").await;

        assert!(result.is_err(), "EVAL on empty stack should fail");
    }
}
