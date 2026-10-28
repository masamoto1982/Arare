//! What `DEF` accepts as a Word's name.
//!
//! A Word is reached by writing its name as one token, so a name that cannot
//! be written is not a name. `DEF` used to take one anyway, and the entry it
//! made could be listed, hovered and exported but never called — which splits
//! exactly what LANG.DICTIONARY.RESOLUTION joins: "the host's lookup, hover,
//! the Reference, and execution must identify the same canonical entry."

use crate::interpreter::Interpreter;

/// Define `name` with a trivial body, and report what `DEF` said.
async fn def(name: &str) -> Result<(), String> {
    let mut interp = Interpreter::new();
    interp
        .execute(&format!("[ [ 1 ] ] '{}' DEF", name))
        .await
        .map_err(|e| e.to_string())
}

/// Every one of these lexes as something other than a single Symbol, so
/// writing it in source could never reach the definition: a bracket is its
/// own whitespace-delimited word (LANG.SOURCE.TEXT), a run of digits is a
/// Number, `#` opens a comment, whitespace separates two tokens, `OR-NIL`
/// is emitted as its own control token, and the empty string is no token
/// at all.
#[tokio::test]
async fn a_name_that_cannot_be_written_is_refused() {
    for name in ["A[B", "2]", "123", "#foo", "my word", ""] {
        let err = def(name)
            .await
            .expect_err(&format!("`{name}` should not be definable"));
        assert!(
            err.contains("is not a name"),
            "`{name}` should be refused as unwritable, got: {err}"
        );
    }
}

/// The rule rejects only what cannot be written. Punctuation that is an
/// ordinary name character still makes a name, and so does a non-ASCII one
/// — including `#` glued inside a name, which is part of that one word
/// rather than a comment now that whitespace is the sole delimiter.
#[tokio::test]
async fn an_ordinary_name_is_still_definable_and_callable() {
    for name in ["DOUBLE", "gentle", "合計", "a#b", "x^y", "1ST"] {
        def(name)
            .await
            .unwrap_or_else(|e| panic!("`{name}` should be definable, got: {e}"));

        let mut interp = Interpreter::new();
        interp
            .execute(&format!("[ [ 7 ] ] '{}' DEF {}", name, name))
            .await
            .unwrap_or_else(|e| panic!("`{name}` should be callable, got: {e}"));
        assert_eq!(
            interp.stack.len(),
            1,
            "`{name}` should have pushed its body"
        );
    }
}

/// A brace is an ordinary name, so a brace-block form is a call of the
/// undefined Word `{` and defines nothing.
#[tokio::test]
async fn a_brace_block_does_not_define_a_word() {
    let mut interp = Interpreter::new();
    let err = interp
        .execute("{ [ 2 ] MUL } 'DOUBLE' DEF")
        .await
        .expect_err("a brace block must not define a Word")
        .to_string();
    assert!(err.contains("Unknown word"), "got: {err}");
    assert!(
        !interp.user_words.contains_key("DOUBLE"),
        "the retired form must not have defined DOUBLE"
    );
}

/// A delimiter is not a name, so it cannot be one a Word is defined
/// under — the half of the allocation that reaches the dictionary.
#[tokio::test]
async fn a_delimiter_is_not_a_definable_name() {
    for name in ["[", "]"] {
        let err = def(name)
            .await
            .expect_err("a delimiter must not be definable")
            .to_string();
        assert!(
            err.contains("it is not a name"),
            "`{name}` should be refused for not being a name, got: {err}"
        );
    }
}

/// A freed character is a perfectly ordinary name, so a Word may be
/// defined under one and called by writing it. This is the other half of
/// the rule above: nothing is reserved, so nothing is refused.
#[tokio::test]
async fn a_freed_character_is_a_definable_name() {
    for name in ["(", ")", "|", "f(x)"] {
        def(name)
            .await
            .unwrap_or_else(|e| panic!("`{name}` should be definable, got: {e}"));

        let mut interp = Interpreter::new();
        interp
            .execute(&format!("[ [ 7 ] ] '{}' DEF {}", name, name))
            .await
            .unwrap_or_else(|e| panic!("`{name}` should be callable, got: {e}"));
        assert_eq!(
            interp.stack.len(),
            1,
            "`{name}` should have pushed its body"
        );
    }
}

/// Names are matched through the canonical normalization, so a lowercase
/// definition answers to either spelling. The rule must not disturb that.
#[tokio::test]
async fn a_lowercase_name_still_answers_to_both_spellings() {
    for call in ["gentle", "GENTLE"] {
        let mut interp = Interpreter::new();
        interp
            .execute(&format!("[ [ 7 ] ] 'gentle' DEF {}", call))
            .await
            .unwrap_or_else(|e| panic!("`{call}` should reach the word, got: {e}"));
        assert_eq!(interp.stack.len(), 1);
    }
}

/// A symbol that was once a second spelling of a Core Word is an ordinary
/// name now: `+` and `<` lex as Symbols, so they are definable like any
/// other, and the Word defined under one is reached by writing it.
#[tokio::test]
async fn a_freed_symbol_is_an_ordinary_definable_name() {
    for name in ["+", "<"] {
        let mut interp = Interpreter::new();
        interp
            .execute(&format!("[ 1 ] '{name}' DEF {name}"))
            .await
            .unwrap_or_else(|e| panic!("`{name}` should be definable, got: {e}"));
        assert_eq!(interp.stack.len(), 1);
    }
}
