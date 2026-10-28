//! Behavioral probes for the reflection Words `DIGEST` and
//! `CONTRACT` — over a Symbol and over a block
//! (LANG.DICTIONARY.RESOLUTION, LANG.DICTIONARY.MUTATION,
//! LANG.CONTRACT.REGISTRY, LANG.CONTRACT.CHECK).

use crate::interpreter::Interpreter;
use crate::test_support::{error_of, run, top};

#[tokio::test]
async fn contract_names_a_word_by_a_string_or_a_symbol() {
    // A String names a Word here as it does for DEF, BIND and DEL, and reads
    // the same entry the Symbol does, Core and User alike.
    assert_eq!(
        top("'ADD' CONTRACT [ ADD ] 0 GET CONTRACT EQ").await,
        "TRUE"
    );
    assert_eq!(
        top("[ 2 MUL ] 'TWICE' DEF 'twice' CONTRACT [ TWICE ] 0 GET CONTRACT EQ").await,
        "TRUE"
    );
    assert_eq!(top("'NOPE' CONTRACT NIL-REASON").await, "'notFound'");
    // Anything that is neither a name nor a block is malformed use, and the
    // operand is back on the stack after the ERROR.
    assert_eq!(error_of("NIL CONTRACT").await, "notASymbol");
    assert_eq!(error_of("5 CONTRACT").await, "notASymbol");
    let mut interp = Interpreter::new();
    let _ = interp.execute("5 CONTRACT").await;
    assert_eq!(interp.stack.len(), 1);
}

#[tokio::test]
async fn digest_reads_a_string_as_text_whatever_it_spells() {
    // `'INC' DIGEST` is the text's digest, before and after a Word INC
    // exists: a value's digest does not depend on the dictionary.
    let before = top("'INC' DIGEST").await;
    let after = top("[ 1 ADD ] 'INC' DEF 'INC' DIGEST").await;
    assert_eq!(before, after);
    assert_eq!(
        top("[ 1 ADD ] 'INC' DEF 'INC' DIGEST [ INC ] 0 GET DIGEST EQ").await,
        "FALSE"
    );
}

#[tokio::test]
async fn digest_is_a_word_identity_or_a_denotation_digest() {
    let core = top("[ ADD ] 0 GET DIGEST").await;
    assert_eq!(core.len(), 2 + 1 + 64, "a #-prefixed 64-hex digest, quoted");
    assert_eq!(
        top("[ ADD ] 0 GET DIGEST [ ADD ] 0 GET DIGEST EQ").await,
        "TRUE"
    );
    assert_eq!(
        top("[ ADD ] 0 GET DIGEST [ SUB ] 0 GET DIGEST EQ").await,
        "FALSE"
    );
    // A User Word's digest is the dictionary's own content identity.
    let interp = run("[ 2 MUL ] 'TWICE' DEF [ TWICE ] 0 GET DIGEST").await;
    let answer = interp.stack.last().unwrap().as_text().unwrap().to_string();
    assert_eq!(Some(&answer), interp.word_identity("TWICE"));
    // Content, not spelling: the same body under two names is one Word.
    assert_eq!(
        top("[ 2 MUL ] 'TWICE' DEF [ 2 MUL ] 'DOUBLE' DEF [ TWICE ] 0 GET DIGEST [ DOUBLE ] 0 GET DIGEST EQ")
            .await,
        "TRUE"
    );
    // A value digests by its denotation (LANG.VALUES.DENOTATION).
    assert_eq!(
        top("8 SQRT DIGEST 2 SQRT 2 SQRT ADD DIGEST EQ").await,
        "TRUE"
    );
    assert_eq!(top("[ 1 2 ] DIGEST [ 1 2 ] DIGEST EQ").await, "TRUE");
    assert_eq!(top("[ 1 2 ] DIGEST [ 2 1 ] DIGEST EQ").await, "FALSE");
    assert_eq!(top("1 0 DIV DIGEST 1 0 DIV DIGEST EQ").await, "TRUE");
    assert_eq!(top("1 0 DIV DIGEST NIL DIGEST EQ").await, "FALSE");
    // A Symbol naming nothing is still a value.
    assert_eq!(
        top("[ TWICE ] 0 GET DIGEST [ TWICE ] 0 GET DIGEST EQ").await,
        "TRUE"
    );
    // Every value has a finite canonical form to digest, an irrational's
    // inside a Vector included.
    assert_eq!(
        top("1 2 SQRT 2 COLLECT DIGEST 1 8 SQRT 2 DIV 2 COLLECT DIGEST EQ").await,
        "TRUE"
    );
}

#[tokio::test]
async fn contract_answers_the_registered_record_of_a_core_word() {
    assert_eq!(top("[ DIV ] 0 GET CONTRACT 'inputs' GET").await, "2/1");
    assert_eq!(top("[ DIV ] 0 GET CONTRACT 'outputs' GET").await, "1/1");
    assert_eq!(top("[ DIV ] 0 GET CONTRACT 'field' GET").await, "'leaving'");
    assert_eq!(top("[ MAP ] 0 GET CONTRACT 'inputs' GET").await, "2/1");
    assert_eq!(
        top("[ SORT ] 0 GET CONTRACT KEYS").await,
        "[ 'inputs' 'outputs' 'field' ]"
    );
}

#[tokio::test]
async fn contract_infers_a_user_word_and_a_block_in_one_shape() {
    let code = "[ 42 PRINT ] 'SHOUT' DEF [ SHOUT ] 0 GET CONTRACT [ 42 PRINT ] CONTRACT";
    let interp = run(code).await;
    let stack = interp.get_stack();
    assert_eq!(stack.len(), 2);
    assert_eq!(
        stack[0], stack[1],
        "CONTRACT of a User Word is CONTRACT of its body"
    );
    assert_eq!(
        top("[ 42 PRINT ] CONTRACT KEYS").await,
        "[ 'inputs' 'outputs' 'field' ]"
    );
    // A key both carry is answered in one vocabulary, so a block compares
    // directly with the Core Word it calls.
    for key in ["inputs", "outputs", "field"] {
        assert_eq!(
            top(&format!("[ DIV ] 0 GET CONTRACT '{key}' GET")).await,
            top(&format!("[ DIV ] CONTRACT '{key}' GET")).await,
            "{key}"
        );
    }
}

#[tokio::test]
async fn contract_reads_the_block_and_restores_a_bad_operand() {
    assert_eq!(
        top("[ 1 ] 'B' BIND B B CONTRACT 'inputs' GET").await,
        "[ 1/1 ] 0/1"
    );
    for source in ["1 CONTRACT", "NIL CONTRACT"] {
        let mut interp = Interpreter::new();
        assert!(interp.execute(source).await.is_err(), "accepted {source}");
        assert_eq!(interp.stack.len(), 1, "operand was not restored: {source}");
    }
}

#[tokio::test]
async fn contract_projects_not_found_for_an_unknown_name() {
    assert_eq!(
        top("[ NOPE ] 0 GET CONTRACT NIL-REASON").await,
        "'notFound'"
    );
    assert_eq!(top("7 'N' BIND [ N ] 0 GET CONTRACT NIL?").await, "TRUE");
}
