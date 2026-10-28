//! MC/DC tables for the decisions of `crate::tokenizer` past the sign
//! preamble `tokenizer_mcdc_tests` covers (AQ-VER-002-F): the body of the
//! numeric grammar, the structural gate and string boundaries, and the digit
//! count the numeric-literal ceiling reads.
//!
//! Split from `tokenizer_mcdc_tests` rather than appended to it, to keep both
//! under the 500-line budget (docs/dev/specification-implementation-rules.md).
//!
//! Trace: docs/quality/TRACEABILITY_MATRIX.md, requirement AQ-REQ-002.

use crate::tokenizer::{denoted_digit_count, tokenize};
use crate::types::Token;

/// Whether the whole lexeme reads as one Number (otherwise it must read as
/// one Symbol of the same spelling: the grammar never errors on a lexeme).
fn reads_as_number(lexeme: &str) -> bool {
    match tokenize(lexeme).unwrap().as_slice() {
        [Token::Number(_)] => true,
        [Token::Symbol(name)] if name.as_ref() == lexeme => false,
        other => panic!("`{lexeme}` read as {other:?}"),
    }
}

fn assert_numbers(lexemes: &[&str]) {
    for lexeme in lexemes {
        assert!(reads_as_number(lexeme), "`{lexeme}` must be a Number");
    }
}

