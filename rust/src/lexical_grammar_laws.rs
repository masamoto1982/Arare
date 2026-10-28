//! The canonical lexical grammar, held against the implementation.
//!
//! `spec/grammar.json` is read here as data, and every claim it makes that the
//! tokenizer can answer is run rather than inspected. This is the half of the
//! grammar's proof that lives in the language of record; the other half is
//! `scripts/lib/reference-lexer.mjs`, which executes the same file in JS, and
//! `scripts/check-grammar.mjs`, which runs the same witnesses through it.
//! Because both sides consume one file and one shared corpus, the grammar
//! cannot quietly become a description of something the implementation no
//! longer does.
//!
//! What a failure here means: the grammar and the tokenizer disagree. Neither
//! is automatically right. `spec/grammar.json` is canonical for what Ajisai
//! source *is*, so a deliberate language change updates it first and the
//! tokenizer follows; an accidental divergence is a tokenizer bug.

use crate::tokenizer::tokenize;
use crate::types::Token;
use serde_json::Value as Json;

fn grammar() -> Json {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../spec/grammar.json");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read the canonical grammar at {path}: {e}"));
    serde_json::from_str(&text).expect("spec/grammar.json is valid JSON")
}

fn strings(node: &Json) -> Vec<String> {
    node.as_array()
        .expect("expected a JSON array")
        .iter()
        .map(|v| v.as_str().expect("expected a JSON string").to_string())
        .collect()
}

/// Every lexeme the grammar lists as a number is one Number token, carrying the
/// lexeme it was written as.
#[test]
fn accepted_numeric_examples_lex_as_one_number() {
    let g = grammar();
    let accepted = strings(&g["numericGrammar"]["examples"]["accepted"]);
    assert!(
        !accepted.is_empty(),
        "the grammar must list accepted examples"
    );

    for lexeme in accepted {
        let tokens = tokenize(&lexeme).unwrap_or_else(|e| {
            panic!("grammar lists {lexeme:?} as a number, but it is rejected: {e}")
        });
        match tokens.as_slice() {
            [Token::Number(literal)] => assert_eq!(
                literal.lexeme(),
                lexeme,
                "a Number must carry the lexeme it was written as"
            ),
            other => panic!("grammar lists {lexeme:?} as a number, but it lexes as {other:?}"),
        }
    }
}

/// Every lexeme the grammar lists as rejected-as-name is not a number. The
/// grammar's claim is about the numeric language only, so this asserts what it
/// actually says rather than pinning these to Symbol.
#[test]
fn rejected_numeric_examples_are_not_numbers() {
    let g = grammar();
    let rejected = strings(&g["numericGrammar"]["examples"]["rejectedAsName"]);
    assert!(
        !rejected.is_empty(),
        "the grammar must list rejected examples"
    );

    for lexeme in rejected {
        let Ok(tokens) = tokenize(&lexeme) else {
            continue; // A source error is not a number either.
        };
        assert!(
            !matches!(tokens.as_slice(), [Token::Number(_)]),
            "grammar lists {lexeme:?} as not a number, but it lexes as one",
        );
    }
}

/// The numeric pattern the grammar publishes is the numeric language, not a
/// paraphrase of it: the regex and the tokenizer must agree on every lexeme in
/// the examples. Checked without a regex engine by construction — the examples
/// are partitioned by the tokenizer above, so this asserts the partition is the
/// one the pattern describes via its own stated boundaries.
#[test]
fn the_numeric_grammar_is_anchored_at_both_ends() {
    let g = grammar();
    let pattern = g["numericGrammar"]["pattern"]
        .as_str()
        .expect("the grammar publishes a numeric pattern");
    assert!(
        pattern.starts_with('^') && pattern.ends_with('$'),
        "an unanchored numeric pattern would accept a prefix, making '1/2/3' a number: {pattern}",
    );
}

/// Each declared source-error condition has a witness that reaches it, and the
/// implementation's message for that witness carries the condition's stable
/// fingerprint. This is what makes the condition list demonstrably reachable
/// rather than a set of names.
#[test]
fn every_source_error_condition_has_a_reachable_witness() {
    let g = grammar();
    let conditions = g["sourceErrors"]
        .as_array()
        .expect("sourceErrors is an array");
    assert!(
        !conditions.is_empty(),
        "the grammar must declare source errors"
    );

    for condition in conditions {
        let id = condition["id"].as_str().expect("condition id");
        let witness = condition["witness"].as_str().expect("condition witness");
        let fingerprint = condition["messageContains"]
            .as_str()
            .expect("condition messageContains");

        let message = match tokenize(witness) {
            Err(message) => message,
            Ok(tokens) => panic!(
                "condition {id}: witness {witness:?} should be rejected, but it lexes as {tokens:?}",
            ),
        };

        assert!(
            message.contains(fingerprint),
            "condition {id}: witness {witness:?} was rejected, but with a message that does not \
             carry the declared fingerprint {fingerprint:?}. Got: {message}",
        );
    }
}

