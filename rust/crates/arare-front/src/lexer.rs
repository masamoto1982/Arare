//! The lexical grammar (AR-LLR-011).
//!
//! Whitespace separates tokens; `[` and `]` are tokens on their own, so they
//! also separate. `#` at the start of a token begins a comment that runs to
//! the end of the line. A token that matches a numeric form is a literal:
//!
//! ```text
//! integer  = "-"? digit+
//! decimal  = "-"? digit+ "." digit+
//! fraction = "-"? digit+ "/" digit+        (a zero denominator is a wheel point)
//! ```
//!
//! Any other token is a Word name.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Open,
    Close,
    Integer,
    Decimal,
    Fraction,
    Name,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub line: usize,
    pub column: usize,
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn classify(text: &str) -> TokenKind {
    let body = text.strip_prefix('-').unwrap_or(text);
    if digits(body) {
        TokenKind::Integer
    } else if body
        .split_once('.')
        .is_some_and(|(a, b)| digits(a) && digits(b))
    {
        TokenKind::Decimal
    } else if body
        .split_once('/')
        .is_some_and(|(a, b)| digits(a) && digits(b))
    {
        TokenKind::Fraction
    } else {
        TokenKind::Name
    }
}

/// The tokens of `source`, in order, with 1-based line and column.
pub fn lex(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    for (l, line) in source.lines().enumerate() {
        let mut chars = line.char_indices().peekable();
        while let Some(&(start, c)) = chars.peek() {
            let column = line[..start].chars().count() + 1;
            if c.is_whitespace() {
                chars.next();
            } else if c == '#' {
                break;
            } else if c == '[' || c == ']' {
                chars.next();
                let kind = if c == '[' {
                    TokenKind::Open
                } else {
                    TokenKind::Close
                };
                tokens.push(Token {
                    kind,
                    text: c.to_string(),
                    line: l + 1,
                    column,
                });
            } else {
                let mut end = start;
                while let Some(&(i, d)) = chars.peek() {
                    if d.is_whitespace() || d == '[' || d == ']' {
                        break;
                    }
                    end = i + d.len_utf8();
                    chars.next();
                }
                let text = &line[start..end];
                tokens.push(Token {
                    kind: classify(text),
                    text: text.to_string(),
                    line: l + 1,
                    column,
                });
            }
        }
    }
    tokens
}
