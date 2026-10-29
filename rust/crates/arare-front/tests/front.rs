//! AR-VER-210: the front end (AR-LLR-011, AR-LLR-012). Normal cases of every
//! token form and every way a source can be refused, with where it is
//! refused; the MC/DC table for folding a quantized literal.

use arare_front::{lex, parse, FrontErrorKind as K, Item, TokenKind as T, Word};
use arare_kernel::{Num, Val, Width};

fn kinds(src: &str) -> Vec<T> {
    lex(src).into_iter().map(|t| t.kind).collect()
}

fn refused(src: &str) -> (K, usize, usize) {
    let e = parse(src).expect_err(src);
    (e.kind, e.line, e.column)
}

#[test]
fn ar_ver_210_tokens() {
    assert_eq!(
        kinds("12 -3 0.5 -1.25 1/3 -2/0 ADD NIL?"),
        [
            T::Integer,
            T::Integer,
            T::Decimal,
            T::Decimal,
            T::Fraction,
            T::Fraction,
            T::Name,
            T::Name
        ]
    );
    // Brackets are tokens even without spaces; `#` comments run to the line's end.
    assert_eq!(
        kinds("[1 2]ADD # a comment [ ]\n3"),
        [
            T::Open,
            T::Integer,
            T::Integer,
            T::Close,
            T::Name,
            T::Integer
        ]
    );
    // Near misses are names, and an unknown name is refused when parsed.
    assert_eq!(kinds("1. .5 1/ -x 1e3 --1"), [T::Name; 6]);
    // A bracket ends a token on either side; near-numeric names stay names.
    assert_eq!(
        kinds("1[2]3"),
        [T::Integer, T::Open, T::Integer, T::Close, T::Integer]
    );
    assert_eq!(kinds("a.5 1.x x/1 1/x -. -/"), [T::Name; 6]);
    let t = &lex("  ADD\n [ 7")[2];
    assert_eq!((t.line, t.column, t.text.as_str()), (2, 4, "7"));
}

#[test]
fn ar_ver_210_items() {
    let p = parse("1 -2 [ 3 [ ADD ] ] SUM").unwrap();
    assert_eq!(
        p.items,
        [
            Item::Int(1),
            Item::Int(-2),
            Item::Quote(vec![Item::Int(3), Item::Quote(vec![Item::Word(Word::Add)])]),
            Item::Word(Word::Sum),
        ]
    );
    for w in Word::ALL {
        assert_eq!(Word::from_name(w.name()), Some(w));
    }
    assert_eq!(Word::from_name("add"), None);
}

#[test]
fn ar_ver_210_quantized_literals() {
    let folded = |src: &str| match &parse(src).unwrap().items[..] {
        [Item::Num(n)] => *n,
        other => panic!("{src}: {other:?}"),
    };
    assert_eq!(
        folded("0.75 8 -4 QUANTIZE"),
        Num::from_ratio(3, 4, Width::W8, -4).unwrap()
    );
    assert_eq!(folded("-1.5 16 -1 QUANTIZE").val(), Val::Fin(-3));
    assert_eq!(folded("2/6 32 -3 QUANTIZE").val(), Val::Fin(3)); // 1/3 · 8 = 2.67 → 3
    assert_eq!(folded("1/0 8 0 QUANTIZE").val(), Val::Inf);
    assert_eq!(folded("-1/0 8 0 QUANTIZE").val(), Val::Inf);
    assert_eq!(folded("0/0 64 0 QUANTIZE").val(), Val::Bot);
    assert_eq!(
        folded("0.123456789012345678 64 -31 QUANTIZE").width(),
        Width::W64
    );
    // An integer followed by QUANTIZE is left to run: QUANTIZE is a Word too.
    assert_eq!(parse("3 8 -1 QUANTIZE").unwrap().items.len(), 4);
}

/// Folding a non-integer literal needs an integer width (A), an integer
/// exponent (B) and QUANTIZE (C) right after it; each, missing alone, refuses.
#[test]
fn ar_ver_210_mcdc_fold() {
    assert!(parse("0.5 8 -1 QUANTIZE").is_ok()); //            (T, T, T)
    assert!(matches!(
        refused("0.5 x -1 QUANTIZE").0,
        K::UnquantizedLiteral(_)
    )); // A
    assert!(matches!(
        refused("0.5 8 0.5 QUANTIZE").0,
        K::UnquantizedLiteral(_)
    )); // B
    assert!(matches!(
        refused("0.5 8 -1 ADD").0,
        K::UnquantizedLiteral(_)
    )); //      C
}

#[test]
fn ar_ver_210_refusals() {
    assert_eq!(refused("1 2 FOO"), (K::UnknownWord("FOO".into()), 1, 5));
    assert_eq!(refused("[ 1 [ 2 ]").0, K::UnclosedQuote);
    assert_eq!(refused("1 ]\n"), (K::UnexpectedClose, 1, 3));
    assert_eq!(
        refused("9223372036854775807").0,
        K::IntegerOutOfRange("9223372036854775807".into())
    );
    assert!(parse("9223372036854775806").is_ok());
    assert_eq!(
        refused("99999999999999999999").0,
        K::IntegerOutOfRange("99999999999999999999".into())
    );
    assert!(matches!(
        refused("0.1234567890123456789012 8 0 QUANTIZE").0,
        K::LiteralTooPrecise(_)
    ));
    assert!(matches!(
        refused("1/99999999999999999999 8 0 QUANTIZE").0,
        K::LiteralTooPrecise(_)
    ));
    assert!(matches!(refused("0.5").0, K::UnquantizedLiteral(_)));
    assert!(matches!(refused("0.5 8").0, K::UnquantizedLiteral(_)));
    assert_eq!(
        refused("\n  0.5 12 -1 QUANTIZE"),
        (K::BadQuantize("12 -1".into()), 2, 7)
    );
    assert_eq!(
        refused("0.5 8 -32 QUANTIZE"),
        (K::BadQuantize("8 -32".into()), 1, 7)
    );
    assert!(parse("0.5 8 31 QUANTIZE").is_ok());
    for (src, message) in [
        ("[ 1", "1:3: a [ is never closed"),
        ("]", "1:1: a ] closes nothing"),
        (
            "99999999999999999999",
            "1:1: 99999999999999999999 is beyond the widest finite range",
        ),
        (
            "1/99999999999999999999 8 0 QUANTIZE",
            "1:1: 1/99999999999999999999 needs more than 64 bits in lowest terms",
        ),
        (
            "0.5 12 0 QUANTIZE",
            "1:5: QUANTIZE 12 0: the width must be 8, 16, 32 or 64 and the exponent within ±31",
        ),
        ("NOPE", "1:1: unknown Word NOPE"),
    ] {
        assert_eq!(parse(src).unwrap_err().to_string(), message);
    }
    let shown = parse("0.5").unwrap_err().to_string();
    assert!(
        shown.starts_with("1:1:") && shown.contains("QUANTIZE"),
        "{shown}"
    );
}
