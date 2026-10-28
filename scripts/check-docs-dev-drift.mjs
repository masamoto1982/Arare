#!/usr/bin/env node
// docs/dev/ design memos accumulate faster than anyone revisits them. A memo
// tagged `[設計根拠]` or `[方針記録]` in docs/dev/INDEX.md claims to be a
// design document the current implementation depends on, or the record of an
// adopted decision (INDEX.md's own tag definitions) — read as load-bearing,
// not as a point-in-time snapshot. Nothing previously verified that a symbol
// such a memo names by its qualified Rust path (`Type::Variant`) or as a
// callable (`snake_case_fn(`) still exists in the implementation it claims to
// describe.
//
// This is exactly the failure a since-removed docs/dev/ audit note found by
// hand: eight Rust-side comments asserted a `ValueData::Unknown` variant and
// an `is_unknown()` predicate that had never landed, or had landed and been
// reverted, leaving only the prose behind (fixed in the PR that added this
// gate). This check runs the same question the other
// direction — docs/dev/ prose making claims about rust/src, rust/tests, and
// src — the way spec/words.schema.json's fields are already checked against
// the implementation (scripts/check-unreachable-contract.mjs).
//
// This is a name-reachability heuristic, not a semantic proof (the same
// caveat check-unreachable-contract.mjs states): a literal-substring match
// cannot tell a present-tense claim ("U is a `ValueData::Unknown` variant")
// from a future-tense one ("add `cost_label(...)` to contract_report.rs" — a
// work-order instruction, not a claim about current code), or from an
// accidental substring collision (a test named
// `...agree_on_what_is_unknown` makes the literal text `is_unknown(` appear
// in rust/tests/ for a reason unrelated to any `is_unknown()` predicate, which
// is why that specific drift needed the zero-based reading's human pass
// rather than this gate). Confirmed heuristic false positives are named in
// KNOWN_FALSE_POSITIVES below, each with the reason it is not drift.
//
// Scope is `[設計根拠]` and `[方針記録]` memos only — the two tags INDEX.md
// itself defines as load-bearing ("現行実装が依拠する", "採用済みの設計判断").
// `[観察ノート]` (a point-in-time descriptive analysis, explicitly not
// policy-setting per INDEX.md) is excluded on purpose: its entire genre is
// naming things that turned out not to exist, as evidence — the zero-based
// reading note itself names `ValueData::Unknown` for exactly that reason, and
// checking it would fail the gate on the note that motivated this gate.
// `[執筆規約]` (writing conventions) is excluded because it does not describe
// implementation internals.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { repoRoot, reporter, skipBuildAndHidden, walk } from './lib/common.mjs';

const docsDevDir = resolve(repoRoot, 'docs/dev');
// Generated sources are projections of spec/, not implementation a memo can
// make claims about.
const skip = (name) => skipBuildAndHidden(name) || name === 'generated';

// The corpus a docs/dev claim must be findable in to count as still true: the
// Rust interpreter and its own test suite, and the TypeScript GUI/runtime.
// Matches check-unreachable-contract.mjs's haystack, plus rust/tests — a
// memo may describe a symbol that only ever lived in the test support code
// (e.g. tests/test_support/generators.rs), not just rust/src.
const haystackFiles = [
  ...walk(resolve(repoRoot, 'rust/src'), ['.rs'], skip),
  ...walk(resolve(repoRoot, 'rust/tests'), ['.rs'], skip),
  ...walk(resolve(repoRoot, 'src'), ['.ts', '.tsx'], skip),
];
const haystack = haystackFiles.map((f) => readFileSync(f, 'utf8')).join('\n');

// A qualified enum-variant/type path (`ValueData::Unknown`): both sides
// capitalized, the shape a construction or match arm writes literally when
// the variant is real. `Type::lower_case` (a method or field reference like
// `Value::hint`) is deliberately excluded — Rust code almost never spells a
// method call with its owning type as a literal prefix, so that shape is
// documentation shorthand rather than a claim checkable by literal match,
// and including it produced only false positives when this gate was
// calibrated.
const TYPE_VARIANT_RE = /\b([A-Z][A-Za-z0-9]*::[A-Z][A-Za-z0-9]*)\b/g;

