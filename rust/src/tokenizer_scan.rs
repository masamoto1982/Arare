//! The reading machinery behind `tokenizer::tokenize_with_spans`: the cursor
//! that walks the source keeping its line and column, and the memo that
//! turns a repeated lexeme into a copy of its first token.
//!
//! Split out of `tokenizer` when that file outgrew the file-size budget in
//! docs/dev/specification-implementation-rules.md. What a lexeme *means* —
//! the number grammar, the delimiter rule, the string sub-grammar — stays
//! there; this is only how the text is walked.

use crate::tokenizer::SourceSpan;

/// Where each lexeme already read first became a token, so that a repeat is
/// a copy of that token rather than a second reading.
///
/// A lexeme of up to seven bytes — nearly every Word, binding and number a
/// program writes — packs into one machine word together with its length
/// (its *key*), and a small direct-mapped table answers it with one compare:
/// hashing the text and comparing it byte by byte against a map's key was a
/// measurable share of reading every token. A longer lexeme, or one whose
/// slot another holds, is looked up by its text.
pub(crate) struct LexemeMemo<'a> {
    recent: [(u64, usize); 64],
    all: crate::fast_hash::FastMap<&'a str, usize>,
}

impl Default for LexemeMemo<'_> {
    fn default() -> Self {
        Self {
            recent: [(0, 0); 64],
            all: Default::default(),
        }
    }
}

impl<'a> LexemeMemo<'a> {
    /// The key of `lexeme`: its bytes packed little-endian with its length in
    /// the top byte, or 0 for a lexeme too long to pack. A packed key is
    /// never 0, since its length is not.
    pub(crate) const fn key(lexeme: &[u8]) -> u64 {
        if lexeme.len() > 7 {
            return 0;
        }
        let mut packed = (lexeme.len() as u64) << 56;
        let mut at = 0;
        while at < lexeme.len() {
            packed |= (lexeme[at] as u64) << (8 * at);
            at += 1;
        }
        packed
    }

    #[inline]
    fn slot(key: u64) -> usize {
        (key.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 58) as usize
    }

    /// The index of the token the lexeme packed as `key` first read as, when
    /// the table holds it.
    #[inline]
    pub(crate) fn recent(&self, key: u64) -> Option<usize> {
        let (held, first) = self.recent[Self::slot(key)];
        (key != 0 && held == key).then_some(first)
    }

    /// The index of the token `lexeme` (packed as `key`) first read as.
    pub(crate) fn find(&mut self, key: u64, lexeme: &'a str) -> Option<usize> {
        let first = *self.all.get(lexeme)?;
        if key != 0 {
            self.recent[Self::slot(key)] = (key, first);
        }
        Some(first)
    }

    pub(crate) fn remember(&mut self, key: u64, lexeme: &'a str, first: usize) {
        if key != 0 {
            self.recent[Self::slot(key)] = (key, first);
        }
        self.all.insert(lexeme, first);
    }
}

/// About how many tokens `input` reads as: the runs of bytes above space.
/// Only a capacity — a string or comment holding spaces makes it more, a
/// control character makes it more, other whitespace makes it fewer, and
/// none of that changes what is read — so it is counted eight bytes at a
/// time: the high bit of each byte of `word` below is set when that byte is
/// above space (the low seven bits plus 0x5F carry into it exactly then, and
/// a byte with it already set is not ASCII and so not a space), and a run
/// starts at each such byte whose predecessor's bit is clear.
pub(crate) fn ascii_word_count(input: &str) -> usize {
    const LOW: u64 = 0x7F7F_7F7F_7F7F_7F7F;
    const ABOVE_SPACE: u64 = 0x5F5F_5F5F_5F5F_5F5F;
    const HIGH: u64 = 0x8080_8080_8080_8080;
    let (chunks, rest) = input.as_bytes().as_chunks::<8>();
    let mut count = 0;
    // The high bit of the byte before the next one, in byte 0's high bit.
    let mut carried = 0u64;
    for chunk in chunks {
        let bytes = u64::from_le_bytes(*chunk);
        let word = (((bytes & LOW) + ABOVE_SPACE) | bytes) & HIGH;
        count += (word & !((word << 8) | carried)).count_ones() as usize;
        carried = word >> 56;
    }
    let mut in_word = carried != 0;
    for &byte in rest {
        let above = byte > b' ';
        count += usize::from(above && !in_word);
        in_word = above;
    }
    count
}

/// A position in the source being tokenized, advanced one character at a
/// time so that its line and column stay those of the next character.
pub(crate) struct Cursor<'a> {
    pub(crate) input: &'a str,
    pub(crate) pos: usize,
    pub(crate) here: SourceSpan,
}