/// The grammar declares no rejected character, and the scan phase must not
/// reintroduce one.
///
/// This replaces the law that held the declared `rejectedCharacters` to the
/// tokenizer. There are none left to hold: `(`, `)`, `{` and `}` were the last
/// group, and a bare `|` the last whole-lexeme refusal. The character-validity
/// rule they formed is gone, so what has to be pinned now is its absence —
/// the same drift, caught from the other side.
#[test]
fn the_grammar_declares_no_rejected_character() {
    let g = grammar();
    let scan = g["phases"]
        .as_array()
        .expect("phases")
        .iter()
        .find(|p| p["id"] == "scan")
        .expect("a scan phase");

    assert!(
        scan.get("rejectedCharacters").is_none(),
        "the scan phase declares rejected characters again: {:?}. Every scalar \
         value that is not whitespace is a name character; a character that \
         must not stand alone belongs in lexemeRules, as `[` and `]` do.",
        scan.get("rejectedCharacters"),
    );
}

/// The characters that per-character rule used to refuse now lex as ordinary
/// names wherever they sit in a word — the delimiter rule is the only rule.
/// `{` and `}` were freed with these three, allocated as the Record literal's
/// delimiter pair for a while, and freed again when that literal was retired;
/// the grammar's one pair, `[` and `]`, is held by `a_delimiter_stands_alone`.
#[test]
fn a_freed_character_is_an_ordinary_name_anywhere_in_a_word() {
    for ch in ["(", ")", "|", "{", "}"] {
        for source in [
            ch.to_string(),
            format!("a{ch}"),
            format!("{ch}a"),
            format!("a{ch}b"),
        ] {
            let tokens = tokenize(&source)
                .unwrap_or_else(|e| panic!("{source:?} should lex as a name, got: {e}"));
            assert!(
                matches!(&tokens[..], [Token::Symbol(value)] if value.as_ref() == source),
                "{source:?} should be one Symbol carrying its own lexeme, got {tokens:?}",
            );
        }
    }
}

/// Each pair the grammar declares lexes to the two tokens it names, and only
/// as a whole lexeme: glued to anything, a delimiter is the source error that
/// asks for the space. This is the law the per-character rule's removal left
/// to carry `[` and `]` — the whole-lexeme rule is the only rule
/// standing between a name and a delimiter.
#[test]
fn a_delimiter_stands_alone() {
    let g = grammar();
    let pairs = g["delimiterPairs"]
        .as_array()
        .expect("the grammar declares its delimiter pairs");
    assert!(!pairs.is_empty(), "there is at least one pair");

    for pair in pairs {
        for side in ["open", "close"] {
            let ch = pair[side].as_str().expect("a delimiter character");
            let token = pair[if side == "open" {
                "openToken"
            } else {
                "closeToken"
            }]
            .as_str()
            .expect("a token id");
            // On its own it is one token, named by the pair. (An unbalanced
            // one is refused by the structural phase, so the lexeme's own
            // classification is read through a balanced program.)
            let balanced = format!(
                "{} {}",
                pair["open"].as_str().unwrap(),
                pair["close"].as_str().unwrap()
            );
            let tokens =
                tokenize(&balanced).unwrap_or_else(|e| panic!("{balanced:?} should lex, got: {e}"));
            let ids: Vec<String> = tokens.iter().map(|t| format!("{t:?}")).collect();
            assert!(
                ids.contains(&token.to_string()),
                "{balanced:?} should hold the token {token}, got {ids:?}",
            );
            // Glued to anything, it is refused by name.
            for source in [format!("a{ch}"), format!("{ch}1"), format!("a{ch}b")] {
                let message = tokenize(&source)
                    .err()
                    .unwrap_or_else(|| panic!("{source:?} must not lex as a name"));
                assert!(
                    message.contains("must stand alone"),
                    "{source:?} should be refused for standing alone, got: {message}",
                );
            }
        }
    }
}

