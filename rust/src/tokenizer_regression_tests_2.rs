//! Regression test suite for `crate::tokenizer` (continued).

use crate::tokenizer::tokenize;
use crate::types::Token;

#[test]
fn test_whitespace_handling() {
    let result = tokenize("1\t2  3   4").unwrap();
    assert_eq!(
        result,
        vec![
            Token::number("1"),
            Token::number("2"),
            Token::number("3"),
            Token::number("4"),
        ]
    );
}

#[test]
fn test_empty_input() {
    let result = tokenize("").unwrap();
    assert_eq!(result, vec![]);
}

#[test]
fn test_only_whitespace() {
    let result = tokenize("   \n  \t  ").unwrap();
    assert_eq!(result, vec![]);
}

#[test]
fn test_symbol_with_special_chars() {
    let result = tokenize("PRINT? SET!").unwrap();
    assert_eq!(
        result,
        vec![Token::Symbol("PRINT?".into()), Token::Symbol("SET!".into()),]
    );
}

#[test]
fn test_fraction_literal() {
    let result = tokenize("1/3").unwrap();
    assert_eq!(result, vec![Token::number("1/3")]);

    let result2 = tokenize("-1/3").unwrap();
    assert_eq!(result2, vec![Token::number("-1/3")]);

    let result3 = tokenize("1/3 + 2/5").unwrap();
    assert_eq!(
        result3,
        vec![
            Token::number("1/3"),
            Token::Symbol("+".into()),
            Token::number("2/5"),
        ]
    );
}

#[test]
fn test_fraction_in_vector() {
    let result = tokenize("[ 1/2 3/4 ]").unwrap();
    assert_eq!(
        result,
        vec![
            Token::VectorStart,
            Token::number("1/2"),
            Token::number("3/4"),
            Token::VectorEnd,
        ]
    );
}

#[test]
fn test_invalid_fraction() {
    let result = tokenize("1/").unwrap();
    assert_eq!(result, vec![Token::Symbol("1/".into())]);

    let result2 = tokenize("1/a").unwrap();
    assert_eq!(result2, vec![Token::Symbol("1/a".into())]);
}

#[test]
fn test_unclosed_string_error() {
    let result = tokenize("'unclosed string");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unclosed literal"));
}

/// A run of `.` lexes as one Symbol. It is not a modifier — the one
/// modifier axis is spelled `,,` — so the dictionary simply does not have it.
#[test]
fn test_dot_lexes_as_a_symbol() {
    let result = tokenize(". + 3").unwrap();
    assert_eq!(
        result,
        vec![
            Token::Symbol(".".into()),
            Token::Symbol("+".into()),
            Token::number("3"),
        ]
    );

    let result2 = tokenize(".. + 3").unwrap();
    assert_eq!(
        result2,
        vec![
            Token::Symbol("..".into()),
            Token::Symbol("+".into()),
            Token::number("3"),
        ]
    );
}

/// `;` and `;;` carry no meaning: they lex as ordinary Symbols the
/// dictionary does not have, rather than erroring in the lexer or expanding
/// into a modifier pair.
#[test]
fn test_semicolon_is_an_ordinary_symbol() {
    assert_eq!(
        tokenize("; +").unwrap(),
        vec![Token::Symbol(";".into()), Token::Symbol("+".into())]
    );
    assert_eq!(
        tokenize(";; +").unwrap(),
        vec![Token::Symbol(";;".into()), Token::Symbol("+".into())]
    );
}

#[test]
fn test_dot_operator_with_vector() {
    let result = tokenize("[ 1 2 3 ] . LENGTH").unwrap();
    assert_eq!(
        result,
        vec![
            Token::VectorStart,
            Token::number("1"),
            Token::number("2"),
            Token::number("3"),
            Token::VectorEnd,
            Token::Symbol(".".into()),
            Token::Symbol("LENGTH".into()),
        ]
    );

    let result2 = tokenize("a b c [ 1 ] .. GET").unwrap();
    assert_eq!(
        result2,
        vec![
            Token::Symbol("a".into()),
            Token::Symbol("b".into()),
            Token::Symbol("c".into()),
            Token::VectorStart,
            Token::number("1"),
            Token::VectorEnd,
            Token::Symbol("..".into()),
            Token::Symbol("GET".into()),
        ]
    );
}