// A snake_case callable (`is_unknown(`). Requires an underscore so
// mathematical-notation spans like `base(w)` / `obs(a b)` — single short
// words, common in the formalization memo's inline formulas — do not match.
const FUNC_RE = /\b([a-z][a-z0-9]*(?:_[a-z0-9]+)+)\s*\(/g;

// Confirmed heuristic false positives, each keyed `file.md::identifier` (so a
// different memo naming the same identifier is still checked) and commented
// with why the finding is not drift. Every entry the gate was calibrated with
// named a memo since retired, so the list is empty; the next confirmed false
// positive goes here, with its reason.
const KNOWN_FALSE_POSITIVES = new Set([]);

function parseIndexScope() {
  const indexText = readFileSync(resolve(docsDevDir, 'INDEX.md'), 'utf8');
  const rowRe = /\|\s*`([^`]+\.md)`\s*\|.*?\|\s*`(\[[^\]]+\])`\s*\|/g;
  const scoped = [];
  for (const match of indexText.matchAll(rowRe)) {
    const [, file, tag] = match;
    if (tag === '[設計根拠]' || tag === '[方針記録]') scoped.push(file);
  }
  return scoped;
}

const report = reporter('docs-dev-drift');
const fail = report.fail;

const scopedFiles = parseIndexScope();
if (scopedFiles.length === 0) {
  fail('docs/dev/INDEX.md yielded no `[設計根拠]`/`[方針記録]` rows — the table format probably changed under this gate\'s parser');
}

let checkedIdentifiers = 0;
for (const file of scopedFiles) {
  const path = resolve(docsDevDir, file);
  const text = readFileSync(path, 'utf8');
  // Strip fenced code blocks first: a ```rust snippet reproducing a past or
  // hypothetical struct/enum shape is a worked illustration, not a claim
  // that every field in it is independently reachable today — the same
  // reason check-unreachable-contract.mjs does not treat spec/ itself as a
  // consumer. Only single-backtick inline spans are checked.
  const prose = text.replace(/```[\s\S]*?```/g, '');
  const spans = [...prose.matchAll(/`([^`\n]+)`/g)].map((m) => m[1]);

  const found = new Set();
  for (const span of spans) {
    // Rust and TypeScript identifiers are ASCII; a non-ASCII span is
    // mathematical notation (`⟦μ·w⟧ = κ_consume(...)`), not code.
    if (!/^[\x00-\x7F]*$/.test(span)) continue;
    for (const m of span.matchAll(TYPE_VARIANT_RE)) found.add(m[1]);
    for (const m of span.matchAll(FUNC_RE)) found.add(m[1]);
  }

  for (const identifier of found) {
    checkedIdentifiers += 1;
    const needle = identifier.includes('::') ? identifier : `${identifier}(`;
    if (haystack.includes(needle)) continue;
    if (KNOWN_FALSE_POSITIVES.has(`${file}::${identifier}`)) continue;
    fail(
      `docs/dev/${file} references \`${identifier}\`, which does not appear in rust/src, rust/tests, or src — the memo is tagged load-bearing but this claim about the implementation may be stale`,
    );
  }
}

// The same question asked of the specification: a load-bearing memo that
// cites a clause by its stable ID (`LANG.VALUES.EXACT`) is claiming that
// clause exists. Clauses are renamed and merged as the language changes; a
// citation to one that is gone sends a reader to nothing.
const clauseIds = new Set(readFileSync(resolve(repoRoot, 'spec/language-semantics.md'), 'utf8').match(/LANG\.[A-Z]+\.[A-Z]+/g));
let checkedClauses = 0;
for (const file of scopedFiles) {
  const text = readFileSync(resolve(docsDevDir, file), 'utf8');
  for (const clause of new Set(text.match(/LANG\.[A-Z]+\.[A-Z]+/g) ?? [])) {
    checkedClauses += 1;
    if (!clauseIds.has(clause)) {
      fail(`docs/dev/${file} cites ${clause}, which spec/language-semantics.md does not define`);
    }
  }
}

report.done(
  `${scopedFiles.length} load-bearing memo(s), ${checkedIdentifiers} referenced identifier(s) and ${checkedClauses} cited clause(s), all reachable.`,
);
