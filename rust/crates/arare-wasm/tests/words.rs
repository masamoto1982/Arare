//! AR-VER-232: the Reference's registry of Words (spec/words.json,
//! AR-LLR-038) lists exactly the Words the front end knows, once each, in
//! the order `Word::ALL` gives them, so neither the Reference nor the
//! Playground's Dictionary can lose or invent a Word.

use arare_front::Word;

#[test]
fn ar_ver_232_words_json_lists_every_word_once() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../spec/words.json");
    let text = std::fs::read_to_string(path).expect("spec/words.json");
    let names: Vec<&str> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("\"name\": \""))
        .map(|rest| rest.split('"').next().expect("a closing quote"))
        .collect();
    let words: Vec<&str> = Word::ALL.iter().map(|w| w.name()).collect();
    assert_eq!(names, words);
}
