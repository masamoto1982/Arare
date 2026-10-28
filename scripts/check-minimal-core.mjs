#!/usr/bin/env node
import { existsSync } from 'node:fs';
import { readJson, reporter, words as wordsDocument } from './lib/common.mjs';

const KERNEL = new Set(`TRUE FALSE AND NOT RECONCILE EQ LT GT
ADD MUL DIV FLOOR SQRT POW
GET LENGTH CONCAT COLLECT RANGE FOLD MAP SHAPE RESHAPE FLATTEN DEPTH
RECORD KEYS VALUES WITHOUT HAS? MERGE PUT
CHARS JOIN NUM STR
SELECT EXEC CONTRACT FAIL NIL NIL? NIL-REASON ABSENT BIND DEF DEL DIGEST PRINT`.split(/\s+/));
const STANDARD = new Set(`SUB ROUND MIN MAX GCD RATIO
TAKE DROP REVERSE FILL SORT ORDER UNIQUE TALLY ZIP GROUP BSEARCH FILTER SCAN
TRIM TOKENIZE SEARCH REPLACE UPPER LOWER FORMAT JSON-DECODE JSON-ENCODE`.split(/\s+/));
// The retired names live in spec/retired-words.json, the one representation
// this gate and rust/tests/beta_removed_words.rs both read: this side asserts
// none of them is canonical, the Rust side asserts the runtime does not
// resolve them. The reasons travel with the names there.
const retired = readJson('spec/retired-words.json');
const REMOVED_LIST = retired.groups.flatMap((group) => group.names);
const REMOVED = new Set(REMOVED_LIST);
const STANDARD_RELATIONS = new Set(['derivable', 'operational']);
const STANDARD_KINDS = new Set(['shorthand', 'namedPattern', 'algorithm', 'operational']);
const OPERATIONAL_LAW_TEST = 'rust/tests/standard_operational_laws.rs';
const DERIVATION_LAW_TEST = 'rust/tests/standard_derivation_laws.rs';
const DERIVABLE = new Set(`SUB ROUND MIN MAX
TAKE DROP REVERSE TRIM TOKENIZE`.split(/\s+/));
// `FORMAT` and the JSON pair are Phase 6 of the vocabulary-100 work order:
// `FORMAT` is the one rounding boundary (a FLOOR-and-STR spelling would
// scatter it), and JSON nesting is input-dependent repetition no definition
// can write, so all three are operational rather than derivable.
// Phase 7 closes the number concept: `GCD` is input-dependent repetition
// (Euclid) and `RATIO` reads representation the language otherwise hides;
// both operational.
const OPERATIONAL = new Set('FILTER SCAN FILL SORT ORDER UNIQUE TALLY ZIP GROUP BSEARCH SEARCH REPLACE FORMAT JSON-DECODE JSON-ENCODE GCD RATIO UPPER LOWER'.split(/\s+/));

// The order in which the vocabulary would give up Words, cheapest to lose first.
//
// The vocabulary is held at 77 (78 until BOTH retired with the three-valued
// truth domain, 2026-10), so an addition has to take a slot from
// something, and a candidate takes the head of this list. Every entry is a
// derivable Standard, checked below: an operational Word cannot be written in
// the language at all (LANG.AUTHORITY.FREEDOM's consequence for a total,
// non-recursive language), so retiring one would cut capability rather than
// spelling, and a Kernel Word is not on the table.
//
// SUB heads the list on the zero-shape reading (LANG.VALUES.SHAPE): it is the
// one derivable Word with no law of its own — its shape table is the sum
// table over a negated operand, as `x y -1 MUL ADD` is — and a wheel has no subtraction among its operations, only addition and
// the reciprocal. It is also the most used, so the move was the owner's call
// (2026-10): the next Word admitted takes SUB's slot first.
// The rest stay in order of use, least first, as counted in the lexicon
// pilot's programs (tools/lexicon-emergence/runs/pilot-2026-09-23): REVERSE 6,
// TRIM 10, TOKENIZE 12, MAX 13, MIN 14, TAKE 16, DROP 32, SUB 54 (written `-`).
// `ROUND` is deliberately absent though it is derivable: its Kernel phrase
// branches on the sign of its operand, which is the one of these a reader is
// likely to write wrongly by hand.
const RETIREMENT_QUEUE = 'SUB REVERSE TRIM TOKENIZE MAX MIN TAKE DROP'.split(/\s+/);

