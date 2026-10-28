//! Tests for contract inference (`word_contract`): the arity a body has and
//! the `field` it keeps, read without running it.

use crate::coreword_registry::FieldClosure;
use crate::interpreter::word_contract::{ContractFlow, WordContract};
use crate::interpreter::Interpreter;
use std::sync::Arc;

async fn contract_for(src: &str, name: &str) -> Arc<WordContract> {
    let mut interp = Interpreter::new();
    interp.execute(src).await.unwrap();
    interp.infer_word_contract(name).unwrap()
}

fn fixed(consumes: u16, produces: u16) -> ContractFlow {
    ContractFlow::Fixed { consumes, produces }
}

#[tokio::test]
async fn arity_joins_along_the_body_and_through_dependencies() {
    let contract = contract_for("[ 2 MUL 1 ADD ] 'F' DEF [ F F ADD ] 'G' DEF", "G").await;
    assert_eq!(contract.flow, fixed(2, 1));
    assert_eq!(contract.field, FieldClosure::Closed);
}

/// `[ ... ]` pushes exactly one value and consumes nothing; its interior is
/// content, not code, so a Word inside it is never applied to the arity.
#[tokio::test]
async fn a_vector_literal_pushes_one_value_whatever_it_holds() {
    for (source, flow) in [
        ("[ [ 1 2 ] ] 'W' DEF", fixed(0, 1)),
        ("[ [ 10 20 ] ADD ] 'W' DEF", fixed(1, 1)),
        ("[ [ 2 MUL ] MAP ] 'W' DEF", fixed(1, 1)),
        ("[ [ [ 1 ] [ 2 ] ] ] 'W' DEF", fixed(0, 1)),
        ("[ [ 'a' PRINT 'b' ] ] 'W' DEF", fixed(0, 1)),
    ] {
        assert_eq!(contract_for(source, "W").await.flow, flow, "{source}");
    }
}

/// A name a `BIND` in the body made reads as one value, not as a Word.
#[tokio::test]
async fn a_bound_name_reads_as_one_value() {
    for (src, name) in [
        ("[ 'V' BIND V V LENGTH DIV ] 'M' DEF", "M"),
        ("[ [ 'A' 'B' ] BIND A B ADD ] 'S' DEF", "S"),
    ] {
        assert_eq!(contract_for(src, name).await.flow, fixed(1, 1), "`{src}`");
    }
}

#[tokio::test]
async fn an_arity_the_walk_cannot_state_is_variable() {
    let body = vec!["1"; 65_537].join(" ");
    let past_u16 = contract_for(&format!("[ {body} ] 'W' DEF"), "W").await;
    assert_eq!(past_u16.flow, ContractFlow::Dynamic);
    let variable = contract_for("[ 2 COLLECT ] 'W' DEF", "W").await;
    assert_eq!(variable.flow, ContractFlow::Dynamic);
}

/// Code a higher-order Word runs is read when it is the literal written right
/// before it; any other code operand was never read, so the body may leave
/// the field and the inference names no exit for it.
#[tokio::test]
async fn unread_code_is_the_weaker_claim() {
    let read = contract_for("[ [ 0 DIV ] MAP ] 'W' DEF", "W").await;
    assert_eq!(read.field, FieldClosure::Leaving);
    assert_eq!(read.field_exits, vec!["DIV".to_string()]);
    assert!(!read.field_unread);
    for body in ["[ 2 MUL ] 'B' BIND B EXEC", "[ [ 1 ] ] 0 GET EXEC", "MAP"] {
        let unread = contract_for(&format!("[ {body} ] 'W' DEF"), "W").await;
        assert_eq!(unread.field, FieldClosure::Leaving, "{body}");
        assert!(unread.field_unread, "{body}");
        assert!(unread.field_exits.is_empty(), "{body}");
    }
    // A Word built on one inherits the weaker claim, not an exit.
    let caller = contract_for("[ MAP ] 'APPLY' DEF [ APPLY ] 'W' DEF", "W").await;
    assert!(caller.field_unread);
    // Data is never run, whatever it spells.
    let data = contract_for("[ [ 0 DIV ] ] 'W' DEF", "W").await;
    assert_eq!(data.field, FieldClosure::Closed);
}

#[tokio::test]
async fn a_name_nothing_defines_leaves_the_arity_variable() {
    let mut interp = Interpreter::new();
    interp.execute("[ 1 ADD ] 'W' DEF").await.unwrap();
    let contract = interp.infer_contract_for_block(&crate::tokenizer::tokenize("1 NOPE").unwrap());
    assert_eq!(contract.flow, ContractFlow::Dynamic);
    assert_eq!(contract.field, FieldClosure::Closed);
}

#[tokio::test]
async fn contract_answers_one_record_shape_for_core_user_and_block() {
    for (code, expected) in [
        (
            "[ ADD ] 0 GET CONTRACT",
            "[ 'inputs' 'outputs' 'field' ] [ 2/1 1/1 'closed' ] RECORD",
        ),
        (
            "[ DIV ] CONTRACT",
            "[ 'inputs' 'outputs' 'field' ] [ 2/1 1/1 'leaving' ] RECORD",
        ),
        (
            "[ 2 DIV ] 'H' DEF [ H ] 0 GET CONTRACT",
            "[ 'inputs' 'outputs' 'field' ] [ 1/1 1/1 'closed' ] RECORD",
        ),
        (
            "[ 2 COLLECT ] CONTRACT",
            "[ 'inputs' 'outputs' 'field' ] [ 'variable' 'variable' 'closed' ] RECORD",
        ),
        ("[ NOPE ] 0 GET CONTRACT NIL-REASON", "'notFound'"),
    ] {
        let interp = crate::test_support::run(code).await;
        let top = crate::types::display::render_stack(interp.get_stack());
        assert_eq!(top.last().map(String::as_str), Some(expected), "{code}");
    }
}
