//! AR-VER-230: where each top-level item is written (AR-LLR-031), which the
//! Playground steps through: one span per item, a quote spanning its
//! brackets, a quantized literal its width, exponent and QUANTIZE, a text
//! token its quotes, columns counted in characters, comments skipped.

use arare_front::{parse, parse_spanned, Span};

fn spans(src: &str) -> Vec<(usize, usize, usize, usize)> {
    let (program, spans) = parse_spanned(src).expect(src);
    assert_eq!(program, parse(src).unwrap(), "{src}: the same program");
    assert_eq!(spans.len(), program.items.len(), "{src}: one span per item");
    spans
        .into_iter()
        .map(|s| (s.line, s.column, s.end_line, s.end_column))
        .collect()
}

#[test]
fn ar_ver_230_one_span_per_item() {
    assert_eq!(
        spans("12 ADD"),
        [(1, 1, 1, 3), (1, 4, 1, 7)],
        "an integer and a Word"
    );
    assert_eq!(
        spans("0.75 8 -2 QUANTIZE 3 MUL"),
        [(1, 1, 1, 19), (1, 20, 1, 21), (1, 22, 1, 25)],
        "a quantized literal is one item"
    );
    assert_eq!(
        spans("[ 1 [ 2 ] ] [ 2 MUL ] MAP"),
        [(1, 1, 1, 12), (1, 13, 1, 22), (1, 23, 1, 26)],
        "a quote spans its brackets, nested quotes inside"
    );
    assert_eq!(
        spans("5 'N' BIND N"),
        [(1, 1, 1, 2), (1, 3, 1, 6), (1, 7, 1, 11), (1, 12, 1, 13)],
        "a text token spans its quotes"
    );
}

#[test]
fn ar_ver_230_lines_comments_and_characters() {
    assert_eq!(
        spans("[ 1\n  2 ] # a vector\n# nothing here\n\tSUM"),
        [(1, 1, 2, 6), (4, 2, 4, 5)],
        "a quote across lines; comments are not items"
    );
    assert_eq!(
        spans("'αβ' 'γ' BIND αβ"),
        [(1, 1, 1, 5), (1, 6, 1, 9), (1, 10, 1, 14), (1, 15, 1, 17)],
        "columns count characters, not bytes"
    );
    assert_eq!(spans(""), [], "an empty program has no items");
    assert_eq!(
        parse_spanned("1 ]").unwrap_err(),
        parse("1 ]").unwrap_err(),
        "a source the parser refuses is refused the same way"
    );
    let s = Span {
        line: 1,
        column: 1,
        end_line: 1,
        end_column: 2,
    };
    assert_eq!(parse_spanned("7").unwrap().1, [s]);
}
