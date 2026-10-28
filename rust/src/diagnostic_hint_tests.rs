//! Every repair `diagnostic_hint` can append is verified Ajisai: each example
//! runs on a fresh interpreter, and each hint reaches the diagnosis it is for.

use super::{
    suggest_core_words, unknown_word_hint, usage_hint, FOREIGN_IDIOMS, MISTAKEN_DOMAIN,
    MISTAKEN_SHAPE,
};
use crate::interpreter::Interpreter;
use crate::kernel::generated::generated_word;

/// The backticked line inside a hint.
fn backticked(hint: &str) -> &str {
    let start = hint.find('`').expect("a hint carries a backticked example") + 1;
    let len = hint[start..].find('`').expect("the example is closed");
    &hint[start..start + len]
}

async fn runs(code: &str) {
    let mut interp = Interpreter::new();
    if let Err(err) = interp.execute(code).await {
        panic!("`{code}` must run, got: {err}");
    }
}

async fn diagnosis(code: &str) -> String {
    let mut interp = Interpreter::new();
    interp
        .execute(code)
        .await
        .expect_err("must raise an ERROR")
        .to_string()
}

fn retired_phrases() -> Vec<(String, String)> {
    let spec: serde_json::Value =
        serde_json::from_str(include_str!("../../spec/retired-words.json")).unwrap();
    let mut phrases = Vec::new();
    for group in spec["groups"].as_array().unwrap() {
        let names: Vec<&str> = group["names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_str().unwrap())
            .collect();
        for (name, phrase) in group["phrases"].as_object().into_iter().flatten() {
            assert!(
                names.contains(&name.as_str()),
                "phrase for {name}, which its group does not retire"
            );
            phrases.push((name.clone(), phrase.as_str().unwrap().to_string()));
        }
    }
    assert!(!phrases.is_empty());
    phrases
}

#[tokio::test]
async fn every_retired_phrase_runs_and_reaches_its_diagnosis() {
    for (name, phrase) in retired_phrases() {
        runs(&phrase).await;
        let message = diagnosis(&name).await;
        assert!(
            message.contains(&format!("`{phrase}`")),
            "{name}: {message}"
        );
    }
}

#[tokio::test]
async fn every_foreign_idiom_is_unknown_and_its_example_runs() {
    for (name, _, example) in FOREIGN_IDIOMS {
        assert!(generated_word(name).is_none(), "{name} is a Core Word");
        runs(example).await;
        let message = diagnosis(name).await;
        assert!(
            message.contains(&format!("`{example}`")),
            "{name}: {message}"
        );
    }
}

#[tokio::test]
async fn a_typo_is_answered_with_the_names_it_is_near() {
    assert_eq!(
        diagnosis("1 2 ADDD").await,
        "Unknown word: ADDD — did you mean ADD or AND?"
    );
    assert_eq!(
        diagnosis("[ 1 2 ] LENGHT").await,
        "Unknown word: LENGHT — did you mean LENGTH?"
    );
    assert_eq!(diagnosis("ZZZZZZZZZZ").await, "Unknown word: ZZZZZZZZZZ");
    assert_eq!(unknown_word_hint("ZZZZZZZZZZ"), None);
}

#[test]
fn the_ranking_is_the_one_word_contract_answers_with() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "../../tools/mcp-server/word-candidates.cases.json"
    ))
    .unwrap();
    for case in cases["cases"].as_array().unwrap() {
        let written = case[0].as_str().unwrap();
        let expected: Vec<&str> = case[1]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_str().unwrap())
            .collect();
        assert_eq!(suggest_core_words(written), expected, "{written}");
    }
}

#[tokio::test]
async fn a_mistaken_domain_points_to_the_word_meant() {
    for (word, condition, got, pointer) in MISTAKEN_DOMAIN {
        assert!(generated_word(word).is_some(), "{word} is not a Core Word");
        runs(backticked(pointer)).await;
        assert_eq!(
            usage_hint(word, condition, Some(got)).as_deref(),
            Some(*pointer)
        );
    }
    assert_eq!(
        diagnosis("'a' 'b' CONCAT").await,
        format!(
            "CONCAT: expected two Vectors, got String — {}",
            MISTAKEN_DOMAIN[0].3
        )
    );
    let join = diagnosis("[ 'a' 'b' ] ', ' JOIN").await;
    assert!(
        join.starts_with("JOIN: expected a Vector, got String — JOIN reads one Vector"),
        "{join}"
    );
}

#[tokio::test]
async fn a_known_shape_misreading_is_answered_with_what_was_meant() {
    for (word, condition, pointer) in MISTAKEN_SHAPE {
        assert!(generated_word(word).is_some(), "{word} is not a Core Word");
        runs(backticked(pointer)).await;
        assert_eq!(usage_hint(word, condition, None).as_deref(), Some(*pointer));
    }
    // Two whole Vectors chosen by one truth: the experiment's subjects wrote
    // this after an optional key was absent and the fallback was `[ ]`.
    let select = diagnosis("[ 'a' 'b' ] [ ] TRUE SELECT").await;
    assert!(
        select.starts_with("SELECT: expected shapes that align")
            && select.ends_with(MISTAKEN_SHAPE[0].2),
        "{select}"
    );
}

#[tokio::test]
async fn a_short_or_mistyped_operand_is_answered_with_the_words_own_example() {
    assert_eq!(
        diagnosis("'a b' TOKENIZE").await,
        "TOKENIZE: stack underflow — for example `'a,b,c' ',' TOKENIZE`"
    );
    assert_eq!(
        diagnosis("TRUE NUM").await,
        "NUM: expected a String, got Boolean — for example `'42' NUM`"
    );
    // A failure that is not about the operands' count or domain carries no
    // example: the example would not say what went wrong.
    let shape = diagnosis("[ 1 2 ] [ 1 2 3 ] ADD").await;
    assert!(!shape.contains("for example"), "{shape}");
}
