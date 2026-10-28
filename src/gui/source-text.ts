// Ajisai source as text: the lexical sub-grammar (LANG.SOURCE.TEXT) as the GUI
// needs it, and the formatter that tidies source into its canonical form.
//
// The scanner is the one behind the formatter and step mode, mirroring
// rust/src/tokenizer.rs so the two never disagree with each other or with it.
//
// Whitespace is the sole delimiter. At a fresh atom position (after whitespace
// or at the start of the source):
//   - `#` starts a comment that runs to the end of the line;
//   - `'` starts a string, which may hold whitespace and line breaks, and is
//     closed by a `'` that is followed by whitespace or the end of input;
//   - anything else is a word that runs to the next whitespace, full stop —
//     a `#` or `'` inside it is part of the name.
//
// Nothing is repaired or rejected here. A string that never closes runs to the
// end of the source and is marked `closed: false`; a word that holds a bracket
// is returned as written. Each caller decides what to do with those, because
// they differ: the formatter refuses to rewrite an unclosed string, step mode
// hands it to the interpreter to report.

export type AtomKind = 'word' | 'string' | 'comment';

export interface Atom {
    readonly kind: AtomKind;
    /** The atom's text, exactly as written (a comment excludes its line break). */
    readonly text: string;
    /** Offset of the first character, in UTF-16 code units of the source. */
    readonly start: number;
    /** Offset one past the last character. */
    readonly end: number;
    /** For a string: whether a closing quote was found. Always true otherwise. */
    readonly closed: boolean;
}

// The whitespace class of spec/grammar.json (characterClasses.whitespace):
// Unicode White_Space, enumerated. ECMAScript's `\s` is not that class — it
// counts U+FEFF as whitespace, which the grammar makes an ordinary name
// character (a byte-order mark glues to the first word), and leaves out U+0085,
// which the grammar makes whitespace — so the code points are spelled out here
// rather than delegated to the host, as the grammar's own note asks. Every one
// of them is a single UTF-16 code unit, so the scanner's per-unit walk below
// sees each exactly once; a surrogate half is never whitespace.
export const isSourceWhitespace = (character: string): boolean => {
    const code = character.charCodeAt(0);
    return (code >= 0x0009 && code <= 0x000d)
        || code === 0x0020
        || code === 0x0085
        || code === 0x00a0
        || code === 0x1680
        || (code >= 0x2000 && code <= 0x200a)
        || code === 0x2028
        || code === 0x2029
        || code === 0x202f
        || code === 0x205f
        || code === 0x3000;
};

/** How many characters of source whitespace `text` opens with. */
export const countLeadingSourceWhitespace = (text: string): number => {
    let count = 0;
    while (count < text.length && isSourceWhitespace(text[count]!)) count += 1;
    return count;
};

const countTrailingSourceWhitespace = (text: string): number => {
    let count = 0;
    while (count < text.length && isSourceWhitespace(text[text.length - 1 - count]!)) count += 1;
    return count;
};

// `String.prototype.trim` strips by ECMAScript's class, so it takes a leading
// byte-order mark off a program the tokenizer would have read as glued to its
// first word; the GUI and the CLI must run the same program from the same
// text, so source is trimmed by the grammar's class instead.
export const trimSource = (text: string): string => {
    const leading = countLeadingSourceWhitespace(text);
    if (leading === text.length) return '';
    return text.slice(leading, text.length - countTrailingSourceWhitespace(text));
};

export const trimSourceEnd = (text: string): string =>
    text.slice(0, text.length - countTrailingSourceWhitespace(text));

const endOfComment = (source: string, start: number): number => {
    let i = start;
    while (i < source.length && source[i] !== '\n') i += 1;
    return i;
};

/** `[end, closed]` of the string literal opening at `start`. */
const endOfString = (source: string, start: number): [number, boolean] => {
    for (let i = start + 1; i < source.length; i += 1) {
        if (source[i] !== "'") continue;
        const next = source[i + 1];
        if (next === undefined || isSourceWhitespace(next)) return [i + 1, true];
    }
    return [source.length, false];
};

const endOfWord = (source: string, start: number): number => {
    let i = start;
    while (i < source.length && !isSourceWhitespace(source[i]!)) i += 1;
    return i;
};