const words = wordsDocument().entries;
const coverage = readJson('docs/formalization-coverage.json');
const wordNames = new Set(words.map((word) => word.name));
const entries = coverage.entries.filter((entry) => wordNames.has(entry.surface));
const primitives = new Set(coverage.algebra_primitives.map((entry) => entry.id));
const bySurface = new Map();
const report = reporter('minimal-core');
const fail = report.fail;
const setDifference = (left, right) => [...left].filter((item) => !right.has(item));

for (const entry of entries) {
  if (bySurface.has(entry.surface)) fail(`duplicate Core Word witness: ${entry.surface}`);
  bySurface.set(entry.surface, entry);
}

const kernelWords = new Set(words.filter((word) => word.vocabularyTier === 'kernel').map((word) => word.name));
const standardWords = new Set(words.filter((word) => word.vocabularyTier === 'standard').map((word) => word.name));
for (const name of setDifference(KERNEL, kernelWords)) fail(`${name}: missing Semantic Kernel classification`);
for (const name of setDifference(kernelWords, KERNEL)) fail(`${name}: unexpected Semantic Kernel classification`);
for (const name of setDifference(STANDARD, standardWords)) fail(`${name}: missing Standard classification`);
for (const name of setDifference(standardWords, STANDARD)) fail(`${name}: unexpected Standard classification`);
if (kernelWords.size !== 49) fail(`Semantic Kernel has ${kernelWords.size} Words; expected 49`);
if (standardWords.size !== 28) fail(`Standard vocabulary has ${standardWords.size} Words; expected 28`);

if (words.length !== 77) fail(`canonical inventory has ${words.length} Words; expected 77`);
for (const name of REMOVED) if (wordNames.has(name)) fail(`${name}: removed Word remains canonical`);
if (REMOVED.size !== REMOVED_LIST.length) fail('spec/retired-words.json names the same Word twice');

for (const word of words) {
  const witness = bySurface.get(word.name);
  if (!witness) {
    fail(`${word.name}: missing Minimal Core witness`);
    continue;
  }
  if (!['Formalized', 'HostedEffect'].includes(witness.status)) fail(`${word.name}: status is ${witness.status}`);
  if (!witness.law_tests?.length) fail(`${word.name}: no executable law test`);
  for (const testPath of witness.law_tests ?? []) {
    if (!existsSync(testPath)) fail(`${word.name}: missing law test file ${testPath}`);
  }
  if (!['identity', 'flow', 'material'].includes(witness.core_tier)) fail(`${word.name}: invalid core_tier ${witness.core_tier}`);
  if (witness.core_tier === word.vocabularyTier) fail(`${word.name}: core_tier is confused with vocabularyTier`);
  if (witness.semantic_role === 'Primitive') {
    if (!witness.primitive) fail(`${word.name}: Primitive role is not marked primitive`);
  } else if (witness.semantic_role === 'Derived') {
    if (witness.primitive) fail(`${word.name}: Derived role is marked primitive`);
    if (!witness.derived_from?.length) fail(`${word.name}: derived Word has no algebra basis`);
  } else if (witness.semantic_role !== 'HostedEffect') {
    fail(`${word.name}: unsupported semantic role ${witness.semantic_role}`);
  }
  for (const dependency of witness.derived_from ?? []) {
    if (!primitives.has(dependency)) fail(`${word.name}: unknown algebra primitive ${dependency}`);
  }
  if (word.vocabularyTier === 'standard') {
    if (!STANDARD_KINDS.has(word.standardKind)) fail(`${word.name}: invalid or missing standardKind`);
    if (!STANDARD_RELATIONS.has(witness.standard_relation)) fail(`${word.name}: invalid or missing Standard relation`);
    if (!witness.conformance_cases?.length) fail(`${word.name}: Standard Word has no conformance case`);
    if (witness.standard_relation === 'operational' && !witness.native_retention_reason) {
      fail(`${word.name}: operational Standard has no native retention reason`);
    }
    if (witness.standard_relation === 'operational' && !witness.law_tests.includes(OPERATIONAL_LAW_TEST)) {
      fail(`${word.name}: operational Standard is not covered by ${OPERATIONAL_LAW_TEST}`);
    }
    if (witness.standard_relation === 'derivable' && !witness.law_tests.includes(DERIVATION_LAW_TEST)) {
      fail(`${word.name}: derivable Standard has no Kernel witness in ${DERIVATION_LAW_TEST}`);
    }
    if (DERIVABLE.has(word.name) && witness.standard_relation !== 'derivable') {
      fail(`${word.name}: expected the derivable relation, found ${witness.standard_relation}`);
    }
    if (OPERATIONAL.has(word.name) && witness.standard_relation !== 'operational') {
      fail(`${word.name}: expected the operational relation, found ${witness.standard_relation}`);
    }
  }
}