/// `[` and `]` are reserved structural words: like every other Ajisai
/// word — and like Forth's own `[` and `]` — they must stand alone,
/// separated by whitespace. Whitespace is the sole token delimiter
/// (LANG.SOURCE.TEXT), so a bracket glued to anything else is a source
/// error asking for the missing space, not an implicit split.
#[test]
fn test_bracket_without_space_is_rejected() {
    for glued in ["[1]", "[1 2 3]", "[[1][2]]", "[1 2]+[3 4]"] {
        let err = tokenize(glued).unwrap_err();
        assert!(
            err.contains("must stand alone"),
            "`{glued}` should be rejected for missing whitespace, got: {err}"
        );
    }
}

#[test]
fn test_bracket_with_space_still_works() {
    let result = tokenize("[ 1 ]").unwrap();
    assert_eq!(
        result,
        vec![Token::VectorStart, Token::number("1"), Token::VectorEnd,]
    );

    let result2 = tokenize("[ 1 2 3 ]").unwrap();
    assert_eq!(
        result2,
        vec![
            Token::VectorStart,
            Token::number("1"),
            Token::number("2"),
            Token::number("3"),
            Token::VectorEnd,
        ]
    );

    let result3 = tokenize("[ [ 1 ] [ 2 ] ]").unwrap();
    assert_eq!(
        result3,
        vec![
            Token::VectorStart,
            Token::VectorStart,
            Token::number("1"),
            Token::VectorEnd,
            Token::VectorStart,
            Token::number("2"),
            Token::VectorEnd,
            Token::VectorEnd,
        ]
    );

    let result4 = tokenize("[ 1 2 ] + [ 3 4 ]").unwrap();
    assert_eq!(
        result4,
        vec![
            Token::VectorStart,
            Token::number("1"),
            Token::number("2"),
            Token::VectorEnd,
            Token::Symbol("+".into()),
            Token::VectorStart,
            Token::number("3"),
            Token::number("4"),
            Token::VectorEnd,
        ]
    );
}

#[test]
fn test_string_with_double_quote() {
    let result = tokenize("'He said \"Hello\"'").unwrap();
    assert_eq!(result, vec![Token::String("He said \"Hello\"".into()),]);
}

#[test]
fn test_string_with_single_quote() {
    let result = tokenize("'It's fine'").unwrap();
    assert_eq!(result, vec![Token::String("It's fine".into()),]);
}

#[test]
fn test_vector_as_code_syntax() {
    let result = tokenize("[ [ 1 ] + ]").unwrap();

    assert_eq!(result.len(), 6);
    assert!(matches!(&result[0], Token::VectorStart));
    assert!(matches!(&result[1], Token::VectorStart));
    assert!(matches!(&result[2], Token::Number(n) if n.lexeme() == "1"));
    assert!(matches!(&result[3], Token::VectorEnd));
    assert!(matches!(&result[4], Token::Symbol(s) if s.as_ref() == "+"));
    assert!(matches!(&result[5], Token::VectorEnd));
}

#[test]
fn test_def_with_vector_code() {
    let result = tokenize("[ [ 2 ] * ] 'DOUBLE' DEF").unwrap();

    assert_eq!(result.len(), 8);
    assert!(matches!(&result[6], Token::String(s) if s.as_ref() == "DOUBLE"));
    assert!(matches!(&result[7], Token::Symbol(s) if s.as_ref() == "DEF"));
}

#[test]
fn test_brace_is_an_ordinary_name_character() {
    // Only `[` and `]` delimit (LANG.SOURCE.TEXT). A brace is a name
    // character like any other punctuation, so it lexes as a Symbol, alone
    // or glued to other characters.
    let result = tokenize("{ [ 2 ] * } a{b}").unwrap();
    assert_eq!(result.first(), Some(&Token::Symbol("{".into())));
    assert_eq!(result[5], Token::Symbol("}".into()));
    assert_eq!(result.last(), Some(&Token::Symbol("a{b}".into())));
}

#[test]
fn test_single_semicolon_lexes_without_a_bespoke_error() {
    let result = tokenize("[ 2 ] * ;").unwrap();
    assert_eq!(result.last(), Some(&Token::Symbol(";".into())));
}

#[test]
fn test_greater_than_tokenizes_as_an_ordinary_symbol() {
    let result = tokenize("5 3 >").unwrap();
    assert_eq!(
        result,
        vec![
            Token::number("5"),
            Token::number("3"),
            Token::Symbol(">".into()),
        ]
    );
}