export const scanAtoms = (source: string): Atom[] => {
    const atoms: Atom[] = [];
    let i = 0;
    while (i < source.length) {
        if (isSourceWhitespace(source[i]!)) {
            i += 1;
            continue;
        }
        const start = i;
        let kind: AtomKind;
        let closed = true;
        if (source[i] === '#') {
            kind = 'comment';
            i = endOfComment(source, start);
        } else if (source[i] === "'") {
            kind = 'string';
            [i, closed] = endOfString(source, start);
        } else {
            kind = 'word';
            i = endOfWord(source, start);
        }
        atoms.push({ kind, text: source.slice(start, i), start, end: i, closed });
    }
    return atoms;
};

// ── Formatter ───────────────────────────────────────────────────────────────
// Tidies input into the canonical written form without changing what the code
// means. A line break is whitespace like any other in Ajisai (LANG.SOURCE.TEXT)
// except that it ends a `#` comment, and the layout is the author's, so the
// line structure is preserved exactly and only the spacing between tokens and
// the indentation at the start of each line are rewritten.
//
// Per line it:
//   - collapses runs of spaces/tabs to a single space;
//   - surrounds the always-standalone delimiters [ ] with spaces, so
//     `[1 2 3]` becomes `[ 1 2 3 ]` and `[[1]]` becomes `[ [ 1 ] ]`;
//   - keeps string literals ('...') and comments (#...) verbatim;
//   - re-indents the line by the bracket nesting depth open at its start.
//
// It never adds or removes a line break. Input it cannot rewrite safely (an
// unterminated string, or a newline inside a string literal) is returned
// unchanged.

const INDENT_UNIT = '  ';

// `[` and `]` are the one delimiter pair of spec/grammar.json, and like every
// other word they must stand alone: the tokenizer (rust/src/tokenizer.rs)
// rejects a bracket glued to adjacent text rather than splitting it off. The
// formatter supplies that whitespace proactively, so `[1 2 3]` becomes valid,
// canonical source instead of a tokenizer error. No other character is split
// out of a word — `^`, `>=`, `#` and `'` inside a name are all part of it — so
// the atoms above are kept whole except for these two. The editor splits the
// same way when it reads the word under the cursor.
export const splitBrackets = (word: string): string[] => word.match(/[[\]]|[^[\]]+/g) ?? [];

// The source as lines of tokens. Returns null when the source cannot be safely
// reformatted (unterminated string, or a newline inside a string literal).
const scanLines = (source: string): string[][] | null => {
    const lines: string[][] = [];
    let line: string[] = [];
    let scanned = 0;

    // Line breaks live in the whitespace between atoms (and inside a string,
    // which is refused below), so the line structure is read off the gaps.
    const advanceTo = (offset: number): void => {
        for (let i = scanned; i < offset; i += 1) {
            if (source[i] === '\n') {
                lines.push(line);
                line = [];
            }
        }
        scanned = offset;
    };

    for (const atom of scanAtoms(source)) {
        advanceTo(atom.start);
        scanned = atom.end;
        switch (atom.kind) {
            case 'string':
                if (!atom.closed || atom.text.includes('\n')) return null;
                line.push(atom.text);
                break;
            case 'comment':
                line.push(trimSourceEnd(atom.text));
                break;
            case 'word':
                line.push(...splitBrackets(atom.text));
                break;
        }
    }
    advanceTo(source.length);
    lines.push(line);
    return lines;
};

const countLeadingClosers = (tokens: string[]): number => {
    let leading = 0;
    while (tokens[leading] === ']') leading += 1;
    return leading;
};

const netBracketDelta = (tokens: string[]): number => {
    let net = 0;
    for (const token of tokens) {
        if (token === '[') net += 1;
        else if (token === ']') net -= 1;
    }
    return net;
};

const renderLines = (lines: string[][]): string => {
    const out: string[] = [];
    let depth = 0;
    let pendingBlank = false;

    for (const tokens of lines) {
        if (tokens.length === 0) {
            // Collapse runs of blank lines and drop leading/trailing ones.
            if (out.length > 0) pendingBlank = true;
            continue;
        }

        if (pendingBlank) {
            out.push('');
            pendingBlank = false;
        }

        const indent = Math.max(0, depth - countLeadingClosers(tokens));
        out.push(INDENT_UNIT.repeat(indent) + tokens.join(' '));
        depth = Math.max(0, depth + netBracketDelta(tokens));
    }

    return out.join('\n');
};

// Format Ajisai source into its canonical written form. Returns the input
// unchanged when it cannot be reformatted without risking a semantic change.
export const formatAjisaiSource = (source: string): string => {
    const lines = scanLines(source);
    if (lines === null) return source;
    return renderLines(lines);
};
