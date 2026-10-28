#!/usr/bin/env node
// Gate for spec/grammar.json, the canonical lexical grammar.
//
// The grammar is the first canonical source that is *executed* rather than
// merely read: scripts/lib/reference-lexer.mjs interprets it, so building the
// lexer at all proves the grammar's actions and matchers are ones the closed
// vocabulary defines. On top of that this gate closes the grammar against the
// registries it must agree with:
//
//   - every source-error condition a rule names exists, and every declared
//     condition is reachable from some rule (no dead entries, no dangling ones)
//   - every error category the grammar names exists in spec/outcomes.json
//   - every lexical surface form in the word manifest is produced by some
//     production, rejected by name, or declared lexically transparent with a
//     reason — and every surface the grammar names exists in the manifest
//   - the lexeme rules are total, and the numeric grammar's own examples lex
//     the way it says they do
//
// Unlike a shape check, the last item is a *truth* condition: the examples are
// run through the grammar, not inspected.

import { readJson, repoRoot, reporter } from './lib/common.mjs';
import { loadGrammar, makeLexer } from './lib/reference-lexer.mjs';

const report = reporter('grammar', { countSummary: true });
const fail = report.fail;

const grammar = loadGrammar(repoRoot);
const outcomes = readJson('spec/outcomes.json');
const manifest = readJson('docs/word-manifest.json');

// Building the lexer validates the grammar's action and matcher vocabulary.
const lex = makeLexer(grammar);

// ---------------------------------------------------------------- conditions

const declaredConditions = new Set(grammar.sourceErrors.map((e) => e.id));
const referencedConditions = new Set();

for (const phase of grammar.phases) {
  for (const group of phase.rejectedCharacters ?? []) {
    referencedConditions.add(group.condition);
  }
  for (const condition of phase.conditions ?? []) {
    referencedConditions.add(condition);
  }
}
for (const rule of grammar.lexemeRules) {
  if (rule.condition) referencedConditions.add(rule.condition);
}
referencedConditions.add(grammar.stringLiteral.condition);

for (const condition of referencedConditions) {
  if (!declaredConditions.has(condition)) {
    fail(`rule names source-error condition "${condition}" with no entry in sourceErrors`);
  }
}
for (const condition of declaredConditions) {
  if (!referencedConditions.has(condition)) {
    fail(
      `sourceErrors declares "${condition}" but no rule or phase reaches it — an unreachable condition is a claim nothing can make true`,
    );
  }
}

// Each condition's witness is run, not inspected: the grammar must actually
// reach the condition it says the witness reaches. The same witnesses are the
// corpus the Rust side runs in rust/src/lexical_grammar_laws.rs, so the two
// implementations are held to one shared set of programs.
for (const entry of grammar.sourceErrors) {
  const result = lex(entry.witness);
  if (result.condition !== entry.id) {
    fail(
      `sourceErrors "${entry.id}" claims witness ${JSON.stringify(entry.witness)} reaches it, but the grammar yields ${describe(result)}`,
    );
  }
}

// ------------------------------------------------------------ error category

const errorCategories = new Set(
  (outcomes.errorCategories ?? []).map((entry) => entry.id),
);
for (const entry of grammar.sourceErrors) {
  if (!errorCategories.has(entry.errorCategory)) {
    fail(
      `sourceErrors "${entry.id}" names error category "${entry.errorCategory}", which spec/outcomes.json does not define`,
    );
  }
}

// ------------------------------------------------------------- phase linkage

const phaseIds = new Set(grammar.phases.map((p) => p.id));
for (const entry of grammar.sourceErrors) {
  if (!phaseIds.has(entry.phase)) {
    fail(`sourceErrors "${entry.id}" names phase "${entry.phase}", which is not a declared phase`);
  }
}

// ------------------------------------------------------------- rule totality

const lastRule = grammar.lexemeRules[grammar.lexemeRules.length - 1];
if (!lastRule?.match?.otherwise) {
  fail(
    'the last lexeme rule must be an "otherwise" fallback, or lexeme classification is not total',
  );
}
const lastPositionRule = grammar.phases
  .find((p) => p.id === 'scan')
  ?.positionRules?.slice(-1)[0];
