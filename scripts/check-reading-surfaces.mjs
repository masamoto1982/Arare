// LANG.AUTHORITY.PRESENT — the vocabulary half of the clause.
//
// The README, the Reference, and the Specification describe the language as it
// currently is, and "every Word it names must exist in the vocabulary
// registry". This gate reads the registry rather than a hand-kept blocklist, so
// a Word deleted from spec/words.json can never be left behind in prose: the
// check fails the moment a reading surface names something the language does
// not have.
//
// It found `<code>ALGO</code>` ("SORT is owned by the ALGO module") surviving
// three manual review passes of the Reference, which is the case it exists for.

import { readJson, readText, reporter } from './lib/common.mjs';

const manifest = readJson('docs/word-manifest.json');

const known = new Set();
for (const entry of manifest.entries) {
  known.add(entry.canonical);
  // The manifest field is `surface` (singular). Reading a plural that no entry
  // has meant the registered *symbols* were never in this set — harmless while
  // only word-shaped tokens were checked, and wrong the moment they are not.
  known.add(entry.surface);
  for (const surface of entry.surfaces ?? []) known.add(surface);
}

// Names the documents introduce themselves: user-defined words and dictionaries
// from worked examples, and string contents that happen to be upper-case. These
// are the documents' own inventions, not vocabulary claims, so each is listed
// deliberately. A new example word is a one-line addition here.
const EXAMPLE_NAMES = new Set([
  // user words defined in the Reference's own examples
  'ADD10', 'GREET', 'APPLY-GAIN', 'SAY-HELLO', 'SAY-WORLD', 'SAY-BANG',
  // dictionaries and qualified paths from the resolution examples
  'EXAMPLE', 'AUDIOLIB', 'DICT@WORD', 'EXAMPLE@ADD10', 'EXAMPLE@GREET', 'AUDIOLIB@GREET',
  // literal string contents shown on the stack
  'TEST', "T'ES'T", 'AB', 'CD',
  // placeholder names, introduced by the Vector clause to show that `[ FOO ]`
  // holds the text FOO whether or not FOO is a defined Word
  'FOO', 'BAR',
  // a deliberate typo, introduced by the Defining Words clause to show that a
  // body's names resolve when the word is called rather than when it is defined
  'TOTALL',
  // a self-referential definition, introduced to show DEF refuses it
  'REC',
  // user words defined by the block-frame and two-argument examples
  'ADDW', 'PSUM', 'PDIFF', 'PMAX',
  // the relaxation loop of the Iterative Numerics clause
  'DELTA', 'STEP',
  // names bound with BIND so a worked example can read a value twice
  'N', 'V', 'R', 'X',
  // the subject the recovery phrase names once and reads twice:
  // `subject 'S' BIND fallback S S NIL? SELECT`
  'S',
  // the block-running example of "A block cannot run itself": a Word that
  // runs the block bound to F, which is its operand
  'APPLY', 'F',
  // identity and doubling words of the worked examples
  'ID', 'DOUBLE',
  // the doubling word of the MAP and FOLD examples. It only became visible to
  // this check when those examples stopped spelling their code operand as the
  // String 'DBL' — a String is no longer a code operand (LANG.SOURCE.CODE,
  // spec/termination.json), so the example now writes the Symbol the Word
  // actually needs, and a Symbol is what this check reads.
  'DBL',
  // binding names from the Naming a Value clause. These are locals, not Words:
  // the clause exists to say a binding is not a dictionary entry, so the
  // registry is correctly the wrong place to look for them.
  'W', 'B', 'A', 'T', 'LIMIT', 'DX', 'XS', 'K', 'I', 'YS',
  // binding names from the pattern clause, likewise locals rather than Words
  'P', 'M', 'D',
  'N',
  // the reflection clause's worked example: TWICE is defined and asked about
  // by DIGEST and CONTRACT; NOPE is written precisely because it is
  // never defined, to show CONTRACT projecting notFound for it.
  'TWICE', 'NOPE',
]);

// Names the *host* allocates rather than the language: shortcut-only GUI
// operations (Reset, Stack clear, Editor clear, Lookup) with no typed
// spelling at all — not even as text a program could type and have the host
// intercept. The presentation profile still has to be able to name them in
// prose, and they are correctly absent from the vocabulary registry, which is
// the whole point being made about them: a word of the same name is an
// ordinary word, not a reserved one.
const HOST_COMMANDS = new Set(['RESET', 'STACK-CLEAR', 'EDITOR-CLEAR', 'LOOKUP']);

const SURFACES = ['README.md', 'public/docs/index.html', 'public/docs/en/index.html', 'public/docs/ja/index.html', 'SPECIFICATION.html'];

