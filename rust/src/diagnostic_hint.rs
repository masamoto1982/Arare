//! One-line repairs appended to a diagnosis message.
//!
//! The error object a host reads is fixed (`category`, `resource`, `word`,
//! `line`, `column` — docs/dev/agent-cli-output-contract.md); a hint lives only
//! in the human-readable message, which LANG.OBSERVATION.DIAGNOSIS lets evolve.
//! No program behavior and no protocol field depends on it.
//!
//! Two questions are answered here:
//!
//! * A name that resolves to nothing: the line of current Ajisai that does
//!   what it meant — a retired Word's phrase from `spec/retired-words.json`,
//!   or an idiom from another language from [`FOREIGN_IDIOMS`] — and failing
//!   both, the Core names within a typo's distance of it.
//! * A Core Word called with too few operands or with an operand of the wrong
//!   domain: the Word's own one-line example (`documentation.syntax` in
//!   `spec/words.json`), preceded where a Word is commonly mistaken for
//!   another by a pointer to the right one ([`MISTAKEN_DOMAIN`]). A Word
//!   refused for its operands' shapes gets a pointer only where the refusal
//!   is a known misreading of what the Word does ([`MISTAKEN_SHAPE`]).
//!
//! Every example here is run by `diagnostic_hint_tests`, so a hint is
//! verified Ajisai rather than prose that resembles it.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::kernel::generated::{generated_word, GENERATED_WORDS};

/// Names from other languages a writer reaches for, each with what to do
/// instead and one runnable line that does it. A name here is never a Core
/// Word (`diagnostic_hint_tests` asserts it).
pub(crate) const FOREIGN_IDIOMS: &[(&str, &str, &str)] = &[
    (
        "DUP",
        "name the value with BIND and read the name twice",
        "5 'X' BIND X X",
    ),
    (
        "SWAP",
        "name both values with BIND and read them in the new order",
        "1 2 'B' BIND 'A' BIND B A",
    ),
    (
        "OVER",
        "name both values with BIND and read them in the order you need",
        "1 2 'B' BIND 'A' BIND A B A",
    ),
    (
        "ROT",
        "name the values with BIND and read them in the order you need",
        "1 2 3 'C' BIND 'B' BIND 'A' BIND B C A",
    ),
    (
        "IF",
        "push both candidates, then the truth, then SELECT",
        "'yes' 'no' 1 2 LT SELECT",
    ),
    (
        "ELSE",
        "push both candidates, then the truth, then SELECT",
        "'yes' 'no' 1 2 LT SELECT",
    ),
    (
        "THEN",
        "push both candidates, then the truth, then SELECT",
        "'yes' 'no' 1 2 LT SELECT",
    ),
    (
        "WHILE",
        "iterate over a finite Vector with MAP, FILTER, FOLD or SCAN",
        "1 5 RANGE [ 2 MUL ] MAP",
    ),
    (
        "LOOP",
        "iterate over a finite Vector with MAP, FILTER, FOLD or SCAN",
        "1 5 RANGE [ 2 MUL ] MAP",
    ),
    (
        "FOR",
        "iterate over a finite Vector with MAP, FILTER, FOLD or SCAN",
        "1 5 RANGE [ 2 MUL ] MAP",
    ),
    ("SUM", "fold with ADD from 0", "[ 1 2 3 ] 0 [ ADD ] FOLD"),
    (
        "PRODUCT",
        "fold with MUL from 1",
        "[ 1 2 3 4 ] 1 [ MUL ] FOLD",
    ),
    ("LEN", "the Word is LENGTH", "[ 1 2 3 ] LENGTH"),
    ("SIZE", "the Word is LENGTH", "[ 1 2 3 ] LENGTH"),
    (
        "COUNT",
        "LENGTH counts elements; TALLY counts each distinct one",
        "[ 1 2 3 ] LENGTH",
    ),
    ("APPEND", "CONCAT joins two Vectors", "[ 1 2 ] [ 3 ] CONCAT"),
    ("PUSH", "CONCAT joins two Vectors", "[ 1 2 ] [ 3 ] CONCAT"),
    (".", "PRINT writes a value to the output", "'hi' PRINT"),
    ("+", "Words are spelled out", "1 2 ADD"),
    ("-", "Words are spelled out", "5 2 SUB"),
    ("*", "Words are spelled out", "2 3 MUL"),
    ("/", "Words are spelled out", "1 3 DIV"),
    ("=", "Words are spelled out", "1 1 EQ"),
    ("<", "Words are spelled out", "1 2 LT"),
    (">", "Words are spelled out", "2 1 GT"),
    ("LET", "name a value with BIND", "5 'X' BIND X 1 ADD"),
    ("SET", "name a value with BIND", "5 'X' BIND X 1 ADD"),
    ("VAR", "name a value with BIND", "5 'X' BIND X 1 ADD"),
    (
        ":",
        "define a Word with a body Vector, a name and DEF",
        "[ 2 MUL ] 'DOUBLE' DEF 21 DOUBLE",
    ),
    (
        "DEFINE",
        "define a Word with a body Vector, a name and DEF",
        "[ 2 MUL ] 'DOUBLE' DEF 21 DOUBLE",
    ),
];

