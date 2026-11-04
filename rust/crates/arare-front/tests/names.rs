//! AR-VER-227: names in the front end (AR-LLR-028). `'X'` is a name as text,
//! for BIND; a bare name is accepted where the source also writes it as text,
//! and refused as an unknown Word otherwise; text that cannot be a name, and
//! a quote left open, are refused where they are written.

use arare_front::{is_name, lex, parse, FrontErrorKind as K, Item, TokenKind as T, Word};

fn refused(src: &str) -> (K, usize, usize) {
    let e = parse(src).expect_err(src);
    (e.kind, e.line, e.column)
}

#[test]
fn ar_ver_227_text_tokens() {
    let tokens = lex("5 'N' BIND N\n['A' 'B']");
    let kinds: Vec<T> = tokens.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        [
            T::Integer,
            T::Text,
            T::Name,
            T::Name,
            T::Open,
            T::Text,
            T::Text,
            T::Close
        ]
    );
    // The text is what lies between the quotes; the column is the opening one.
    assert_eq!((tokens[1].text.as_str(), tokens[1].column), ("N", 3));
    assert_eq!((tokens[5].text.as_str(), tokens[5].line), ("A", 2));
    // The closing quote ends the token, and a bracket inside quotes is text.
    assert_eq!(lex("'A'B")[1].text, "B");
    assert_eq!(lex("'[x]'")[0].text, "[x]");
    // An unclosed quote keeps its opening mark and runs to the line's end.
    assert_eq!(lex("'AB C\n1")[0].text, "'AB C");
    assert_eq!(lex("''")[0].text, "");
}

#[test]
fn ar_ver_227_items() {
    let p = parse("5 'N' BIND N [ N 'M' BIND M ]").unwrap();
    assert_eq!(
        p.items,
        [
            Item::Int(5),
            Item::Text("N".into()),
            Item::Word(Word::Bind),
            Item::Name("N".into()),
            Item::Quote(vec![
                Item::Name("N".into()),
                Item::Text("M".into()),
                Item::Word(Word::Bind),
                Item::Name("M".into()),
            ]),
        ]
    );
    // A name written as text anywhere in the source may be used anywhere:
    // whether it is bound where it runs is the interpreter's question.
    assert_eq!(parse("X 'X'").unwrap().items[0], Item::Name("X".into()));
    // Names are case-sensitive and may hold digits, `-` and `?`.
    assert!(parse("1 'x-1?' BIND x-1?").is_ok());
    assert_eq!(Word::from_name("BIND"), Some(Word::Bind));
}

#[test]
fn ar_ver_227_refusals() {
    // A bare name no text names cannot be bound: it is an unknown Word.
    assert_eq!(refused("1 'N' BIND M"), (K::UnknownWord("M".into()), 1, 12));
    assert_eq!(refused("1 'n' BIND N"), (K::UnknownWord("N".into()), 1, 12));
    // Text that cannot be a name.
    for (src, text) in [
        ("1 '' BIND", ""),
        ("1 '12' BIND", "12"),
        ("1 '-3/4' BIND", "-3/4"),
        ("1 'ADD' BIND", "ADD"),
        ("1 'a b' BIND", "a b"),
        ("1 'a#' BIND", "a#"),
        ("1 '[x]' BIND", "[x]"),
    ] {
        assert_eq!(refused(src), (K::BadName(text.into()), 1, 3), "{src}");
    }
    assert_eq!(refused("1\n 'N BIND"), (K::UnclosedText, 2, 2));
    assert_eq!(
        parse("1 'ADD' BIND").unwrap_err().to_string(),
        "1:3: 'ADD' cannot be a name: it is empty, a number, a Word, or holds a space, bracket or #"
    );
    assert_eq!(
        parse("'N").unwrap_err().to_string(),
        "1:1: a ' is never closed on its line"
    );
}

#[test]
fn ar_ver_227_is_name() {
    for good in ["X", "x", "ARARE_W", "a-1", "NIL?", "1x"] {
        assert!(is_name(good), "{good}");
    }
    for bad in [
        "", "1", "-2", "0.5", "1/3", "BIND", "SUM", "a b", "a[", "a]", "a'", "a#", "\t",
    ] {
        assert!(!is_name(bad), "{bad:?}");
    }
}
