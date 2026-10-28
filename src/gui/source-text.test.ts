// The GUI's one scanner of Ajisai source, checked against the rules of
// rust/src/tokenizer.rs: whitespace is the sole delimiter, and `#` and `'` are
// special only at the start of an atom.

import { describe, expect, test } from 'vitest';
import {
    countLeadingSourceWhitespace,
    isSourceWhitespace,
    scanAtoms,
    trimSource,
    trimSourceEnd,
    formatAjisaiSource
} from './source-text';
import corpus from '../../tests/formatter-corpus.json';

const texts = (source: string): string[] => scanAtoms(source).map((a) => a.text);

describe('scanAtoms', () => {
    test('splits words on whitespace of any kind', () => {
        expect(texts('1 \t2\n\nADD')).toEqual(['1', '2', 'ADD']);
    });

    test('a word runs to whitespace and nothing else splits it', () => {
        expect(texts('[1 2]')).toEqual(['[1', '2]']);
        expect(texts('a^b >= x')).toEqual(['a^b', '>=', 'x']);
    });

    test('a hash at the start of an atom is a comment to end of line', () => {
        const atoms = scanAtoms('1 # two 3\n4');
        expect(atoms.map((a) => [a.kind, a.text])).toEqual([
            ['word', '1'],
            ['comment', '# two 3'],
            ['word', '4']
        ]);
    });

    test('a hash inside a word is part of the word', () => {
        expect(scanAtoms('a#b 1').map((a) => [a.kind, a.text])).toEqual([
            ['word', 'a#b'],
            ['word', '1']
        ]);
    });

    test('a string holds whitespace and closes at a quote before whitespace', () => {
        expect(texts("'hello world' PRINT")).toEqual(["'hello world'", 'PRINT']);
        expect(texts("'it''s' x")).toEqual(["'it''s'", 'x']);
    });

    test('a quote followed by anything but whitespace does not close', () => {
        const [atom] = scanAtoms("'foo'[1] 2");
        expect(atom).toMatchObject({ kind: 'string', text: "'foo'[1] 2", closed: false });
    });

    test('a quote inside a word is part of the word', () => {
        expect(scanAtoms("a'b c").map((a) => [a.kind, a.text])).toEqual([
            ['word', "a'b"],
            ['word', 'c']
        ]);
    });

    test('an unclosed string runs to the end and says so', () => {
        const atoms = scanAtoms("1 'oops");
        expect(atoms[1]).toEqual({ kind: 'string', text: "'oops", start: 2, end: 7, closed: false });
    });

    test('every atom is exactly the source it points at', () => {
        const source = "[ 1 ] # c\n'a b' x#y";
        for (const atom of scanAtoms(source)) {
            expect(source.slice(atom.start, atom.end)).toBe(atom.text);
        }
    });
});