/// A Core Word given an operand of a domain it does not read, where what the
/// writer meant is a different Word: `(word, condition, got, pointer)`. The
/// pointer leads the Word's own example, and its backticked line is run by
/// `diagnostic_hint_tests` like every other.
pub(crate) const MISTAKEN_DOMAIN: &[(&str, &str, &str, &str)] = &[
    ("CONCAT", "nonVector", "String", "Strings are joined by JOIN over a Vector of them: `'a' 'b' 2 COLLECT JOIN`"),
    ("JOIN", "nonVector", "String", "JOIN reads one Vector of Strings and takes no separator; put it between the parts first: `[ 'a' 'b' ] [ 'E' BIND ', ' E 2 COLLECT ] MAP FLATTEN 1 DROP JOIN`"),
    ("GET", "nonContainer", "String", "a String is not a Vector of characters; take them with CHARS first: `'abc' CHARS 1 GET`"),
    ("LENGTH", "nonVector", "String", "a String is not a Vector of characters; take them with CHARS first: `'abc' CHARS LENGTH`"),
    ("REVERSE", "nonVector", "String", "a String is not a Vector of characters: `'abc' CHARS REVERSE JOIN`"),
];

/// A Core Word refused for its operands' shapes, where the refusal usually
/// means the writer read the Word as doing something it does not:
/// `(word, condition, pointer)`. The pointer is the whole repair, and its
/// backticked line is run by `diagnostic_hint_tests`.
///
/// SELECT chooses lane by lane, so two whole Vectors of different lengths are
/// `shapeMismatch` — the one error the lexicon-emergence experiment's subjects
/// kept making after SKILL.md said so (Phase 3, 2026-10-15).
pub(crate) const MISTAKEN_SHAPE: &[(&str, &str, &str)] = &[(
    "SELECT",
    "shapeMismatch",
    "SELECT chooses lane by lane; to choose one whole value by one truth, collect both and read one: `[ 1 2 3 ] [ 9 ] 2 COLLECT 0 1 FALSE SELECT GET`",
)];

/// The retired names that have a phrase, read once from the retired-word list.
fn retired_phrases() -> &'static HashMap<String, String> {
    static PHRASES: OnceLock<HashMap<String, String>> = OnceLock::new();
    PHRASES.get_or_init(|| {
        let spec: serde_json::Value =
            serde_json::from_str(include_str!("../../spec/retired-words.json"))
                .expect("spec/retired-words.json is valid JSON");
        let mut phrases = HashMap::new();
        for group in spec["groups"].as_array().into_iter().flatten() {
            for (name, phrase) in group["phrases"].as_object().into_iter().flatten() {
                if let Some(phrase) = phrase.as_str() {
                    phrases.insert(name.clone(), phrase.to_string());
                }
            }
        }
        phrases
    })
}