// Word-shaped: upper-case initial, then the characters an Ajisai name may use.
// Anything else inside <code> is a literal, a fragment, or punctuation.
const WORD_SHAPED = /^[A-Z][A-Z0-9@?!'-]*$/;

// Punctuation-shaped: a token of nothing but punctuation is a *symbol* claim.
// This half of the check did not exist, and the whole retired set — `,,`, `<=`,
// `>=`, `<>` and `&` — sat in the in-app Reference presenting itself as usable
// sugar while the gate reported success, because a symbol is not word-shaped.
// Every Ajisai symbol is ASCII punctuation, so the class is restricted to
// ASCII punctuation rather than "anything that is not a Latin letter or
// digit": a translated surface (public/docs/ja/) sets prose like <code>分子/分母</code>
// beside real tokens, and CJK characters are prose, not a symbol claim, the
// same way the Latin word "numerator" beside them is prose rather than a
// Word claim.
const SYMBOL_SHAPED = /^[!-/:-@[-`{-~]+$/;

// Characters a document names in order to say the language does *not* allocate
// them, plus prose placeholders inside shape sketches. Each is a deliberate
// mention rather than a claim, so each is listed rather than inferred.
const UNALLOCATED_MENTIONS = new Set([
  // the decimal-literal clause names the point to say a lone `.` is a name
  '.',
  // `{ ... }` sketches a block's shape; the ellipsis stands for a body
  '...',
  // `+ - * / = < >` were once second spellings of ADD SUB MUL DIV EQ LT GT
  // and are ordinary names now. The reference names `+`, `<` and a bare `/`
  // to say exactly that (its lexeme section), and writes `-` in notation that
  // is not a program (the remainder formula `a - floor(a/b)·b`).
  '+', '-', '<', '/',
  // named once by the recovery clause to say it is retired: `OR-NIL` was the
  // lazy NIL-coalescing directive, replaced by the `BIND` `NIL?` `SELECT` phrase. A
  // reader meeting it in older material needs the clause to say so.
  'OR-NIL',
  // named once beside it for the same reason: `COND` was the branch `SELECT`
  // replaced.
  'COND',
  // the brackets Ajisai gives no role to. They are ordinary name characters,
  // so the registry does not know them; the reference names them to say the
  // language allocates them nothing, and `{ }` also appears as the Record's
  // display, which is not source.
  '(', ')', '{', '}',
  // the predicate naming convention: HAS?'s entry in the Reference's Word list
  // says a predicate "ends in `?`". That names a character of a name, not a
  // Word, the way `.` above does.
  '?',
]);

const report = reporter('reading-surfaces');

for (const path of SURFACES) {
  const source = readText(path);
  const seen = new Map();

  // A backticked markdown link label is a repository path, not a program
  // token — [`LICENSE`](LICENSE) names a file. Drop those spans first.
  const prose = source.replace(/\[`[^`]+`\]\([^)]*\)/g, '');

  // Markdown backticks and HTML <code> mark a token as belonging to the
  // program, so both are vocabulary claims.
  const spans = [
    ...[...prose.matchAll(/<code>([^<]+)<\/code>/g)].map((m) => m[1]),
    ...[...prose.matchAll(/`([^`\n]+)`/g)].map((m) => m[1]),
  ];

  for (const span of spans) {
    // A span may hold a whole program (`2 SQRT 2 LT`), so check every token.
    // Whitespace is only one of the boundaries the lexer has: a structural
    // delimiter is always a token of its own, so `][` is `]` then `[` rather
    // than one unknown symbol, and `[FOO]` names FOO.
    const tokens = span
      .trim()
      .split(/\s+/)
      .flatMap((part) => part.split(/([[\]{}()])/))
      .filter(Boolean);
    for (const raw of tokens) {
      // Markdown escapes a table-cell pipe as `\|`; the token is the pipe.
      const token = raw.replace(/\\([|`*_])/g, '$1');
      if (WORD_SHAPED.test(token)) {
        if (known.has(token) || EXAMPLE_NAMES.has(token)) continue;
        if (HOST_COMMANDS.has(token)) continue;
        // A name may also be written only to say the language does not have it.
        if (UNALLOCATED_MENTIONS.has(token)) continue;
        seen.set(token, (seen.get(token) ?? 0) + 1);
        continue;
      }
      if (!SYMBOL_SHAPED.test(token)) continue;
      // A token opening with a quote is a string literal, and its content is
      // not vocabulary — `''` is the empty string, not a symbol.
      if (token.startsWith("'")) continue;
      if (known.has(token) || UNALLOCATED_MENTIONS.has(token)) continue;
      if (HOST_COMMANDS.has(token)) continue;
      seen.set(token, (seen.get(token) ?? 0) + 1);
    }
  }

  for (const [token, count] of [...seen].sort()) {
    report.fail(`${path} names ${token} (${count}x), which is not in the vocabulary registry`);
  }
}

if (report.failed) {
  console.error('[reading-surfaces] LANG.AUTHORITY.PRESENT: a reading surface may name only Words the language has.');
  console.error('[reading-surfaces] If the name is a new worked example, add it to EXAMPLE_NAMES in this script.');
  console.error('[reading-surfaces] If a name is written only to say the language does not allocate it, add it to UNALLOCATED_MENTIONS.');
}
report.done(
  `${SURFACES.length} surfaces name only the ${manifest.entries.length} registered surfaces, ${EXAMPLE_NAMES.size} declared example names, ${HOST_COMMANDS.size} host commands and ${UNALLOCATED_MENTIONS.size} declared unallocated mentions.`,
);
