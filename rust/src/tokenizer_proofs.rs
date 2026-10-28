//! The canonical lexical grammar (`spec/grammar.json`), proved of the
//! tokenizer's two sub-grammars by bounded model checking (AQ-VER-002-J,
//! docs/quality/TRACEABILITY_MATRIX.md).
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take. The grammar's own examples
//! are run by `scripts/check-grammar.mjs` and the MC/DC tables test chosen
//! lexemes; these take every lexeme up to a length, each length a concrete
//! case:
//!
//! - the numeric grammar, every byte string up to [`NUMBER_LEN`] bytes, held
//!   to a finite automaton written from `numericGrammar`'s EBNF;
//! - the string literal's close rule (`closing_quote`), and the cuts
//!   (`literal_cuts`) `value_as_code` relies on to write any text as String
//!   literals, every text up to [`TEXT_LEN`] characters over the four kinds
//!   the rule tells apart: a quote, a space, a line break, and anything else.
//!   Both take positioned characters, as their callers hand them from a
//!   `str`; decoding one is the standard library's.

use super::{closing_quote, is_number_bytes, literal_cuts};

/// The longest lexeme the numeric harness takes.
const NUMBER_LEN: usize = 7;

/// The longest text the string harnesses take.
const TEXT_LEN: usize = 4;

/// The states of the automaton `numericGrammar`'s EBNF reads as:
///
/// ```text
/// number   = [ sign ] , digits , [ rational | [ fraction ] , [ exponent ] ] ;
/// rational = "/" , digits ;  fraction = "." , digits ;
/// exponent = ( "e" | "E" ) , [ sign ] , digits ;
/// ```
#[derive(Clone, Copy, PartialEq)]
enum State {
    Start,
    Sign,
    Integer,
    Slash,
    Denominator,
    Point,
    Fraction,
    E,
    ExponentSign,
    Exponent,
    Reject,
}

fn step(state: State, byte: u8) -> State {
    use State::*;
    let digit = byte.is_ascii_digit();
    let sign = byte == b'+' || byte == b'-';
    match state {
        Start if sign => Sign,
        Start | Sign if digit => Integer,
        Integer if digit => Integer,
        Integer if byte == b'/' => Slash,
        Integer if byte == b'.' => Point,
        Integer | Fraction if byte == b'e' || byte == b'E' => E,
        Slash | Denominator if digit => Denominator,
        Point | Fraction if digit => Fraction,
        E if sign => ExponentSign,
        E | ExponentSign | Exponent if digit => Exponent,
        _ => Reject,
    }
}

fn accepts(bytes: &[u8]) -> bool {
    let end = bytes.iter().fold(State::Start, |state, &b| step(state, b));
    matches!(
        end,
        State::Integer | State::Denominator | State::Fraction | State::Exponent
    )
}

/// The tokenizer's numeric grammar is the specification's, on every byte
/// string up to [`NUMBER_LEN`] bytes — multi-byte characters' bytes included.
#[kani::proof]
#[kani::unwind(9)]
fn number_lexemes_are_the_numeric_grammar() {
    let bytes: [u8; NUMBER_LEN] = kani::any();
    for len in 0..=NUMBER_LEN {
        assert_eq!(is_number_bytes(&bytes[..len]), accepts(&bytes[..len]));
    }
}

/// The characters a string literal's close rule tells apart: a quote, two
/// kinds of whitespace, and anything else.
const ALPHABET: [char; 4] = ['\'', ' ', '\n', 'a'];

/// [`TEXT_LEN`] characters, each any of [`ALPHABET`].
fn any_chars() -> [char; TEXT_LEN] {
    std::array::from_fn(|_| {
        let pick: usize = kani::any();
        kani::assume(pick < ALPHABET.len());
        ALPHABET[pick]
    })
}

/// A literal holds its body up to the first quote that whitespace or the end
/// follows, and is unclosed when no quote closes. The characters are handed
/// to `closing_quote` as `string_literal_content` hands them, positioned, but
/// without a `str` between: decoding one is the standard library's, and the
/// solver does not finish it.
#[kani::proof]
#[kani::unwind(8)]
fn a_string_literal_closes_at_its_first_closing_quote() {
    let body: [char; TEXT_LEN] = any_chars();
    for len in 0..=TEXT_LEN {
        let body = &body[..len];
        let first_close = (0..len).find(|&at| {
            body[at] == '\'' && body.get(at + 1).is_none_or(|next| next.is_whitespace())
        });
        assert_eq!(closing_quote(body.iter().copied().enumerate()), first_close);
    }
}

/// The cuts `string_literal_pieces` makes fall just past every quote that
/// whitespace follows, and nowhere else, so no piece holds a quote that would
/// close its literal early and every piece but the last ends in the quote
/// before a cut.
#[kani::proof]
#[kani::unwind(8)]
fn literal_cuts_fall_after_every_closing_quote() {
    let text: [char; TEXT_LEN] = any_chars();
    for len in 0..=TEXT_LEN {
        let text = &text[..len];
        let mut cuts = literal_cuts(text.iter().copied().enumerate());
        for at in 0..len {
            if text[at] == '\'' && text.get(at + 1).is_some_and(|next| next.is_whitespace()) {
                assert_eq!(cuts.next(), Some(at + 1));
            }
        }
        assert_eq!(cuts.next(), None);
    }
}
