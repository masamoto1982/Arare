//! Test suite for what the Core vocabulary admits.

use crate::interpreter::Interpreter;
use crate::kernel::generated::GENERATED_WORDS;

#[tokio::test]
async fn core_vocabulary_holds_words_and_not_surface_forms() {
    let interp = Interpreter::new();
    assert!(interp.core_vocabulary.contains_key("ADD"));
    assert!(interp.core_vocabulary.contains_key("MAP"));
    assert!(interp.core_vocabulary.contains_key("DEF"));

    // A retired Word and a lexical surface form are both absent: the
    // vocabulary holds runtime Words only.
    assert!(!interp.core_vocabulary.contains_key("FRAME"));
    assert!(!interp.core_vocabulary.contains_key("'"));
}

#[tokio::test]
async fn def_registers_a_user_word() {
    let mut interp = Interpreter::new();
    interp.execute("[ 1 ] 'X' DEF").await.unwrap();
    assert!(interp.user_words.contains_key("X"));
}

#[test]
fn now_is_not_a_core_word() {
    assert!(GENERATED_WORDS.iter().all(|word| word.name != "NOW"));
}
