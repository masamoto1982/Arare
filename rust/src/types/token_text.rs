//! The text a name or number token carries, and the numeric literal built on
//! it.
//!
//! Every token of a program is a copy of the one its lexeme first read as
//! (`tokenizer_scan::LexemeMemo`), so the copy is the cost of reading a
//! token. Holding the text as an `Arc<str>` made each copy an atomic
//! increment of a count every other copy shares, and each drop an atomic
//! decrement of it: measured on the program's token walk, those two were a
//! third of the time a plain top-level word took. A name is short — nearly
//! every Word, binding and number fits in a few bytes — so [`Name`] keeps
//! that much inline, where copying it is copying its bytes, and shares an
//! `Arc<str>` only for the rare longer one.
//!
//! A `Name` is its text: it compares, orders, hashes and prints exactly as
//! that `str` does, so nothing that reads a token can tell which way it is
//! held.

use super::fraction::Fraction;
use std::sync::Arc;

/// How many bytes of text a [`Name`] holds without an allocation: what fills
/// the space an `Arc<str>` and a discriminant take anyway.
const INLINE: usize = 22;

/// A name's or a number's text (see the module documentation).
pub struct Name(Repr);

enum Repr {
    /// `len` bytes of text at the front of `bytes`; the rest are zero, so two
    /// inline names are equal exactly when their `len` and `bytes` are.
    Inline {
        len: u8,
        bytes: [u8; INLINE],
    },
    Shared(Arc<str>),
}

impl Name {
    pub fn new(text: &str) -> Self {
        match Self::inline(text) {
            Some(name) => name,
            None => Name(Repr::Shared(Arc::from(text))),
        }
    }

    #[inline]
    fn inline(text: &str) -> Option<Self> {
        if text.len() > INLINE {
            return None;
        }
        let mut bytes = [0; INLINE];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        Some(Name(Repr::Inline {
            len: text.len() as u8,
            bytes,
        }))
    }

    /// The text's bytes, read without re-checking that they are UTF-8.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        match &self.0 {
            Repr::Inline { len, bytes } => &bytes[..usize::from(*len)],
            Repr::Shared(text) => text.as_bytes(),
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        match &self.0 {
            // Built from a `str`, so this cannot fail; the check is the price
            // of reading inline bytes as text without `unsafe`, and it is a
            // few instructions for a short ASCII name.
            Repr::Inline { len, bytes } => std::str::from_utf8(&bytes[..usize::from(*len)])
                .expect("a Name holds the text it was built from"),
            Repr::Shared(text) => text,
        }
    }

    /// The text as an `Arc<str>`, sharing it when it is held as one.
    pub fn to_arc(&self) -> Arc<str> {
        match &self.0 {
            Repr::Inline { .. } => Arc::from(self.as_str()),
            Repr::Shared(text) => Arc::clone(text),
        }
    }
}

impl Clone for Name {
    #[inline(always)]
    fn clone(&self) -> Self {
        Name(match &self.0 {
            Repr::Inline { len, bytes } => Repr::Inline {
                len: *len,
                bytes: *bytes,
            },
            Repr::Shared(text) => Repr::Shared(Arc::clone(text)),
        })
    }
}

impl std::ops::Deref for Name {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for Name {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Name {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for Name {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (
                Repr::Inline { len, bytes },
                Repr::Inline {
                    len: other_len,
                    bytes: other_bytes,
                },
            ) => len == other_len && words(bytes) == words(other_bytes),
            _ => self.as_bytes() == other.as_bytes(),
        }
    }
}

impl Eq for Name {}

/// Inline text as three machine words (the last two overlap), so that two
/// names compare in three compares rather than byte by byte.
#[inline]
fn words(bytes: &[u8; INLINE]) -> [u64; 3] {
    let word = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"));
    [word(0), word(8), word(INLINE - 8)]
}

impl PartialEq<str> for Name {
    fn eq(&self, other: &str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl PartialEq<&str> for Name {
    fn eq(&self, other: &&str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

/// Hashes as `str` does — its bytes, then the `0xff` that ends a string —
/// without reading the bytes back as text first: a binding table keyed by
/// `Name` is probed by `str` too, so the two must hash alike.
impl std::hash::Hash for Name {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write(self.as_bytes());
        state.write_u8(0xff);
    }
}

impl std::fmt::Debug for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_str(), f)
    }
}

impl From<&str> for Name {
    fn from(text: &str) -> Self {
        Name::new(text)
    }
}

impl From<String> for Name {
    fn from(text: String) -> Self {
        Name::new(&text)
    }
}

impl From<&String> for Name {
    fn from(text: &String) -> Self {
        Name::new(text)
    }
}

impl From<Arc<str>> for Name {
    fn from(text: Arc<str>) -> Self {
        Self::inline(&text).unwrap_or(Name(Repr::Shared(text)))
    }
}

impl From<&Arc<str>> for Name {
    fn from(text: &Arc<str>) -> Self {
        Self::inline(text).unwrap_or_else(|| Name(Repr::Shared(Arc::clone(text))))
    }
}

/// A numeric literal: how it was written, and — when it is a plain machine
/// integer — the integer it denotes.
///
/// The parse belongs to the *source*, not to the execution. It used to happen
/// wherever a `Token::Number` was reached, which meant a literal inside a block
/// was re-parsed once per element: `[ 2 MUL ] MAP` rebuilt a `BigInt` from `"2"`
/// and gcd-normalized it on every lane. `value_as_code` made that worse by
/// formatting an already-parsed `Scalar` back into a string for the token, so a
/// block literal made a full round trip — parsed value → string → parsed value,
/// once per element.
///
/// **Only `i64` is carried, and that is a measurement rather than a shortcut.**
/// Carrying the full `Fraction` worked for blocks and lost elsewhere: a literal is
/// met *once* per straight-line program, so hoisting its parse to the lexer saves
/// nothing there, while a 64-byte `Fraction` in every token took `Token` from 24
/// bytes to 80 and pushing a thousand literals from 238 ns each to 293. An `i64`
/// costs a digit scan rather than a `BigInt` allocation to fill, and covers the
/// literals that actually appear in hot blocks. A rational, an exponent, a
/// decimal point or a digit separator simply has no `integer` and is parsed
/// where it is reached, as before.
///
/// **The lexeme is kept, not discarded.** Three consumers need the spelling
/// rather than the value: the numeric-literal digit ceiling counts digits as
/// written, `format_token_to_string` echoes source back, and the tokenizer's own
/// round-trip check compares a lexeme against itself.
///
/// **A malformed lexeme is not an error here.** This type validates nothing: it
/// keeps whatever spelling it is handed. A `NumberLiteral` built directly from
/// a lexeme that is not a number simply has no `integer`, and [`Self::parsed`]
/// reports the parse's own message when the value is read.
#[derive(Debug, PartialEq, Hash)]
pub struct NumberLiteral {
    lexeme: Name,
    /// Set only when the lexeme is exactly `[+-]?digits` in `i64` range, in which
    /// case it denotes precisely this integer over 1. Every other spelling —
    /// `1/2`, `1.5`, `1e5`, `1_000`, `1/0` (one of the two points over zero),
    /// or anything wider than `i64` — leaves
    /// it `None` and reaches `Fraction::from_str` as it did before.
    integer: Option<i64>,
}

impl Clone for NumberLiteral {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            lexeme: self.lexeme.clone(),
            integer: self.integer,
        }
    }
}

