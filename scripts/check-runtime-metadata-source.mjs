#!/usr/bin/env node
// Enforces the invariant that the Core Words' runtime view is the generated
// registry itself (rust/src/kernel/generated/word_registry.rs, projected from
// spec/words.json), never a copy of it and never a table of its own.
//
// The failure this exists to prevent is not a particular retired type name; it
// is the shape of the bug. A hand-authored Core Word metadata table
// (historically `RuntimeSpec` + `SPEC_DEFAULT`) appears alongside the registry
// and the two disagree — or, as happened once, one side is deleted while the
// other keeps importing it and the tree stops compiling. The projection that
// replaced that table (`BuiltinSpec`, seven registry fields copied one for one
// into a second `OnceLock`'d table) is gone too: a copy that can only ever
// equal its source is a second place for the facts to live, and this gate
// spent its whole budget proving the copy faithful. Grepping for old names
// would only catch a repeat under the same spelling, and would pass the moment
// someone calls the parallel table something else.
//
// So the check is positive, over the shape of rust/src/builtins.rs:
//
//   - no struct declared there carries a field of `GeneratedWord` (a struct
//     that does is a copy or a table, whatever it is named);
//   - lookup by name answers the registry entry itself (`&'static
//     GeneratedWord`, through `generated_word`);
//   - the one tuple projection the wasm bindings read iterates
//     `GENERATED_WORDS`, so the inventory has no second source either.
//
// Together with `word-registry:check` (the generated registry matches
// spec/words.json) this closes the path from the canonical source to the
// runtime view.

import { matchBrace, readText, reporter } from './lib/common.mjs';

const DEFINITIONS = 'rust/src/builtins.rs';
const CONTRACT = 'rust/src/kernel/generated/word_registry.rs';

const report = reporter('runtime-metadata');
const fail = report.fail;

// Rust source is scanned with a brace matcher (scripts/lib/common.mjs
// `matchBrace`) rather than a regex: field initializers contain nested calls
// and generics, so a non-greedy match would stop at the first `}` inside the
// body.

// Split `a: x, b: f(y, z)` on top-level commas only, so an argument list does
// not read as a field boundary.
function splitTopLevel(body) {
  const parts = [];
  let depth = 0;
  let inString = false;
  let current = '';
  for (let i = 0; i < body.length; i += 1) {
    const char = body[i];
    if (inString) {
      current += char;
      if (char === '\\') current += body[(i += 1)];
      else if (char === '"') inString = false;
      continue;
    }
    if (char === '"') inString = true;
    else if ('([{<'.includes(char)) depth += 1;
    else if (')]}>'.includes(char)) depth -= 1;
    if (char === ',' && depth === 0) {
      parts.push(current);
      current = '';
      continue;
    }
    current += char;
  }
  parts.push(current);
  return parts.map((part) => part.trim()).filter(Boolean);
}

// Strip line comments and attributes so `#[allow(dead_code)]` between fields
// does not read as a field.
function structFields(source, declaration, label) {
  const at = source.indexOf(declaration);
  if (at === -1) {
    fail(`${label}: could not find \`${declaration}\``);
    return [];
  }
  const open = source.indexOf('{', at);
  const close = matchBrace(source, open);
  const body = source
    .slice(open + 1, close)
    .replace(/\/\/[^\n]*/g, '')
    .replace(/#\[[^\]]*\]/g, '');
  return splitTopLevel(body)
    .map((field) => field.match(/^(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:/)?.[1])
    .filter(Boolean);
}

const definitions = readText(DEFINITIONS);
const contract = readText(CONTRACT);

// The registry must actually be generator output. If it were hand-editable,
// "sourced from `word.x`" would guarantee nothing.
if (!contract.startsWith('// @generated')) {
  fail(`${CONTRACT}: missing \`// @generated\` banner; it must be generator output`);
}

const contractFields = new Set(structFields(contract, 'pub struct GeneratedWord', CONTRACT));
if (contractFields.size === 0) fail(`${CONTRACT}: GeneratedWord declares no fields`);

// No struct in the runtime view may carry a registry field. One that does is a
// copy of the registry or a table beside it, whatever it is called.
const structPattern = /\b(?:pub(?:\([^)]*\))?\s+)?struct\s+([A-Z][A-Za-z0-9_]*)\s*\{/g;
for (const match of definitions.matchAll(structPattern)) {
  const name = match[1];
  const line = definitions.slice(0, match.index).split('\n').length;
  const copied = structFields(definitions, match[0], DEFINITIONS).filter((field) =>
    contractFields.has(field),
  );
  if (copied.length > 0) {
    fail(
      `${DEFINITIONS}:${line}: struct ${name} declares ${copied.join(', ')}, ` +
        `which are fields of GeneratedWord in ${CONTRACT}. The registry entry is the runtime ` +
        'view; Core Word metadata is neither copied out of it nor authored beside it.',
    );
  }
}

// Lookup by name answers the registry entry itself.
const lookup = definitions.match(
  /pub fn lookup_builtin_spec\(name: &str\) -> Option<&'static GeneratedWord> \{/,
);
if (!lookup) {
  fail(`${DEFINITIONS}: lookup_builtin_spec must answer \`Option<&'static GeneratedWord>\``);
} else {
  const open = definitions.indexOf('{', lookup.index);
  const body = definitions.slice(open + 1, matchBrace(definitions, open));
  if (!/\bgenerated_word\(/.test(body)) {
    fail(`${DEFINITIONS}: lookup_builtin_spec must resolve through generated_word()`);
  }
}

// The tuple projection the wasm bindings read must iterate the generated
// registry. Iterating anything else would mean the inventory has a second
// source.
const at = definitions.indexOf('fn collect_core_builtin_definitions(');
if (at === -1) {
  fail(`${DEFINITIONS}: could not find collect_core_builtin_definitions()`);
} else {
  const open = definitions.indexOf('{', at);
  const body = definitions.slice(open + 1, matchBrace(definitions, open));
  if (!/GENERATED_WORDS\s*\n?\s*\.iter\(\)/.test(body)) {
    fail(`${DEFINITIONS}: collect_core_builtin_definitions() must iterate GENERATED_WORDS`);
  }
}

// Each failure above names the canonical source the runtime view must read;
// `done` exits 1 when there was any.
report.done(
  `the runtime view reads the generated registry (${contractFields.size} fields) directly; no copy and no authored metadata table.`,
);
