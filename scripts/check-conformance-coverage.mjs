#!/usr/bin/env node
// CI gate over tests/conformance/index.html, three ways:
//
//   1. Every Core-classified surface word (docs/word-manifest.json) appears in
//      at least one `ajisai-source` program.
//   2. Every outcome spec/outcomes.json declares is reached by running the
//      corpus: each NIL reason appears in some case's final stack (or is the
//      answer of a NIL-REASON there), and each error category is some case's
//      error — except the categories in OUTCOME_EXEMPT, each with its reason.
//      The corpus is what decides whether an implementation is Ajisai, so an
//      outcome no case reaches is one a port could get wrong unnoticed.
//   3. Every case's data-category is a family of spec/words.json or
//      `language`, and the per-category counts are printed.
//
// Exits non-zero if any of the three fails.
//
// Usage:
//   node scripts/check-conformance-coverage.mjs            # human summary
//   node scripts/check-conformance-coverage.mjs --json     # JSON report
//   node scripts/check-conformance-coverage.mjs --suite F  # alternate suite file

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { decodeEntities, readJson, repoRoot, resolveAjisaiBin, runAgent, words } from './lib/common.mjs';

const args = process.argv.slice(2);
const asJson = args.includes('--json');
const suiteFlag = args.indexOf('--suite');
const suitePath = suiteFlag !== -1 && args[suiteFlag + 1]
  ? resolve(args[suiteFlag + 1])
  : resolve(repoRoot, 'tests/conformance/index.html');

const manifest = readJson('docs/word-manifest.json');

const coreWords = new Set(
  manifest.entries
    .filter((e) => e.classification === 'Core')
    .map((e) => e.surface),
);

const html = readFileSync(suitePath, 'utf8');
const sources = [];
const srcPattern = /<pre class="ajisai-source">([\s\S]*?)<\/pre>/g;
for (const m of html.matchAll(srcPattern)) {
  sources.push(decodeEntities(m[1]));
}
if (sources.length === 0) {
  console.error(`no ajisai-source programs found in ${suitePath}`);
  process.exit(2);
}

const seen = new Set();
for (const src of sources) {
  for (const tok of src.split(/\s+/)) {
    if (tok) seen.add(tok);
  }
}

const covered = [...coreWords].filter((w) => seen.has(w)).sort();
const missing = [...coreWords].filter((w) => !seen.has(w)).sort();
const pct = (100 * covered.length) / coreWords.size;

if (asJson) {
  process.stdout.write(`${JSON.stringify({
    schemaVersion: 3,
    suite: suitePath,
    caseSources: sources.length,
    coreWords: coreWords.size,
    covered: covered.length,
    coveragePercent: Number(pct.toFixed(1)),
    missing,
  }, null, 2)}\n`);
} else {
  console.log(`conformance suite: ${sources.length} case sources`);
  console.log(`Core Profile words: ${coreWords.size}`);
  console.log(`covered: ${covered.length} (${pct.toFixed(1)}%)`);
  if (missing.length) {
    console.log(`missing (${missing.length}): ${missing.join(' ')}`);
  } else {
    console.log('missing: none');
  }
}

if (missing.length > 0) {
  console.error(
    `[conformance-coverage] ${missing.length} Core word(s) `
    + 'have no conformance case. Add an `ajisai-source` case that uses each, '
    + 'or run with --json to see the full list.',
  );
  process.exitCode = 1;
}

// ------------------------------------------------------------ categories

const families = new Set(words().entries.map((w) => w.family));
const categories = {};
for (const m of html.matchAll(/<section class="ajisai-case" id="([^"]+)"[^>]*data-category="([^"]+)"/g)) {
  const [, id, category] = m;
  if (category !== 'language' && !families.has(category)) {
    console.error(`[conformance-coverage] case ${id}: data-category "${category}" is neither a Word family nor "language"`);
    process.exitCode = 1;
  }
  categories[category] = (categories[category] ?? 0) + 1;
}
if (!asJson) {
  console.log(`categories: ${Object.entries(categories).sort((a, b) => b[1] - a[1]).map(([c, n]) => `${c} ${n}`).join(', ')}`);
}

// --------------------------------------------------------------- outcomes

// An outcome the corpus may leave unreached, and why. Nothing else may be.
const OUTCOME_EXEMPT = {
  contractViolation: 'decided by the host\'s pre-execution check of a #:contract declaration (LANG.CONTRACT.CHECK), not by running a program',
  executionLimitExceeded: 'the step ceiling is the host\'s to set (LANG.MACHINE.LIMITS), so no case can pin where it fires',
};
const outcomes = readJson('spec/outcomes.json');
const reasonIds = new Set(outcomes.nilReasons.map((r) => r.id));
const reachedReasons = new Set();
const reachedErrors = new Set();
const collectReasons = (node) => {
  if (!node || typeof node !== 'object') return;
  const reason = node.semantics?.absence?.reason;
  if (node.type === 'nil' && reason) reachedReasons.add(reason);
  if (node.type === 'record' && node.value) [...node.value.keys, ...node.value.values].forEach(collectReasons);
  else if (Array.isArray(node.value)) node.value.forEach(collectReasons);
};
const bin = resolveAjisaiBin('conformance-coverage');
for (const source of sources) {
  const report = runAgent(bin, source);
  if (report.error?.category) reachedErrors.add(report.error.category);
  (report.stack ?? []).forEach(collectReasons);
  if (source.includes('NIL-REASON')) {
    for (const shown of report.stackDisplay ?? []) {
      const quoted = shown.match(/^'(\w+)'$/);
      if (quoted && reasonIds.has(quoted[1])) reachedReasons.add(quoted[1]);
    }
  }
}
const unreachedReasons = [...reasonIds].filter((id) => !reachedReasons.has(id));
const unreachedErrors = outcomes.errorCategories
  .map((c) => c.id)
  .filter((id) => !reachedErrors.has(id) && !(id in OUTCOME_EXEMPT));
for (const id of Object.keys(OUTCOME_EXEMPT)) {
  if (!outcomes.errorCategories.some((c) => c.id === id)) {
    console.error(`[conformance-coverage] OUTCOME_EXEMPT names ${id}, which spec/outcomes.json does not declare`);
    process.exitCode = 1;
  }
}
if (unreachedReasons.length || unreachedErrors.length) {
  console.error(
    `[conformance-coverage] outcomes no case reaches — NIL reasons: ${unreachedReasons.join(' ') || 'none'}; ` +
      `error categories: ${unreachedErrors.join(' ') || 'none'}. Add a case that reaches each.`,
  );
  process.exitCode = 1;
} else if (!asJson) {
  console.log(
    `outcomes: all ${reasonIds.size} NIL reasons and ${outcomes.errorCategories.length - Object.keys(OUTCOME_EXEMPT).length} ` +
      `error categories reached (${Object.keys(OUTCOME_EXEMPT).join(', ')} exempt)`,
  );
}
