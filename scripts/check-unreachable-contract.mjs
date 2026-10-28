#!/usr/bin/env node
// Every classification field spec/words.schema.json declares must be reachable
// from outside spec/ itself — read by a generator, or by the Rust interpreter,
// or by the TypeScript GUI/runtime. Declaring one that nothing reads by name
// is a contract the registry still promises for a concept the language no
// longer has: `interpretationRole` was exactly this — a field required by
// spec/words.schema.json, populated on all 65 entries, read by nothing.
//
// This check is a name-reachability heuristic, not a semantic proof: a field
// referenced anywhere in scripts/, rust/src/, or src/ counts as reachable,
// whether or not the code path that reads it is itself meaningful — and it
// does not distinguish a field that is read from one that is merely
// redundant with another field that already carries the same fact (that
// distinction needs a human comparison, the way `capability`/`hostedEffect`
// were found to duplicate the canonical `effects` field and removed anyway).
//
// It deliberately does not extend to errorWhen condition strings. A first
// version tried grepping the Rust source for each condition's literal text
// and flagged 27 of the registry's ~30 distinct conditions as unreachable —
// not because they are dead, but because Rust error messages are prose
// ("expected number, got other format"), not the camelCase identifiers
// words.json uses, so a real, reachable condition and a dead one look
// identical to a literal-match grep. Telling them apart needs domain
// knowledge — the way `stackTargetMode` was confirmed dead here by knowing
// the TOP/STAK modifier axis no longer exists in the language, not by
// searching for its spelling. That check is not automated by this gate.
//
// `scripts/check-outcome-bijection.mjs` now covers exactly this range, by a
// different method: it executes a witness for every declared NIL reason and
// error category (from the exhaustive table or spec/outcome-witnesses.json)
// and fails if any has none, which tells a live condition from a dead one by
// running it rather than by matching its name.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { readJson, repoRoot, reporter, walk } from './lib/common.mjs';

const schema = readJson('spec/words.schema.json');

// Excluded from its own haystack: this file's docstring names past dead
// fields as worked examples, which would otherwise make the gate satisfy
// itself the moment it explains what it caught.
const SELF_PATH = resolve(import.meta.dirname, 'check-unreachable-contract.mjs');

// The corpus a field must be referenced from to count as reachable:
// generators and checks (scripts/), the Rust interpreter (rust/src/), and the
// TypeScript GUI/runtime (src/). spec/ itself is excluded — declaring a field
// there is not using it, and tools/mcp-server's packaged assets are a synced
// copy of spec/words.json, not an independent consumer.
const haystackFiles = [
  ...walk(resolve(repoRoot, 'scripts'), ['.mjs']),
  ...walk(resolve(repoRoot, 'rust/src'), ['.rs']),
  ...walk(resolve(repoRoot, 'src'), ['.ts', '.tsx']),
].filter((f) => f !== SELF_PATH);
const haystack = haystackFiles.map((f) => readFileSync(f, 'utf8')).join('\n');

const report = reporter('unreachable-contract');
const fail = report.fail;

// Structural fields carry per-word mechanics (shape, prose, identity) rather
// than naming a concept, and scripts/rust/src reference them constantly by
// construction — checking them adds noise, not signal.
const STRUCTURAL_FIELDS = new Set([
  'name', 'aliases', 'stack', 'projection', 'errorWhen', 'clauses',
  'documentation', 'executorKey', 'effects', 'standardKind',
]);
const properties = schema.$defs.word.properties;
const checkedFields = Object.keys(properties).filter((field) => !STRUCTURAL_FIELDS.has(field));
for (const field of checkedFields) {
  const needle = new RegExp(`\\b${field}\\b`);
  if (!needle.test(haystack)) {
    fail(`field \`${field}\` is declared in spec/words.schema.json but referenced by no generator or implementation file`);
  }
}

report.done(`${checkedFields.length} classification field(s) are all reachable.`);
