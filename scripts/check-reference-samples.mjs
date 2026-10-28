// LANG.AUTHORITY.PRESENT — the worked-example half of the clause.
//
// The Reference (public/docs/en/index.html and its Japanese version,
// public/docs/ja/index.html) shows 80-odd sample blocks, each a program next to
// the value it is said to produce, each with an "Open in Playground" button that
// loads it ready to run. A reader learns the language by running
// them, so an example that does not produce its stated value is not a cosmetic
// defect: the diagnostic is precise, the Reference is confident, and the reader
// concludes they mistyped it.
//
// Two such examples shipped. The `COND` clause-form and paired-form examples
// both dispatched on `[ 0 ]` / `[ -5 ]` and compared against `[ 0 ]`, and
// comparison Words lift element-wise, so each guard answered `[ FALSE ]` /
// `[ TRUE ]` — a one-element Vector, not the bare Boolean a guard must return
// (LANG.VALUES.TRUTH, LANG.VALUES.DISJOINT). Both raised `nonTruthGuard`
// instead of the `'zero'` / `'negative'` printed beside them. They were the
// language's two introductory examples of conditional dispatch.
//
// Every other reading surface that carries runnable code is already gated —
// SKILL.md against its own corpus, the MCP evaluation corpus against the CLI —
// and the Reference was the one that was not. This gate runs each sample
// through the same `ajisai` binary a reader's Playground runs and compares the
// result against the expectation printed beside it, so the Reference cannot
// drift from the language again without CI saying so.
//
// Both language versions are run: they share their sample code by design, but
// each is a page a reader runs, so each is checked rather than assumed equal.
//
// The three expectation notations are distinguished by their own markup,
// not by guessing at the text:
//
//   <td><code>VALUE</code></td>   under Expected value / 期待値          → final stack
//   <td><code>TEXT</code></td>    under Expected output / 期待される出力  → standard output
//   <td>error</td>                (bare, no <code>)            → any error
//   <td>—</td>                    (bare em dash)                → empty stack
//
// A row that is none of these is reported rather than skipped: a sample with no
// checkable expectation is exactly the hole this gate exists to close.

import { readFileSync, writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { resolveAjisaiBin } from './lib/common.mjs';

const SURFACES = [
  { path: 'public/docs/en/index.html', stackHeader: 'Expected value', outputHeader: 'Expected output' },
  { path: 'public/docs/ja/index.html', stackHeader: '期待値', outputHeader: '期待される出力' },
];
const BINARY = resolveAjisaiBin('reference-samples');

const ANY_ERROR = 'error';
const EMPTY_STACK = '—';

// `<br>` is a line break in the sample source, and a line break ends a `#`
// comment, so it has to become a newline rather than be stripped with the rest
// of the markup.
const cellText = (cell) =>
  cell
    .replace(/<br\s*\/?>/g, '\n')
    .replace(/<[^>]+>/g, '')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, '&');

// Whitespace is insignificant between the printed form and the CLI's, and only
// there: this normalization is for comparing two renderings of one value, never
// for deciding what a program means. Inside a string literal whitespace is
// content (`'rate not quoted'` is not `'ratenotquoted'`), so a quoted run is
// kept verbatim; it closes on a quote followed by whitespace, a closing
// bracket, or the end, as the literal grammar closes it (spec/grammar.json
// stringLiteral), with the bracket allowed because a rendering may glue it.
const normalizeStack = (text) => {
  let out = '';
  let i = 0;
  while (i < text.length) {
    const ch = text[i];
    if (ch === "'") {
      let end = i + 1;
      while (end < text.length && !(text[end] === "'" && (end + 1 === text.length || /[\s\]}]/.test(text[end + 1])))) end += 1;
      out += text.slice(i, end + 1);
      i = end + 1;
    } else {
      if (!/\s/.test(ch)) out += ch;
      i += 1;
    }
  }
  return out;
};
// Printed output is text, so its spaces are content too; only line-end and
// edge whitespace, which the table cell cannot show, is ignored.
const normalizeOutput = (text) =>
  text
    .split('\n')
    .map((line) => line.trimEnd())
    .join('\n')
    .trim();

const samples = [];
const errors = [];

const blockPattern = /<div class="sample">([\s\S]*?)<div class="sample-actions">/g;
const rowPattern =
  /<tr>\s*<td><code>([\s\S]*?)<\/code><\/td>\s*<td>([\s\S]*?)<\/td>\s*<td class="notes">[\s\S]*?<\/td>\s*<\/tr>/g;