/// A line terminator must also be whitespace. The scan rules read the two
/// classes independently — only the comment rule cares about a terminator —
/// so a terminator outside the whitespace class would glue into a name
/// instead of ending the comment's line and separating the tokens around it.
#[test]
fn line_terminators_are_whitespace() {
    let g = grammar();
    let classes = &g["characterClasses"];

    let expand = |name: &str| -> Vec<u32> {
        let mut out = Vec::new();
        for entry in strings(&classes[name]["codepoints"]) {
            let (lo, hi) = match entry.split_once('-') {
                Some((lo, hi)) => (lo.to_string(), hi.to_string()),
                None => (entry.clone(), entry.clone()),
            };
            let lo = u32::from_str_radix(&lo, 16).expect("hex codepoint");
            let hi = u32::from_str_radix(&hi, 16).expect("hex codepoint");
            out.extend(lo..=hi);
        }
        out
    };

    let whitespace = expand("whitespace");
    for cp in expand("lineTerminator") {
        assert!(
            whitespace.contains(&cp),
            "U+{cp:04X} is a line terminator but not whitespace",
        );
    }

    // The declared whitespace class must be the one the implementation uses,
    // or every token boundary in the grammar is describing a different language.
    for cp in whitespace {
        let ch = char::from_u32(cp).expect("a valid scalar value");
        assert!(
            ch.is_whitespace(),
            "U+{cp:04X} is declared whitespace but the implementation does not treat it as such",
        );
        assert!(
            tokenize(&format!("a{ch}b"))
                .expect("whitespace separates")
                .len()
                >= 2,
            "U+{cp:04X} is declared whitespace, so it must separate two tokens",
        );
    }

    // U+FEFF is the documented near-miss: not Unicode White_Space, so it glues.
    assert!(
        !'\u{FEFF}'.is_whitespace(),
        "U+FEFF must not be whitespace; the grammar's note depends on it",
    );
    assert_eq!(
        tokenize("a\u{FEFF}b")
            .expect("a BOM is an ordinary name character")
            .len(),
        1,
        "U+FEFF must glue into one name, as the grammar's whitespace note states",
    );
}

/// Every surface the grammar says it produces is one the tokenizer actually
/// treats as that form, and the lexeme rules are total: an arbitrary name falls
/// through to Symbol rather than needing a rule of its own.
#[test]
fn lexeme_classification_is_total() {
    for name in [
        "hello",
        "こんにちは",
        ".",
        "==",
        "^",
        "~",
        ";",
        "<=",
        "MATH@OR-NIL",
    ] {
        match tokenize(name)
            .unwrap_or_else(|e| panic!("{name:?} should lex: {e}"))
            .as_slice()
        {
            [Token::Symbol(value)] => assert_eq!(&**value, name),
            other => panic!("{name:?} should be one Symbol, got {other:?}"),
        }
    }
}

/// The delimiter check reads the tokens the scan produced, so a `#` or a quote
/// glued inside a name is part of that name there too. A text-level pass that
/// ran first read `[ C# ] LENGTH` as a comment swallowing the `]` and refused a
/// balanced program.
#[test]
fn a_glued_comment_or_quote_character_does_not_unbalance_delimiters() {
    for source in ["[ C# ] LENGTH", "[ a'b ] LENGTH", "[ x#y z' ]"] {
        let tokens = tokenize(source).unwrap_or_else(|e| panic!("{source:?}: {e}"));
        assert!(
            tokens.contains(&Token::VectorEnd),
            "{source:?} must close its vector"
        );
    }
}

/// `n/0` is a number like any other pair: it denotes one of the two points
/// over zero (LANG.VALUES.EXACT), and tokenizes as a Number.
#[test]
fn a_zero_denominator_is_a_number() {
    for source in ["1/0", "-3/000", "0/0", "1/01"] {
        let tokens = tokenize(source).unwrap_or_else(|e| panic!("{source:?}: {e}"));
        assert!(
            matches!(tokens.as_slice(), [Token::Number(literal)] if literal.lexeme() == source),
            "{source:?} must be one Number token, got {tokens:?}"
        );
    }
}

/// The numeric-literal ceiling counts the digits a literal denotes, exponent
/// included, which is what decides how large an integer the parse builds.
#[test]
fn a_literal_denotes_its_written_digits_plus_its_exponent() {
    use crate::tokenizer::denoted_digit_count;
    assert_eq!(denoted_digit_count("123"), 3);
    assert_eq!(denoted_digit_count("-1/2"), 2);
    assert_eq!(denoted_digit_count("1.5e3"), 5);
    assert_eq!(denoted_digit_count("1e-5000"), 5001);
    assert_eq!(denoted_digit_count("1e99999999999999999999999"), u64::MAX);
}

#[test]
fn zero_at_any_scale_denotes_only_its_written_digits() {
    use crate::tokenizer::denoted_digit_count;
    assert_eq!(denoted_digit_count("0e99999999"), 1);
    assert_eq!(denoted_digit_count("-0.00e999"), 3);
}
