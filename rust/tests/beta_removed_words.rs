use ajisai_core::interpreter::host_lookup::resolve_host_lookup;
use ajisai_core::interpreter::Interpreter;
use ajisai_core::AjisaiError;

/// The names that were once Words and must stay unknown, read from
/// `spec/retired-words.json` — the one representation this suite and
/// `scripts/check-minimal-core.mjs` share. That gate asserts none of them is
/// canonical in `spec/words.json`; this suite asserts the runtime, the
/// compiled path and the host lookup do not resolve them either. The reasons
/// each name went, and the names that came back, are recorded there.
fn removed_words() -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../spec/retired-words.json");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read the retired-word list at {path}: {e}"));
    let spec: serde_json::Value =
        serde_json::from_str(&text).expect("spec/retired-words.json is valid JSON");
    let names: Vec<String> = spec["groups"]
        .as_array()
        .expect("spec/retired-words.json has a groups array")
        .iter()
        .flat_map(|group| {
            group["names"]
                .as_array()
                .expect("each retired-word group has a names array")
                .iter()
                .map(|name| {
                    name.as_str()
                        .expect("each retired name is a string")
                        .to_string()
                })
        })
        .collect();
    assert!(
        !names.is_empty(),
        "spec/retired-words.json names no retired Word"
    );
    names
}

#[tokio::test]
async fn removed_beta_words_are_unknown_at_runtime() {
    for word in &removed_words() {
        let mut interpreter = Interpreter::new();
        let error = match interpreter.execute(word).await {
            Ok(()) => panic!("removed Word {word} unexpectedly executed"),
            Err(error) => error,
        };
        assert!(
            matches!(error, AjisaiError::UnknownWord(ref name) if name == word),
            "{word} resolved through a stale runtime entry: {error}"
        );
    }
}

/// A removed name reached as a bare Symbol inside a `[ ]`-built Vector is
/// still an Unknown Word: building the literal does not resolve names (LANG.
/// VALUES.VECTOR), so the deleted entry cannot reappear by being constructed
/// as data rather than written directly as source (`REFLECT`, which used to
/// be the dedicated crossing for this, is gone along with the CodeBlock/
/// Vector split it crossed: code and data share one Vector domain and any
/// Vector is executable, so there is no separate boundary to test).
#[tokio::test]
async fn removed_beta_words_are_unknown_through_a_constructed_vector() {
    for word in &removed_words() {
        let mut interpreter = Interpreter::new();
        let source = format!("[ {word} ] EXEC");
        let error = match interpreter.execute(&source).await {
            Ok(()) => panic!("removed Word {word} executed from a constructed Vector"),
            Err(error) => error,
        };
        assert!(
            matches!(error, AjisaiError::UnknownWord(ref name) if name == word),
            "{word} resolved through a constructed Vector: {error}"
        );
    }
}

/// The compiled path a User Word body takes is the same inventory: defining a
/// body over a removed name is allowed (the body is data until called), and
/// calling it fails as an Unknown Word rather than dispatching a stale arm.
#[tokio::test]
async fn removed_beta_words_are_unknown_in_a_compiled_user_word() {
    for word in &removed_words() {
        let mut interpreter = Interpreter::new();
        let source = format!("[ 1 {word} ] 'CALL-REMOVED' DEF CALL-REMOVED");
        let error = match interpreter.execute(&source).await {
            Ok(()) => panic!("removed Word {word} executed from a compiled body"),
            Err(error) => error,
        };
        assert!(
            matches!(error, AjisaiError::UnknownWord(ref name) if name == word),
            "{word} resolved inside a block: {error}"
        );
    }
}

/// The host's lookup is the dictionary's own reading surface: it must not
/// describe a Word the inventory no longer has.
#[tokio::test]
async fn removed_beta_words_are_unknown_to_the_host_lookup() {
    for word in &removed_words() {
        let interpreter = Interpreter::new();
        let error = match resolve_host_lookup(&interpreter, word) {
            Ok(_) => panic!("the host lookup described removed Word {word}"),
            Err(error) => error,
        };
        assert!(
            matches!(error, AjisaiError::UnknownWord(ref name) if name == word),
            "{word} is still documented by the host lookup: {error}"
        );
    }
}

#[tokio::test]
async fn removed_eat_alias_is_not_a_no_op() {
    let mut interpreter = Interpreter::new();
    assert!(matches!(
        interpreter.execute(",").await,
        Err(AjisaiError::UnknownWord(name)) if name == ","
    ));
}