/// Every symbol is exactly one character, so a two-character spelling is one
/// ordinary name token rather than a comparison. None of these names a Word:
/// the relations are spelled `a b LT NOT`, `a b GT NOT` and `a b EQ NOT`.
#[test]
fn test_two_character_comparisons_are_plain_names() {
    for lexeme in [">=", "<>", "<="] {
        assert_eq!(
            tokenize(lexeme).unwrap(),
            vec![Token::Symbol(lexeme.into())],
            "`{lexeme}` must be one name token"
        );
    }
}

#[test]
fn test_multiline_vector_body_allowed() {
    // A `[ ]` body may span multiple lines; the breaks are whitespace and
    // the body is the same token stream as the one-line spelling.
    let multi = tokenize("[ LENGTH [ 1 ] =\n[ 10 ] ] 'CHECK_ONE' DEF");
    let flat = tokenize("[ LENGTH [ 1 ] = [ 10 ] ] 'CHECK_ONE' DEF");
    assert!(multi.is_ok(), "multi-line vector body should tokenize");
    assert_eq!(multi, flat);
}

#[test]
fn test_unbalanced_bracket_is_still_refused_around_a_paren() {
    // The paren is a name and contributes nothing; the stray `]` is the
    // error, and it must still be reported as one.
    let result = tokenize("( [ 2 ] * ]");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unexpected ']'"));
}

#[test]
fn test_unclosed_bracket_is_still_refused_before_a_paren() {
    let result = tokenize("[ 1 2 3 )");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unclosed"));
}

#[test]
fn test_stray_close_bracket_rejected() {
    let result = tokenize("[ 1 2 3 ] ]");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unexpected ']'"));
}

#[test]
fn test_matched_brackets_ok() {
    let result = tokenize("[ [ 2 ] * ]");
    assert!(result.is_ok());
}

#[test]
fn test_paren_lexes_as_a_name_at_top_level() {
    let result = tokenize("( [ 2 ] * )").unwrap();
    assert_eq!(result.first(), Some(&Token::Symbol("(".into())));
    assert_eq!(result.last(), Some(&Token::Symbol(")".into())));
}

#[test]
fn test_paren_lexes_as_a_name_in_nested_position() {
    let result = tokenize("[ ( [ 1 ] + ) ]").unwrap();
    assert!(result.contains(&Token::Symbol("(".into())));
    assert!(result.contains(&Token::Symbol(")".into())));
}

#[test]
fn test_unclosed_nested_bracket_rejected() {
    let result = tokenize("[ [ 1 ] +");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unclosed"));
}

#[test]
fn test_brackets_in_string_ignored() {
    let result = tokenize("'{ ( [' [ 1 ]");
    assert!(result.is_ok());
}

#[test]
fn test_brackets_in_comment_ignored() {
    let result = tokenize("[ 1 ] # { ( [");
    assert!(result.is_ok());
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(8192))]

    /// `is_symbol_token_lexeme` decides printable-ASCII lexemes without the
    /// tokenizer; on every lexeme it answers what tokenizing does.
    #[test]
    fn symbol_lexeme_shortcut_matches_the_tokenizer(lexeme in "[ -~\u{a0}é]{0,6}") {
        let tokenized = matches!(
            tokenize(&lexeme).ok().as_deref(),
            Some([Token::Symbol(value)]) if value.as_ref() == lexeme
        );
        proptest::prop_assert_eq!(
            crate::tokenizer::is_symbol_token_lexeme(&lexeme),
            tokenized,
            "{:?}",
            lexeme
        );
    }
}

/// The ASCII fast path through a word, its fallback to characters, and the
/// sizing and name sharing around it must not move a token or its span:
/// a vertical tab and a no-break space still separate words, a multi-byte
/// character inside one still counts one column, and a repeated name still
/// reads as the same name.
#[test]
fn spans_survive_the_word_fast_path() {
    let (tokens, spans) =
        crate::tokenizer::tokenize_with_spans("ab\u{0B}1 é漢x\n  X X\na\u{A0}b").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Symbol("ab".into()),
            Token::number("1"),
            Token::Symbol("é漢x".into()),
            Token::Symbol("X".into()),
            Token::Symbol("X".into()),
            Token::Symbol("a".into()),
            Token::Symbol("b".into()),
        ]
    );
    let at = |line, column| crate::tokenizer::SourceSpan { line, column };
    assert_eq!(
        spans,
        vec![
            at(1, 1),
            at(1, 4),
            at(1, 6),
            at(2, 3),
            at(2, 5),
            at(3, 1),
            at(3, 3)
        ]
    );
}