/// What to write instead of `name`, a name that resolved to nothing — or
/// `None` when nothing useful can be said.
pub(crate) fn unknown_word_hint(name: &str) -> Option<String> {
    let upper = name.to_uppercase();
    if let Some(phrase) = retired_phrases().get(&upper) {
        return Some(format!("written as a phrase, for example `{phrase}`"));
    }
    if let Some((_, advice, example)) = FOREIGN_IDIOMS.iter().find(|(n, _, _)| *n == upper) {
        return Some(format!("{advice}: `{example}`"));
    }
    match suggest_core_words(name).as_slice() {
        [] => None,
        [only] => Some(format!("did you mean {only}?")),
        [rest @ .., last] => Some(format!("did you mean {} or {last}?", rest.join(", "))),
    }
}

/// The repair for a Core Word that failed for want of operands, for an
/// operand of the wrong domain or for operands of the wrong shapes: a pointer
/// to what was meant, if the failure is a known confusion, and otherwise —
/// for the first two — the Word's own one-line example.
/// `got` is the domain the message reports, when it reports one.
pub(crate) fn usage_hint(word: &str, condition: &str, got: Option<&str>) -> Option<String> {
    if let Some((_, _, _, pointer)) = MISTAKEN_DOMAIN
        .iter()
        .find(|(w, c, g, _)| *w == word && *c == condition && Some(*g) == got)
    {
        return Some((*pointer).to_string());
    }
    if let Some((_, _, pointer)) = MISTAKEN_SHAPE
        .iter()
        .find(|(w, c, _)| *w == word && *c == condition)
    {
        return Some((*pointer).to_string());
    }
    let reads_operands =
        condition == "stackUnderflow" || (condition.starts_with("non") && condition.len() > 3);
    if !reads_operands {
        return None;
    }
    generated_word(word)
        .and_then(|entry| entry.syntax)
        .map(|syntax| format!("for example `{syntax}`"))
}

/// The domain a declared message names after its final "got ", if any —
/// every declared message is written "expected …, got …".
pub(crate) fn reported_domain(message: &str) -> Option<&str> {
    let (_, tail) = message.rsplit_once("got ")?;
    tail.split(|c: char| !c.is_ascii_alphanumeric()).next()
}

const MAX_CANDIDATES: usize = 3;

/// Core names within a plausible typo distance of `written`, best first:
/// edit distance under a ceiling set by the name's length, at most three, ties
/// broken by name. The ranking is the one `tools/mcp-server/word-candidates.js`
/// answers `word_contract` with; both read
/// `tools/mcp-server/word-candidates.cases.json` as their test cases, so the two
/// cannot drift apart unnoticed.
pub(crate) fn suggest_core_words(written: &str) -> Vec<&'static str> {
    let written = written.trim().to_uppercase();
    let needle: Vec<char> = fold_full_width(&written).chars().collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let ceiling = match needle.len() {
        0..=3 => 1,
        4..=7 => 2,
        _ => 3,
    };
    let mut scored: Vec<(usize, &'static str)> = GENERATED_WORDS
        .iter()
        .map(|word| word.name)
        .filter(|name| *name != written)
        .map(|name| (edit_distance(&needle, name), name))
        .filter(|(distance, _)| *distance <= ceiling)
        .collect();
    scored.sort();
    scored
        .into_iter()
        .take(MAX_CANDIDATES)
        .map(|(_, name)| name)
        .collect()
}

/// Full-width ASCII (what a Japanese input method produces for `ＡＤＤ`) as
/// its ASCII form.
fn fold_full_width(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFF01 + 0x21).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// Levenshtein distance, two rows at a time.
fn edit_distance(left: &[char], right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];
    for (i, a) in left.iter().enumerate() {
        current[0] = i + 1;
        for (j, b) in right.iter().enumerate() {
            let substitution = previous[j] + usize::from(a != b);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[cfg(test)]
#[path = "diagnostic_hint_tests.rs"]
mod diagnostic_hint_tests;