if (!lastPositionRule?.guard?.otherwise) {
  fail(
    'the last scan position rule must be an "otherwise" fallback, or scanning is not total',
  );
}

// --------------------------------------------------------- character classes

const { whitespace, lineTerminator } = grammar.characterClasses;
const expand = (spec) => {
  const out = new Set();
  for (const entry of spec.codepoints ?? []) {
    const [lo, hi] = entry.split('-');
    const start = Number.parseInt(lo, 16);
    const end = Number.parseInt(hi ?? lo, 16);
    for (let cp = start; cp <= end; cp += 1) out.add(cp);
  }
  return out;
};
const whitespaceSet = expand(whitespace);
for (const cp of expand(lineTerminator)) {
  if (!whitespaceSet.has(cp)) {
    fail(
      `lineTerminator U+${cp.toString(16).toUpperCase()} is not in the whitespace class; a line terminator that does not delimit tokens would leave the scan rules inconsistent`,
    );
  }
}
if (whitespaceSet.has(0xfeff)) {
  fail(
    'U+FEFF is listed as whitespace, but it is not in Unicode White_Space — the implementation treats it as a name character',
  );
}

// -------------------------------------------------- numeric grammar examples

for (const lexeme of grammar.numericGrammar.examples.accepted) {
  const result = lex(lexeme);
  const tokens = result.tokens;
  if (!tokens || tokens.length !== 1 || tokens[0].id !== 'Number') {
    fail(
      `numericGrammar lists ${JSON.stringify(lexeme)} as accepted, but the grammar lexes it as ${describe(result)}`,
    );
  }
}
for (const lexeme of grammar.numericGrammar.examples.rejectedAsName) {
  const result = lex(lexeme);
  const tokens = result.tokens;
  const isNumber = tokens?.length === 1 && tokens[0].id === 'Number';
  if (isNumber) {
    fail(
      `numericGrammar lists ${JSON.stringify(lexeme)} as rejected, but the grammar lexes it as a Number`,
    );
  }
}

function describe(result) {
  if (result.condition) return `source error "${result.condition}"`;
  return `[${result.tokens.map((t) => t.id).join(', ')}]`;
}

// ------------------------------------------------------- manifest closure

const LEXICAL_KINDS = new Set([
  'source_directive',
  'control_directive',
  'delimiter_sugar',
  'literal_sugar',
  'reserved_marker',
  'retired_form',
]);

const producedSurfaces = new Set();
for (const phase of grammar.phases) {
  for (const rule of phase.positionRules ?? []) {
    if (rule.surface) producedSurfaces.add(rule.surface);
  }
  for (const group of phase.rejectedCharacters ?? []) {
    for (const ch of group.chars) producedSurfaces.add(ch);
  }
}
for (const rule of grammar.lexemeRules) {
  if (rule.surface) producedSurfaces.add(rule.surface);
}
const transparentSurfaces = new Set(
  (grammar.lexicallyTransparent ?? []).map((e) => e.surface),
);

const manifestSurfaces = new Set(manifest.entries.map((e) => e.surface));

for (const entry of manifest.entries) {
  if (!LEXICAL_KINDS.has(entry.kind)) continue;
  if (producedSurfaces.has(entry.surface)) continue;
  if (transparentSurfaces.has(entry.surface)) continue;
  fail(
    `manifest entry ${entry.id} (${entry.surface}, ${entry.kind}) is a lexical surface form that no grammar production produces, rejects, or declares lexically transparent`,
  );
}

for (const surface of producedSurfaces) {
  if (!manifestSurfaces.has(surface)) {
    fail(
      `the grammar allocates surface "${surface}" but the word manifest has no entry for it`,
    );
  }
}
for (const surface of transparentSurfaces) {
  if (!manifestSurfaces.has(surface)) {
    fail(
      `lexicallyTransparent lists "${surface}" but the word manifest has no entry for it`,
    );
  }
}

// ------------------------------------------------------------------- report

report.done(
  `${grammar.lexemeRules.length} lexeme rules, ${grammar.phases.length} phases, ${grammar.sourceErrors.length} source-error conditions, ${producedSurfaces.size} surfaces produced — all closed against outcomes and the word manifest`,
);