fn assert_names(lexemes: &[&str]) {
    for lexeme in lexemes {
        assert!(!reads_as_number(lexeme), "`{lexeme}` must be a Symbol");
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-002-G
// DUT: rust/src/tokenizer.rs `parse_number_from_string`, after the sign
//
//     if i >= len || !digit(i) { None }                     // G1 integer part
//     if i < len && chars[i] == '/' {                       // G2 (L, S)
//         if i >= len || !digit(i) { None }                 // G3 (E, N)
//         if i == len { Number } else { None }              // G4
//     }
//     if i < len && chars[i] == '.' {                       // G5 (L, P)
//         if i >= len || !digit(i) { None }                 // G6 (E, N)
//     }
//     if i < len && (chars[i] == 'e' || chars[i] == 'E') {  // G7 (L, e, E)
//         if i < len && (chars[i] == '-' || chars[i] == '+') // G8 (L, m, p)
//         if i >= len || !digit(i) { None }                 // G9 (E, N)
//     }
//     if i == start && !has_dot { None }                    // G10
//     if i == len { Number } else { None }                  // G11
//
// Each row is observed as the token kind of the whole lexeme. A row where
// the decision is false and the scan falls through ends at G11, which
// rejects whatever character stopped it, so a false row usually reads as a
// Symbol and its true partner as a Number.
//
// G1 (E, N): (F, F) "5" Number; (F, T) ".5" Symbol [N]. E = T is
//   unreachable: an empty lexeme returns first and a sign needs a digit after
//   it (AQ-VER-002-F), so `i < len` here.
// G2 (L, S): (T, T) "3/4" Number; (T, F) "3x4" Symbol [S]; (F, *) "3" — the
//   lexeme ends, G11 accepts [L].
// G3 (E, N): (F, F) "3/4"; (T, *) "3/" Symbol [E]; (F, T) "3/x" Symbol [N].
// G4: T "3/4" Number; F "3/4x", "3/4/5", "3/4.5", "3/4e2" Symbol.
// G5 (L, P): (T, T) "2.5"; (T, F) "2x5" Symbol [P]; (F, *) "2" [L].
// G6 (E, N): (F, F) "2.5"; (T, *) "2." Symbol [E]; (F, T) "2.e5" Symbol [N].
// G7 (L, e, E): (T, T, *) "2e5"; (T, F, T) "2E5"; (T, F, F) "2x5" Symbol
//   [e and E each against it]; (F, *, *) "2" [L].
// G8 (L, m, p): (T, T, *) "2e-5"; (T, F, T) "2e+5"; (T, F, F) "2e5"; L = F
//   is the lexeme ending right after `e`, which G9 then rejects.
// G9 (E, N): (F, F) "2e5"; (T, *) "2e", "2e-" Symbol [E]; (F, T) "2ex",
//   "2e+x" Symbol [N].
// G0, `chars.is_empty()`: `tokenize` never hands it an empty lexeme, but
//   the code-data decoder's `is_number_token_lexeme` can.
// G10 has no true row: G1 has already required a digit, so `i > start`.
// G11: T "2.5e-3" Number; F "2.5.5", "2e5e5", "2e5x" Symbol.
// ---------------------------------------------------------------------------
mod number_grammar_body {
    use super::*;

    #[test]
    fn aq_ver_002_g_g1_an_integer_part_is_required() {
        assert_numbers(&["5", "0", "007", "-5"]);
        assert_names(&[".5", "-.5", "+.5", ".", "x5"]);
    }

    #[test]
    fn aq_ver_002_g_g2_to_g4_a_fraction_needs_digits_and_then_the_end() {
        assert_numbers(&["3/4", "-3/4", "+3/4", "3/0", "0/0", "12/345"]);
        assert_names(&["3x4", "3\\4"]);
        assert_names(&["3/", "-3/"]);
        assert_names(&["3/x", "3/-4", "3/+4", "3/.5"]);
        assert_names(&["3/4x", "3/4/5", "3/4.5", "3/4e2"]);
    }

    #[test]
    fn aq_ver_002_g_g5_g6_a_point_needs_digits_after_it() {
        assert_numbers(&["2.5", "-2.5", "0.0", "2.50"]);
        assert_names(&["2x5", "2,5"]);
        assert_names(&["2.", "-2."]);
        assert_names(&["2.e5", "2.x", "2.-5"]);
    }

    #[test]
    fn aq_ver_002_g_g7_to_g9_an_exponent_needs_digits_after_its_sign() {
        assert_numbers(&["2e5", "2E5", "2e-5", "2e+5", "2E-5", "2.5e3", "2e05"]);
        assert_names(&["2x5", "2f5", "2D5"]);
        assert_names(&["2e", "2E", "2e-", "2e+"]);
        assert_names(&["2ex", "2e+x", "2e-x", "2e--5", "2e.5"]);
    }

    #[test]
    fn aq_ver_002_g_g11_the_whole_lexeme_must_be_consumed() {
        assert_numbers(&["2.5e-3", "1/2"]);
        assert_names(&["2.5.5", "2e5e5", "2e5x", "2e5.5", "5x", "1_000", "0x10"]);
    }

    #[test]
    fn aq_ver_002_g_g0_the_empty_lexeme_is_no_number() {
        use crate::tokenizer::is_number_token_lexeme;
        assert!(!is_number_token_lexeme(""));
        assert!(is_number_token_lexeme("2.5e-3"));
        assert!(!is_number_token_lexeme("2.5e"));
    }

    /// A multi-byte character is never a digit, sign, point, `e` or `/`: it
    /// stops a number wherever it stands.
    #[test]
    fn aq_ver_002_g_a_non_ascii_byte_stops_a_number() {
        assert_names(&["5é", "é5", "1/２", "2e５", "１"]);
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-002-H
// DUT: rust/src/tokenizer.rs, the structural gate and the string boundaries
//
//   `delimiter_token` / `token_str.contains(DELIMITERS)`: a lexeme is one
//     delimiter, holds none, or is refused.
//       "[" / "]" -> structure;  "ab" -> name;  "[a", "a]", "a[b" -> error
//   `validate_code_tokens`: `depth.checked_sub(1)` fails (U), and
//     `depth != 0` at the end (O).
//       balanced -> Ok;  U = T "] [" -> error though the counts agree;
//       O = T "[ [ ]" -> error
//   `string_literal_content`, `c == '\'' && peek.is_none_or(whitespace)`
//     (Q, C): (T, T) close; (T, F) a quote inside; (F, *) content.
//       C's None arm — end of input closes — is "'foo'" and "''";
//       no closing quote at all is "'foo" and "'".
//   `string_literal_pieces` (the inverse the source writer uses), `c == '\''
//     && peek.is_some_and(whitespace)` (Q, W) and `!current.is_empty()` (E):
//       (T, T) cut;  (T, F) a final quote stays;  E = F only for "".
//   A quote or `#` opens a string or comment only at a word position.
// ---------------------------------------------------------------------------
mod structure_and_string_boundaries {
    use super::*;

    #[test]
    fn aq_ver_002_h_a_lexeme_is_one_delimiter_or_holds_none() {
        assert_eq!(
            tokenize("[ ab ]").unwrap(),
            vec![
                Token::VectorStart,
                Token::Symbol("ab".into()),
                Token::VectorEnd
            ]
        );
        for source in ["[a ]", "[ a]", "[ a[b ]", "[]", "][", "[["] {
            let err = tokenize(source).unwrap_err();
            assert!(err.contains("must stand alone"), "`{source}`: {err}");
        }
    }

    #[test]
    fn aq_ver_002_h_brackets_must_balance_in_order() {
        assert!(tokenize("[ [ 1 ] [ ] ]").is_ok());
        let stray = tokenize("] [").unwrap_err();
        assert!(stray.contains("Unexpected ']'"), "U = T: {stray}");
        let open = tokenize("[ [ ]").unwrap_err();
        assert!(open.contains("Unclosed '['"), "O = T: {open}");
        // A bracket inside a string or a comment is not structure.
        assert!(tokenize("'[' # ]").is_ok());
    }

    /// The same gate over tokens a program built as code data rather than
    /// wrote as source (`EXEC`, `DEF`): `tokenize` holds it inside its scan,
    /// so this is the gate's own entry, with the same messages.
    #[test]
    fn aq_ver_002_h_code_data_brackets_must_balance_in_order() {
        use crate::tokenizer::validate_code_tokens;
        let (open, close) = (Token::VectorStart, Token::VectorEnd);
        assert!(validate_code_tokens(&[open.clone(), close.clone()]).is_ok());
        let stray = validate_code_tokens(&[close.clone(), open.clone()]).unwrap_err();
        assert!(stray.contains("Unexpected ']'"), "U = T: {stray}");
        let unclosed = validate_code_tokens(&[open.clone(), open, close]).unwrap_err();
        assert!(unclosed.contains("Unclosed '['"), "O = T: {unclosed}");
    }

    /// A position reads as the line and column a reader counts.
    #[test]
    fn aq_ver_002_h_a_span_displays_its_line_and_column() {
        let span = crate::tokenizer::SourceSpan {
            line: 3,
            column: 14,
        };
        assert_eq!(span.to_string(), "line 3, column 14");
    }

    #[test]
    fn aq_ver_002_h_end_of_input_closes_a_string() {
        assert_eq!(
            tokenize("'foo'").unwrap(),
            vec![Token::String("foo".into())]
        );
        assert_eq!(tokenize("''").unwrap(), vec![Token::String("".into())]);
        assert_eq!(
            tokenize("'' x").unwrap(),
            vec![Token::String("".into()), Token::Symbol("x".into())]
        );
        assert_eq!(
            tokenize("'a'b'").unwrap(),
            vec![Token::String("a'b".into())]
        );
    }

    #[test]
    fn aq_ver_002_h_a_string_with_no_closing_quote_is_refused() {
        for source in ["'foo", "'", "x 'foo'bar"] {
            let err = tokenize(source).unwrap_err();
            assert!(err.contains("Unclosed literal"), "`{source}`: {err}");
        }
    }

    #[test]
    fn aq_ver_002_h_string_pieces_cut_after_a_quote_before_whitespace() {
        use crate::tokenizer::string_literal_pieces;
        assert_eq!(string_literal_pieces("a' b"), ["a'", " b"]);
        assert_eq!(string_literal_pieces("a'b"), ["a'b"]);
        assert_eq!(string_literal_pieces("a'"), ["a'"]);
        assert_eq!(string_literal_pieces("x' y' "), ["x'", " y'", " "]);
        assert!(string_literal_pieces("").is_empty());
    }

    #[test]
    fn aq_ver_002_h_quote_and_hash_open_only_at_a_word_position() {
        assert_eq!(
            tokenize("a'b'").unwrap(),
            vec![Token::Symbol("a'b'".into())]
        );
        assert_eq!(
            tokenize("[ C# ]").unwrap(),
            vec![
                Token::VectorStart,
                Token::Symbol("C#".into()),
                Token::VectorEnd
            ]
        );
    }
}

// ---------------------------------------------------------------------------
// AQ-VER-002-I
// DUT: rust/src/tokenizer.rs `denoted_digit_count`, which the numeric-literal
// ceiling reads (LANG.MACHINE.LIMITS)
//
//     if mantissa.chars().all(|c| !c.is_ascii_digit() || c == '0') {   // Z
//         return written;
//     }
//     let scale = exponent.map_or(0, |e| e.trim_start_matches(['+', '-'])
//         .parse::<u64>().unwrap_or(u64::MAX));                          // X, P
//     written.saturating_add(scale)
//
// Z: T "0e99" -> 1 (zero is built at no scale); F "1e99" -> 100   pair shows Z
//   Z's per-character condition (D = digit, O = '0'): a sign or point
//   (D = F) keeps Z true, a '0' digit (D = T, O = T) keeps it, any other
//   digit (D = T, O = F) makes it false.
// X (an exponent written): F "123" -> 3; T "1e5" -> 6.
// P (the exponent parses as u64): F -> u64::MAX, saturating; T as written.
// ---------------------------------------------------------------------------
mod denoted_digits {
    use super::*;

    #[test]
    fn aq_ver_002_i_a_zero_mantissa_counts_only_its_written_digits() {
        assert_eq!(denoted_digit_count("0e99"), 1);
        assert_eq!(denoted_digit_count("-0.000e99999999999999999999"), 4);
        assert_eq!(denoted_digit_count("1e99"), 100);
        assert_eq!(denoted_digit_count("0.001e99"), 103);
        assert_eq!(denoted_digit_count("10e5"), 7);
    }

    #[test]
    fn aq_ver_002_i_the_exponent_adds_its_magnitude() {
        assert_eq!(denoted_digit_count("123"), 3);
        assert_eq!(denoted_digit_count("-12/34"), 4);
        assert_eq!(denoted_digit_count("1e5"), 6);
        assert_eq!(denoted_digit_count("1E+5"), 6);
        assert_eq!(denoted_digit_count("1e-5"), 6);
    }

    #[test]
    fn aq_ver_002_i_an_exponent_past_u64_saturates() {
        assert_eq!(denoted_digit_count("1e99999999999999999999"), u64::MAX);
        assert_eq!(denoted_digit_count("12e18446744073709551615"), u64::MAX);
    }
}