// spec/grammar.json, characterClasses.whitespace: Unicode White_Space,
// enumerated because host predicates disagree at the edges — ECMAScript's `\s`
// counts U+FEFF as whitespace and U+0085 as not, and the grammar says the
// opposite of both.
describe('isSourceWhitespace', () => {
    test('is the grammar\'s enumerated class', () => {
        for (const ws of ['\t', '\n', '\v', '\f', '\r', ' ', '\u0085', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', '　']) {
            expect(isSourceWhitespace(ws)).toBe(true);
        }
        for (const name of ['﻿', 'a', '1', '[', '​', '、']) {
            expect(isSourceWhitespace(name)).toBe(false);
        }
    });

    test('a byte-order mark glues to the first word, as the grammar\'s note says', () => {
        expect(texts('﻿1 2 ADD')).toEqual(['﻿1', '2', 'ADD']);
    });

    test('U+0085 separates words and closes a string, as any whitespace does', () => {
        expect(texts('1\u00852')).toEqual(['1', '2']);
        expect(texts("'a'\u0085b")).toEqual(["'a'", 'b']);
    });
});

describe('trimSource', () => {
    test('strips the grammar\'s whitespace from both ends', () => {
        expect(trimSource(' \n　 1 2 ADD\u0085\t')).toBe('1 2 ADD');
        expect(trimSource(' \n ')).toBe('');
        expect(trimSource('')).toBe('');
    });

    test('keeps a byte-order mark, which is part of the first word', () => {
        expect(trimSource(' ﻿1 2 ADD ')).toBe('﻿1 2 ADD');
        expect(countLeadingSourceWhitespace(' ﻿1')).toBe(1);
    });

    test('trimSourceEnd strips the end only', () => {
        expect(trimSourceEnd('  # a comment \r')).toBe('  # a comment');
    });
});

// ── merged from code-formatter.test.ts ──

// Tests for the Ajisai source formatter. The formatter must tidy spacing and
// indentation while preserving meaning: line breaks (statement separators
// inside blocks) and the contents of strings and comments are never altered.


// tests/formatter-corpus.json pins this formatter's input->expected pairs.
// There is currently no second implementation reading it (no Rust-side
// formatter exists in this repo), so it is not itself a cross-implementation
// drift check; treat it as a regression corpus for this module only, and keep
// its cases honest against the real tokenizer (rust/src/tokenizer.rs) by hand.
describe('formatAjisaiSource shared corpus', () => {
    for (const c of corpus.cases) {
        test(`corpus: ${c.name}`, () => {
            expect(formatAjisaiSource(c.input)).toBe(c.expected);
        });
    }
});

describe('formatAjisaiSource', () => {
    test('returns empty string unchanged', () => {
        expect(formatAjisaiSource('')).toBe('');
    });

    test('pads the inside of vector brackets', () => {
        expect(formatAjisaiSource('[1 2 3]')).toBe('[ 1 2 3 ]');
    });

    test('separates nested brackets into standalone tokens', () => {
        expect(formatAjisaiSource('[[1 2][3 4]]')).toBe('[ [ 1 2 ] [ 3 4 ] ]');
    });

    test('collapses runs of spaces between tokens', () => {
        expect(formatAjisaiSource('[ 1    2   3 ]')).toBe('[ 1 2 3 ]');
    });

    test('trims leading and trailing whitespace on a line', () => {
        expect(formatAjisaiSource('   [ 1 ] PRINT   ')).toBe('[ 1 ] PRINT');
    });

    test('splits brackets that are glued to adjacent words', () => {
        expect(formatAjisaiSource('[1 2 3]PRINT')).toBe('[ 1 2 3 ] PRINT');
    });

    test('does not split a glued caret or bar out of a word', () => {
        // `^` and `|` are ordinary word characters in the lexer (they end a
        // token only at whitespace or a bracket, same as `~`), so forcing
        // spaces around one glued to a name would turn one Symbol token into
        // three — exactly the meaning change this formatter must not make.
        expect(formatAjisaiSource('a^c')).toBe('a^c');
        expect(formatAjisaiSource('a|c')).toBe('a|c');
        expect(formatAjisaiSource('a~b')).toBe('a~b');
    });

    test('keeps a hash or quote glued to a word as part of the word', () => {
        // Only at the start of an atom do `#` and `'` start a comment or a
        // string (rust/src/tokenizer.rs); inside a word they are name
        // characters, and splitting them out would change the program.
        expect(formatAjisaiSource('a#b 1')).toBe('a#b 1');
        expect(formatAjisaiSource("a'b c")).toBe("a'b c");
    });

    test('still pads an already-standalone bar between spaced tokens', () => {
        // Written with its own whitespace, `|` already scans as its own
        // token; the formatter just normalizes the spacing around it, same as
        // any other token-to-token gap.
        expect(formatAjisaiSource('[ a   |   b ]')).toBe('[ a | b ]');
    });

    test('leaves a string not closed before a following > or = untouched', () => {
        // `>` and `=` do not end a token in the real tokenizer, so a `'` right
        // before one does not close the string either; the real tokenizer
        // reports an unclosed literal for this input, and the formatter
        // refuses to reformat what it cannot safely rewrite.
        expect(formatAjisaiSource("'foo'>bar")).toBe("'foo'>bar");
        expect(formatAjisaiSource("'foo'=bar")).toBe("'foo'=bar");
    });

    test('leaves a string glued to a following bracket untouched', () => {
        // Whitespace is the sole token delimiter, so `[` does not close a
        // string that runs right into it — the real tokenizer finds no real
        // close and reports an unclosed literal, and the formatter must
        // refuse to reformat rather than confidently splitting off `[1]`.
        expect(formatAjisaiSource("'foo'[1]")).toBe("'foo'[1]");
    });

    test('is idempotent on already-canonical input', () => {
        const canonical = '[ [ 1 ] [ 2 ] ADD ] \'ADD12\' DEF';
        expect(formatAjisaiSource(canonical)).toBe(canonical);
    });

    test('keeps the contents of a string literal verbatim', () => {
        expect(formatAjisaiSource("[ 'a  b   c' ]")).toBe("[ 'a  b   c' ]");
    });

    test('does not pad brackets that live inside a string', () => {
        expect(formatAjisaiSource("'[not code]'")).toBe("'[not code]'");
    });

    test('keeps comment text (including spacing) verbatim', () => {
        expect(formatAjisaiSource('[ 1 ]   #   keep   spacing'))
            .toBe('[ 1 ] #   keep   spacing');
    });

    test('preserves a comment-only line', () => {
        expect(formatAjisaiSource('# ===== header ====='))
            .toBe('# ===== header =====');
    });

    test('preserves significant line breaks between statements', () => {
        const input = '[1]PRINT\n[2]PRINT';
        expect(formatAjisaiSource(input)).toBe('[ 1 ] PRINT\n[ 2 ] PRINT');
    });

    test('indents the body of a multi-line block and dedents its close', () => {
        const input = [
            '[',
            "[ 'big' ]   [ 'small' ]",
            'N [ 5 ]  GT',
            'SELECT',
            "] 'SIZE' DEF",
        ].join('\n');
        const expected = [
            '[',
            "  [ 'big' ] [ 'small' ]",
            '  N [ 5 ] GT',
            '  SELECT',
            "] 'SIZE' DEF",
        ].join('\n');
        expect(formatAjisaiSource(input)).toBe(expected);
    });

    test('indents nested multi-line blocks by depth', () => {
        const input = '[\n[\n[ 1 ]\n]\n]';
        const expected = '[\n  [\n    [ 1 ]\n  ]\n]';
        expect(formatAjisaiSource(input)).toBe(expected);
    });

    test('collapses multiple blank lines and trims surrounding blanks', () => {
        const input = '\n\n[ 1 ]\n\n\n[ 2 ]\n\n';
        expect(formatAjisaiSource(input)).toBe('[ 1 ]\n\n[ 2 ]');
    });

    test('leaves an unterminated string untouched', () => {
        const input = "[ 'oops ]";
        expect(formatAjisaiSource(input)).toBe(input);
    });

    test('leaves a newline inside a string untouched', () => {
        const input = "'line one\nline two'";
        expect(formatAjisaiSource(input)).toBe(input);
    });

    test('does not expand the ; modifier sugar', () => {
        expect(formatAjisaiSource('[ 1 ] ;')).toBe('[ 1 ] ;');
    });

    test('keeps a two-character comparison spelling intact', () => {
        // `>` is not split, so `>=` survives as one token rather than becoming
        // `> =` (GT followed by EQ).
        expect(formatAjisaiSource('[ 1 ] [ 3 ] >=')).toBe('[ 1 ] [ 3 ] >=');
        expect(formatAjisaiSource('[ 1 ] [ 3 ] <>')).toBe('[ 1 ] [ 3 ] <>');
    });

    test('formatting is idempotent on a multi-line block', () => {
        const messy = "[\n[ [ 5 ]   GT ]\n] 'SIZE' DEF";
        const once = formatAjisaiSource(messy);
        expect(formatAjisaiSource(once)).toBe(once);
    });
});

// The formatter adds no line break of its own: line structure is purely the
// author's, which is what makes the line-break rule of LANG.SOURCE.TEXT safe
// to leave alone.
describe('formatAjisaiSource line structure', () => {
    test('a branch written on one line stays on one line', () => {
        const source = "[ 'big' ] [ 'small' ] [ 5 ] [ 3 ] GT SELECT";
        expect(formatAjisaiSource(source)).toBe(source);
    });

    test('a branch written across lines keeps every break', () => {
        const source = "[ 'big' ]\n[ 'small' ]\n[ 5 ] [ 3 ] GT\nSELECT";
        expect(formatAjisaiSource(source)).toBe(source);
    });

    test('an ordinary vector is not rearranged', () => {
        expect(formatAjisaiSource('[ 1 2 ] [ 1 MUL ] MAP [ 2 MUL ] MAP'))
            .toBe('[ 1 2 ] [ 1 MUL ] MAP [ 2 MUL ] MAP');
    });
});


// The formatter runs before every Run in the GUI, so where it disagrees with
// the tokenizer about what whitespace is, the GUI runs a different program
// from the CLI. spec/grammar.json enumerates the class: U+FEFF is a name
// character, U+0085 is whitespace; ECMAScript's `\s` says the opposite.
describe('formatAjisaiSource whitespace class', () => {
    test('keeps a byte-order mark glued to the first word, as the tokenizer reads it', () => {
        expect(formatAjisaiSource('﻿1 2 ADD')).toBe('﻿1 2 ADD');
    });

    test('separates words on U+0085 like any other whitespace', () => {
        expect(formatAjisaiSource('1\u00852 ADD')).toBe('1 2 ADD');
    });
});
