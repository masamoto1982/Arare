// What scripts/coverage-summary.mjs and scripts/check-coverage-ratchet.mjs
// both read: the source files the traceability matrix names as a
// requirement's implementation, and the two coverage reports CI produces —
// a `cargo llvm-cov report --json --summary-only` export for Rust and a
// Vitest (istanbul) `coverage-summary.json` for TypeScript — each normalized
// to one shape keyed by repository-relative path. One reading of each, so the
// summary a reviewer sees and the gate CI enforces cannot disagree about
// which files are gated or what was measured for them.

import { readFileSync } from 'node:fs';
import { relative, resolve } from 'node:path';
import { readText, repoRoot } from './common.mjs';

/** Which report measures a traced file: Rust sources live under rust/src. */
export const languageOf = (path) => (path.startsWith('rust/') ? 'rust' : 'ts');

/**
 * Implementation files (Rust and TypeScript) of every requirement row in
 * docs/quality/TRACEABILITY_MATRIX.md, in matrix order:
 * `path -> { requirements: string[], level: 'QL-A' | ... }`.
 */
export function tracedFiles() {
  const traced = new Map();
  // Requirement rows: | **AQ-REQ-00N** | statement | implementation | verification | QL |
  for (const line of readText('docs/quality/TRACEABILITY_MATRIX.md').split('\n')) {
    const cells = line.split('|').map((cell) => cell.trim());
    const requirement = cells[1]?.match(/AQ-REQ-\d{3}/)?.[0];
    if (!requirement || cells.length < 6) continue;
    const level = cells[5];
    for (const [, path] of cells[3].matchAll(/`((?:rust\/)?src\/[^`]+\.(?:rs|ts))`/g)) {
      const entry = traced.get(path) ?? { requirements: [], level };
      entry.requirements.push(requirement);
      traced.set(path, entry);
    }
  }
  return traced;
}

const rel = (filename) => relative(repoRoot, filename).split('\\').join('/');

function fail(tag, message) {
  console.error(`[${tag}] ${message}`);
  process.exit(1);
}

/**
 * An llvm-cov JSON export: `{ totals, files }`, `files` mapping each path to
 * its summary (`branches`, `lines`, `regions`, `functions`, each with
 * `count` and `covered`). Exits on anything else, so a gate never passes for
 * want of data.
 */
export function readCoverageExport(exportPath, tag) {
  const exported = JSON.parse(readFileSync(resolve(exportPath), 'utf8'));
  const data = exported.data?.[0];
  if (!data?.files || !data?.totals) {
    fail(tag, `${exportPath} is not an llvm-cov JSON export (no data[0].files / totals)`);
  }
  return { totals: data.totals, files: new Map(data.files.map((file) => [rel(file.filename), file.summary])) };
}

/**
 * A Vitest istanbul `coverage-summary.json`, in the same shape as
 * `readCoverageExport`: istanbul's `{ total, covered }` per metric becomes
 * `{ count, covered }`, and `statements` stands where llvm-cov has `regions`.
 */
export function readVitestSummary(summaryPath, tag) {
  const summary = JSON.parse(readFileSync(resolve(summaryPath), 'utf8'));
  if (!summary?.total?.lines) fail(tag, `${summaryPath} is not an istanbul coverage-summary.json (no total.lines)`);
  const normalize = (entry) => ({
    branches: { count: entry.branches.total, covered: entry.branches.covered },
    lines: { count: entry.lines.total, covered: entry.lines.covered },
    regions: { count: entry.statements.total, covered: entry.statements.covered },
    functions: { count: entry.functions.total, covered: entry.functions.covered },
  });
  const files = new Map(
    Object.entries(summary)
      .filter(([key]) => key !== 'total')
      .map(([filename, entry]) => [rel(filename), normalize(entry)]),
  );
  return { totals: normalize(summary.total), files };
}