impl NumberLiteral {
    /// From a lexeme the lexer has just recognized. The `i64` attempt is a digit
    /// scan with no allocation; it declines rather than approximating.
    pub(crate) fn from_lexeme(lexeme: &str) -> Self {
        let integer = lexeme.parse::<i64>().ok();
        Self {
            lexeme: Name::new(lexeme),
            integer,
        }
    }

    /// From a rational the caller already holds. `value_as_code` uses this when
    /// bridging a `Scalar` element back into a token, so an integer it already
    /// had is never re-parsed from the string it was formatted into.
    pub(crate) fn from_value(value: Fraction) -> Self {
        Self::from_lexeme(&format!("{value}"))
    }

    /// The literal as written.
    pub fn lexeme(&self) -> &str {
        &self.lexeme
    }

    /// How many digits the number this denotes can take to write out
    /// (`tokenizer::denoted_digit_count`). An `i64` lexeme is a sign and
    /// digits, so that is its length less the sign, with no scan.
    pub(crate) fn denoted_digits(&self) -> u64 {
        match self.integer {
            Some(_) => {
                let bytes = self.lexeme.as_bytes();
                let signed = matches!(bytes.first(), Some(b'-' | b'+'));
                (bytes.len() - usize::from(signed)) as u64
            }
            None => crate::tokenizer::denoted_digit_count(&self.lexeme),
        }
    }

    /// The rational this denotes, or the message its own parse reports.
    ///
    /// The integer case is the whole point: it builds a `Fraction` from an `i64`
    /// with no scan, no `BigInt` and no allocation. Everything else parses here,
    /// exactly where it used to.
    #[inline]
    pub(crate) fn parsed(&self) -> std::result::Result<Fraction, String> {
        match self.integer {
            Some(n) => Ok(Fraction::from(n)),
            None => Fraction::from_str(&self.lexeme),
        }
    }

    /// [`Self::parsed`] as an `Option`, for the callers that decline rather than
    /// report — contract inference and the word identity's token encoding.
    pub(crate) fn value(&self) -> Option<Fraction> {
        self.parsed().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::{Name, INLINE};
    use std::hash::{BuildHasher, BuildHasherDefault};

    fn texts() -> Vec<String> {
        vec![
            String::new(),
            "X".into(),
            "ADD".into(),
            "é漢x".into(),
            "a".repeat(INLINE),
            "b".repeat(INLINE + 1),
            "LONG-NAME-THAT-IS-NOT-INLINE".into(),
        ]
    }

    /// A `Name` is its text: equal, hashed, shown and read back exactly as
    /// the `str` it was built from, whether it is held inline or shared.
    #[test]
    fn a_name_is_its_text() {
        let std_hasher = std::collections::hash_map::RandomState::new();
        let fx_hasher = BuildHasherDefault::<crate::fast_hash::FxHasher>::default();
        for text in texts() {
            let name = Name::from(text.as_str());
            let shared = Name::from(std::sync::Arc::<str>::from(text.as_str()));
            assert_eq!(name.as_str(), text);
            assert_eq!(name.as_bytes(), text.as_bytes());
            assert_eq!(name, shared);
            assert_eq!(name, *text.as_str());
            assert_eq!(format!("{name:?}"), format!("{text:?}"));
            assert_eq!(format!("{name}"), text);
            assert_eq!(&*name.to_arc(), text.as_str());
            assert_eq!(
                std_hasher.hash_one(&name),
                std_hasher.hash_one(text.as_str())
            );
            assert_eq!(fx_hasher.hash_one(&name), fx_hasher.hash_one(text.as_str()));
        }
        let all = texts();
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                let (left, right) = (Name::from(a.as_str()), Name::from(b.as_str()));
                assert_eq!(left == right, i == j);
            }
        }
    }
}
