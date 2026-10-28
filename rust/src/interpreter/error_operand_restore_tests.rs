//! The ERROR discipline of every Core Word (LANG.STACK.CONSUMPTION): a Word
//! selects its operands, validates, computes, and only then consumes them, so
//! a Word that raises leaves the stack holding exactly the operands it was
//! given — unchanged, and in the order they were written. The stack an error
//! report carries is then the one the reader wrote, never one the failing
//! Word half-rewrote or emptied on its way out.

use crate::interpreter::Interpreter;
use crate::types::display::render_stack;

/// Run `source`, require it to fail, and answer the stack it left, rendered
/// through the shared observation surface.
async fn stack_after_error(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new();
    let outcome = interp.execute(source).await;
    assert!(outcome.is_err(), "`{source}` was expected to fail");
    render_stack(interp.get_stack())
}

async fn stack_after(source: &str) -> Vec<String> {
    let mut interp = Interpreter::new();
    interp
        .execute(source)
        .await
        .unwrap_or_else(|e| panic!("`{source}` unexpectedly failed: {e}"));
    render_stack(interp.get_stack())
}

/// A walk that fails part way through puts the seed back, not the running
/// accumulator: the seed is the operand the program wrote.
#[tokio::test]
async fn fold_restores_the_seed_not_the_running_accumulator() {
    assert_eq!(
        stack_after_error("[ 1 2 'x' 4 ] 100 [ ADD ] FOLD").await,
        vec!["[ 1/1 2/1 'x' 4/1 ]", "100/1", "[ ADD ]"]
    );
    assert_eq!(
        stack_after_error("[ 1 2 'x' 4 ] 100 [ ADD ] SCAN").await,
        vec!["[ 1/1 2/1 'x' 4/1 ]", "100/1", "[ ADD ]"]
    );
}

#[tokio::test]
async fn fold_restores_its_operands_when_the_block_leaves_nothing() {
    assert_eq!(
        stack_after_error("[ 1 2 ] 0 [ 'K' BIND 'J' BIND ] FOLD").await,
        vec!["[ 1/1 2/1 ]", "0/1", "[ 'K' BIND 'J' BIND ]"]
    );
}

/// An underflow found after the code operand was taken puts it back.
#[tokio::test]
async fn higher_order_words_restore_the_block_on_underflow() {
    assert_eq!(stack_after_error("[ ADD ] FOLD").await, vec!["[ ADD ]"]);
    assert_eq!(
        stack_after_error("5 [ ADD ] FOLD").await,
        vec!["5/1", "[ ADD ]"]
    );
    assert_eq!(stack_after_error("[ ADD ] SCAN").await, vec!["[ ADD ]"]);
    assert_eq!(
        stack_after_error("[ 1 ADD ] MAP").await,
        vec!["[ 1/1 ADD ]"]
    );
    assert_eq!(
        stack_after_error("[ 1 GT ] FILTER").await,
        vec!["[ 1/1 GT ]"]
    );
}

#[tokio::test]
async fn def_restores_both_operands_on_every_refusal() {
    // nonText name
    assert_eq!(
        stack_after_error("[ 1 ] 5 DEF").await,
        vec!["[ 1/1 ]", "5/1"]
    );
    // invalidDefinitionBody: not a Vector
    assert_eq!(stack_after_error("5 'X' DEF").await, vec!["5/1", "'X'"]);
    // protectedWord, refused inside op_def_inner
    assert_eq!(
        stack_after_error("[ 1 2 ] 'ADD' DEF").await,
        vec!["[ 1/1 2/1 ]", "'ADD'"]
    );
    // invalidName, refused inside op_def_inner
    assert_eq!(
        stack_after_error("[ 1 2 ] '[' DEF").await,
        vec!["[ 1/1 2/1 ]", "'['"]
    );
}

#[tokio::test]
async fn del_restores_its_operand_on_every_refusal() {
    assert_eq!(stack_after_error("'NOPE' DEL").await, vec!["'NOPE'"]);
    assert_eq!(stack_after_error("'ADD' DEL").await, vec!["'ADD'"]);
    assert_eq!(stack_after_error("5 DEL").await, vec!["5/1"]);
}

#[tokio::test]
async fn exec_restores_its_operand_when_it_refuses_to_run() {
    assert_eq!(stack_after_error("5 EXEC").await, vec!["5/1"]);
    assert_eq!(stack_after_error("'ADD' EXEC").await, vec!["'ADD'"]);
}

/// A Scalar a program built is a value, not a lexeme: running it from a block
/// is not reading the numeric grammar, so the numeric-literal ceiling does not
/// apply — and the answer is the same whichever Word applies the block.
#[tokio::test]
async fn a_computed_scalar_runs_from_a_block_on_every_route() {
    let direct = stack_after("2 20000 POW").await;
    assert_eq!(stack_after("[ 2 ] 20000 POW EXEC").await, direct);
    // The computed Scalar is appended to a block that consumes the element,
    // so the block leaves the one value it owes (LANG.COLLECTIONS.HIGHER).
    let mapped = stack_after("[ 1 ] [ 2 ] 20000 POW [ MUL ] CONCAT MAP 0 GET").await;
    assert_eq!(mapped, direct);
}