for (const { path, stackHeader, outputHeader } of SURFACES) {
  const html = readFileSync(path, 'utf8');
  const before = samples.length;
  for (const block of html.matchAll(blockPattern)) {
    const body = block[1];
    const line = html.slice(0, block.index).split('\n').length;
    const header = /<thead>\s*<tr>([\s\S]*?)<\/tr>/.exec(body);
    const headings = header ? [...header[1].matchAll(/<th>([\s\S]*?)<\/th>/g)].map((m) => cellText(m[1]).trim()) : [];
    const checksOutput = headings.includes(outputHeader);
    if (!checksOutput && !headings.includes(stackHeader)) {
      errors.push(`${path}:${line}: sample block heads its expectation column "${headings[1] ?? '(none)'}", which is neither "${stackHeader}" nor "${outputHeader}"`);
      continue;
    }
    for (const row of body.matchAll(rowPattern)) {
      const rawExpectation = row[2].trim();
      const expectation = cellText(rawExpectation).trim();
      // A `<code>`-wrapped expectation is a value; a bare one is a notation.
      const quoted = /^<code>[\s\S]*<\/code>$/.test(rawExpectation);
      let kind;
      if (quoted) kind = checksOutput ? 'output' : 'stack';
      else if (expectation === ANY_ERROR) kind = 'error';
      else if (expectation === EMPTY_STACK) kind = 'empty';
      else {
        errors.push(`${path}:${line}: sample expects "${expectation}", which is not a value, "${ANY_ERROR}" or "${EMPTY_STACK}"`);
        continue;
      }
      samples.push({ path, line, code: cellText(row[1]), expectation, kind });
    }
  }
  if (samples.length === before) errors.push(`${path}: found no samples — the extraction pattern no longer matches the page.`);
}

// The language versions differ in prose only: the same samples, in the same
// order, with the same expectations. A sample changed in one version and not
// the other is reported here, before anything runs.
const programsOf = (path) => samples.filter((sample) => sample.path === path).map((sample) => `${sample.code} ⇒ ${sample.expectation}`);
const [reference, ...translations] = SURFACES.map(({ path }) => ({ path, programs: programsOf(path) }));
for (const translation of translations) {
  const length = Math.max(reference.programs.length, translation.programs.length);
  for (let i = 0; i < length; i += 1) {
    if (reference.programs[i] === translation.programs[i]) continue;
    errors.push(`sample ${i + 1} differs between the language versions:\n  ${reference.path}: ${reference.programs[i] ?? '(none)'}\n  ${translation.path}: ${translation.programs[i] ?? '(none)'}`);
    break;
  }
}

if (errors.length) {
  for (const error of errors) console.error(`[reference-samples] ${error}`);
  process.exit(1);
}

const workdir = mkdtempSync(join(tmpdir(), 'ajisai-reference-samples-'));
const program = join(workdir, 'sample.ajisai');

try {
  for (const sample of samples) {
    writeFileSync(program, `${sample.code}\n`, 'utf8');
    const run = spawnSync(BINARY, ['run', program], { encoding: 'utf8', timeout: 60_000 });
    if (run.error) {
      console.error(`[reference-samples] could not run ${BINARY}: ${run.error.message}`);
      console.error('[reference-samples] Build it first: cargo build --manifest-path rust/Cargo.toml --bin ajisai');
      process.exit(1);
    }
    const combined = `${run.stdout ?? ''}${run.stderr ?? ''}`;
    const lines = combined.split('\n');
    const stackLine = lines.find((l) => l.startsWith('stack:'));
    const stack = stackLine ? stackLine.slice('stack:'.length).trim() : null;
    // Everything the program printed, which is every line before the stack
    // report the CLI appends.
    const output = lines.slice(0, stackLine ? lines.indexOf(stackLine) : lines.length).join('\n').trim();
    const failed = stack === null;
    // The CLI exits 0 with a stack report, or 1 with an `error:` report. Any
    // other status, or a signal, is a crash: its text must not pass for the
    // error a sample expects, nor for a value.
    const crashed =
      run.signal !== null ||
      (failed ? run.status !== 1 || !lines.some((l) => l.startsWith('error:')) : run.status !== 0);
    const at = `${sample.path}:${sample.line}: ${sample.code.replace(/\n/g, ' ⏎ ')}`;

    if (crashed) {
      errors.push(`${at}\n  the CLI did not exit cleanly (status ${run.status}, signal ${run.signal}): ${lines[0]}`);
      continue;
    }
    if (sample.kind === 'error') {
      if (!failed) errors.push(`${at}\n  expected an error, got stack ${stack}`);
      continue;
    }
    if (failed) {
      errors.push(`${at}\n  expected ${sample.kind === 'output' ? 'output' : 'stack'} ${sample.expectation}, got ${lines[0]}`);
      continue;
    }
    if (sample.kind === 'empty') {
      if (stack !== '(empty)') errors.push(`${at}\n  expected an empty stack, got ${stack}`);
      continue;
    }
    const actual = sample.kind === 'output' ? output : stack;
    const normalize = sample.kind === 'output' ? normalizeOutput : normalizeStack;
    if (normalize(actual) !== normalize(sample.expectation)) {
      errors.push(`${at}\n  expected ${sample.kind} ${sample.expectation}, got ${actual}`);
    }
  }
} finally {
  rmSync(workdir, { recursive: true, force: true });
}

if (errors.length) {
  for (const error of errors) console.error(`[reference-samples] ${error}`);
  console.error(`[reference-samples] ${errors.length} of ${samples.length} Reference samples do not produce the value printed beside them.`);
  console.error('[reference-samples] A sample carries an "Open in Playground" button, so a reader runs it: fix the sample, or fix the expectation.');
  process.exitCode = 1;
} else {
  console.log(`[reference-samples] all ${samples.length} samples in ${SURFACES.map((s) => s.path).join(' and ')} produce the value printed beside them.`);
}
