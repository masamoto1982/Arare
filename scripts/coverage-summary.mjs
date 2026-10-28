#!/usr/bin/env node
// Renders CI's two coverage reports as Markdown for the job summary: the Rust
// (`cargo llvm-cov`) and TypeScript (Vitest, istanbul) totals, then every
// source file the traceability matrix names as the implementation of a
// requirement, with the requirement and quality level beside it.
//
// The file list is read from docs/quality/TRACEABILITY_MATRIX.md rather than
// written down here, so a requirement row added or retargeted there shows up
// in the summary without touching this script.
//
// The summary only reports. The gate is scripts/check-coverage-ratchet.mjs,
// which fails CI when a QL-A or QL-B file loses branch or line coverage against
// docs/quality/coverage-baseline.json; every other figure here is evidence
// for a reviewer, not a threshold.
//
// Usage: node scripts/coverage-summary.mjs --rust <coverage.json> --ts <coverage-summary.json>

import { languageOf, readCoverageExport, readVitestSummary, tracedFiles } from './lib/coverage.mjs';

const TAG = 'coverage-summary';
const flag = (name) => {
  const at = process.argv.indexOf(name);
  return at === -1 ? undefined : process.argv[at + 1];
};
const rustPath = flag('--rust');
const tsPath = flag('--ts');
if (!rustPath || !tsPath) {
  console.error('usage: node scripts/coverage-summary.mjs --rust <coverage.json> --ts <coverage-summary.json>');
  process.exit(2);
}

const reports = { rust: readCoverageExport(rustPath, TAG), ts: readVitestSummary(tsPath, TAG) };

const pct = ({ count, covered }) => (count === 0 ? '—' : `${((100 * covered) / count).toFixed(1)}%`);
const cell = (metric) => `${pct(metric)} (${metric.count - metric.covered} missed)`;

const out = [];
out.push('## Coverage', '');
out.push(
  'QL-A and QL-B files are gated: CI fails if one loses branch or line coverage against `docs/quality/coverage-baseline.json`. Every other figure is reported only.',
  '',
);
out.push('| Scope | Branches | Lines | Regions / statements | Functions |', '|---|---|---|---|---|');
for (const [label, { totals }] of [
  ['**Rust** (`cargo llvm-cov --branch`, pinned nightly)', reports.rust],
  ['**TypeScript** (Vitest, istanbul)', reports.ts],
]) {
  out.push(`| ${label} | ${cell(totals.branches)} | ${cell(totals.lines)} | ${cell(totals.regions)} | ${pct(totals.functions)} |`);
}
out.push('', '### Files implementing a traced requirement', '');
out.push('| File | Requirement | QL | Branches | Lines | Regions / statements |', '|---|---|---|---|---|---|');
for (const [path, { requirements, level }] of tracedFiles()) {
  const summary = reports[languageOf(path)].files.get(path);
  const head = `| \`${path}\` | ${requirements.join(', ')} | ${level}`;
  if (!summary) {
    out.push(`${head} | not measured (feature-gated or untested build) | | |`);
    continue;
  }
  out.push(`${head} | ${cell(summary.branches)} | ${cell(summary.lines)} | ${cell(summary.regions)} |`);
}
out.push('');
console.log(out.join('\n'));