impl Cursor<'_> {
    #[inline]
    pub(crate) fn peek(&self) -> Option<char> {
        // Source is nearly all ASCII, which is its own character; decoding
        // UTF-8 from a fresh slice per character was most of this loop.
        match *self.input.as_bytes().get(self.pos)? {
            byte if byte.is_ascii() => Some(char::from(byte)),
            _ => self.input[self.pos..].chars().next(),
        }
    }

    /// Advance past the word at the cursor, to the next whitespace character
    /// or the end of input, and answer its key (`LexemeMemo::key`). A word
    /// holds no newline, so only the column moves. Its printable ASCII bytes
    /// — nearly all a program writes — are taken a byte at a time and packed
    /// on the way; anything else is walked by `peek`/`bump` and the word
    /// packed afterwards.
    #[inline]
    pub(crate) fn skip_word(&mut self) -> u64 {
        let bytes = self.input.as_bytes();
        let start = self.pos;
        let mut packed = 0u64;
        let mut end = start;
        // `!` to DEL: ASCII, and not whitespace.
        while let Some(&byte) = bytes.get(end) {
            if byte.wrapping_sub(b'!') > 0x7F - b'!' {
                break;
            }
            if end - start < 7 {
                packed |= u64::from(byte) << (8 * (end - start));
            }
            end += 1;
        }
        self.here.column += (end - start) as u32;
        self.pos = end;
        if !matches!(
            bytes.get(end),
            None | Some(b' ' | b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r')
        ) {
            self.skip_word_rest();
            return LexemeMemo::key(&bytes[start..self.pos]);
        }
        let length = end - start;
        if length > 7 {
            0
        } else {
            packed | ((length as u64) << 56)
        }
    }

    /// The rest of a word that holds a control character or a non-ASCII one.
    fn skip_word_rest(&mut self) {
        let bytes = self.input.as_bytes();
        while let Some(&byte) = bytes.get(self.pos) {
            if byte.is_ascii() {
                // `char::is_whitespace` on ASCII, vertical tab included.
                if matches!(byte, b' ' | b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r') {
                    return;
                }
                self.pos += 1;
                self.here.column += 1;
            } else {
                match self.peek() {
                    Some(c) if !c.is_whitespace() => self.bump(),
                    _ => return,
                }
            }
        }
    }

    /// Advance to byte offset `end`, a character boundary: a column per
    /// character and a fresh line after each newline, as `bump` moves. A
    /// UTF-8 continuation byte is the one kind that begins no character.
    #[inline]
    pub(crate) fn advance_to(&mut self, end: usize) {
        for &byte in &self.input.as_bytes()[self.pos..end] {
            if byte == b'\n' {
                self.here.line += 1;
                self.here.column = 1;
            } else if byte & 0xC0 != 0x80 {
                self.here.column += 1;
            }
        }
        self.pos = end;
    }

    #[inline]
    pub(crate) fn bump(&mut self) {
        if let Some(c) = self.peek() {
            self.pos += c.len_utf8();
            if c == '\n' {
                self.here.line += 1;
                self.here.column = 1;
            } else {
                self.here.column += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ascii_word_count, LexemeMemo};

    /// The eight-at-a-time count is the byte-at-a-time one, across chunk
    /// edges, runs that straddle them, non-ASCII bytes and control bytes.
    #[test]
    fn word_count_counts_runs_above_space() {
        let naive = |text: &str| {
            let mut count = 0;
            let mut in_word = false;
            for &byte in text.as_bytes() {
                count += usize::from(byte > b' ' && !in_word);
                in_word = byte > b' ';
            }
            count
        };
        let cases = [
            "",
            " ",
            "a",
            "1 2 ADD 3 MUL ",
            "abcdefgh ijklmnop",
            "abcdefghijklmnopq r",
            "       x       y        ",
            "\t\n\x0B\x0C\r a\x01b é漢x\n  X X\na\u{A0}b \x7F",
            "[ 'X' BIND X X MUL 1 ADD ] 'SQ1' DEF 5 SQ1 3 SQ1 ADD",
        ];
        for text in cases {
            assert_eq!(ascii_word_count(text), naive(text), "{text:?}");
        }
    }

    /// A key packs a lexeme of up to seven bytes and declines a longer one.
    #[test]
    fn keys_tell_short_lexemes_apart() {
        assert_eq!(LexemeMemo::key(b"ABCDEFGH"), 0);
        assert_ne!(LexemeMemo::key(b"A"), LexemeMemo::key(b"A\0"));
        assert_ne!(LexemeMemo::key(b"ABCDEFG"), LexemeMemo::key(b"ABCDEF"));
        assert_ne!(LexemeMemo::key(b"["), 0);
    }
}