const derivableWords = new Set(
  words.filter((word) => bySurface.get(word.name)?.standard_relation === 'derivable').map((word) => word.name),
);
const operationalWords = new Set(
  words.filter((word) => bySurface.get(word.name)?.standard_relation === 'operational').map((word) => word.name),
);
if (derivableWords.size !== DERIVABLE.size) {
  fail(`${derivableWords.size} derivable Standards declared; expected ${DERIVABLE.size}`);
}
if (operationalWords.size !== OPERATIONAL.size) {
  fail(`${operationalWords.size} operational Standards declared; expected ${OPERATIONAL.size}`);
}

// The retirement queue has to stay the thing it claims to be: every derivable
// Standard but `ROUND`, each one a spelling of a Kernel phrase rather than a
// capability. A queue naming a Word that has already gone, or one whose
// relation drifted to operational, would promise room it cannot give.
for (const name of setDifference(DERIVABLE, new Set([...RETIREMENT_QUEUE, 'ROUND']))) {
  fail(`${name}: derivable Standard missing from the retirement queue`);
}
if (new Set(RETIREMENT_QUEUE).size !== RETIREMENT_QUEUE.length) {
  fail('retirement queue names the same Word twice');
}
for (const name of RETIREMENT_QUEUE) {
  if (!wordNames.has(name)) {
    fail(`${name}: queued for retirement but not in the canonical inventory`);
    continue;
  }
  if (!DERIVABLE.has(name)) {
    fail(`${name}: queued for retirement but not a derivable Standard`);
  }
}

for (const entry of coverage.entries.filter((entry) => entry.kind === 'coreword')) {
  if (!wordNames.has(entry.surface)) fail(`${entry.surface}: witness has no canonical Word`);
}
if (bySurface.size !== words.length) fail(`witness inventory has ${bySurface.size} entries; expected ${words.length}`);

report.done(
  `${kernelWords.size}/${kernelWords.size} Semantic Kernel Words have executable witnesses.`,
);
console.log(
  `[minimal-core] ${standardWords.size}/${standardWords.size} Standard Words have complete contracts and law witnesses.`,
);
console.log(
  `[minimal-core] ${DERIVABLE.size}/${DERIVABLE.size} derivable Standards carry a Kernel-only witness; ` +
    `${OPERATIONAL.size}/${OPERATIONAL.size} operational Standards state a native retention reason.`,
);
console.log(
  `[minimal-core] ${RETIREMENT_QUEUE.length} Words queued for retirement, first out ${RETIREMENT_QUEUE[0]}: ` +
    RETIREMENT_QUEUE.join(' '),
);
