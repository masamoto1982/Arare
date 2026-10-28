#!/usr/bin/env node
// docs/quality/TRACEABILITY_MATRIX.md is the requirement -> implementation ->
// verification link QUALITY_POLICY.md §2 requires and RELEASE_VERIFICATION_
// CHECKLIST.md asks to be gap-free. Seven source files cite it by path.
//
// For most of the repository's history it did not exist: every one of those
// citations was dangling, and nothing said so. Writing it down fixes that once.
// This gate is what keeps it fixed, because a matrix drifts in two directions
// and both had already happened by the time it was written:
//
//   - a suite is deleted and its rows stay, leaving the matrix claiming
//     coverage that is gone (AQ-VER-003-A, -003-B and -007-E were removed with
//     their subjects, and no check noticed);
//   - a suite is added with a fresh ID and no row, so the matrix silently
//     stops being the whole picture.
//
// So: every ID in the source must be in the matrix, every live ID and path in
// the matrix must be in the tree, and an ID the matrix calls retired must not
// still be in the source. This is a name-reachability check, the same shape as
// check-docs-dev-drift.mjs — it cannot tell whether a row *describes* its suite
// correctly, only whether both ends still exist.

import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { readText, repoRoot, reporter, walk } from './lib/common.mjs';

const matrixPath = 'docs/quality/TRACEABILITY_MATRIX.md';
const RETIRED_HEADING = '## Retired verification IDs';
const UNASSIGNED_HEADING = '## Unassigned requirement IDs';

// Both shapes the repository writes: AQ-REQ-001, AQ-VER-001, AQ-VER-001-A.
const ID_RE = /\bAQ-(?:REQ|VER)-\d{3}(?:-[A-Z]\d?)?\b/g;

// A backticked token counts as a path claim when it looks like one: it has a
// separator and either an extension this repository uses or a trailing slash
// (a directory). `LANG.FAILURE.PROJECT`, `safe_preview` and `npm run x` do not
// match, which is the point.
const PATH_RE = /`([A-Za-z0-9_.\-]+(?:\/[A-Za-z0-9_.\-]+)+\/?)`/g;
const PATH_EXT = /\.(rs|ts|tsx|mjs|js|json|md|html|toml|yml)$/;

const report = reporter('traceability');
function fail(message, detail) {
  report.fail(message);
  for (const line of detail ?? []) console.error(`    ${line}`);
}

if (!existsSync(resolve(repoRoot, matrixPath))) {
  fail(`${matrixPath} is missing, but the quality process and the source citations both require it.`);
  process.exit(1);
}

const matrixText = readText(matrixPath);
// Everything before the first of the two closing sections is a live claim;
// everything from there on names an ID that must NOT be in the source, whether
// because it was retired or because it was never assigned.
const cutAt = [UNASSIGNED_HEADING, RETIRED_HEADING]
  .map((heading) => matrixText.indexOf(heading))
  .filter((at) => at !== -1);
if (cutAt.length !== 2) {
  fail(`${matrixPath} needs both a "${UNASSIGNED_HEADING}" and a "${RETIRED_HEADING}" section; this check uses them to tell a live row from a closed one.`);
  process.exit(1);
}
const firstCut = Math.min(...cutAt);
const liveText = matrixText.slice(0, firstCut);
const closedText = matrixText.slice(firstCut);

const liveIds = new Set(liveText.match(ID_RE) ?? []);
const retiredIds = new Set((closedText.match(ID_RE) ?? []).filter((id) => !liveIds.has(id)));

// The source corpus that may carry a verification ID. The matrix itself is
// excluded so it cannot satisfy its own claims.
const sourceFiles = [
  ...walk(resolve(repoRoot, 'rust/src'), ['.rs']),
  ...walk(resolve(repoRoot, 'rust/tests'), ['.rs']),
  ...walk(resolve(repoRoot, 'rust/wasm-tests'), ['.rs']),
  ...walk(resolve(repoRoot, 'src'), ['.ts', '.tsx']),
  resolve(repoRoot, 'vitest.config.ts'),
].filter((f) => existsSync(f));

const idsInSource = new Map();
for (const file of sourceFiles) {
  const rel = file.slice(repoRoot.length + 1);
  for (const id of readFileSync(file, 'utf8').match(ID_RE) ?? []) {
    if (!idsInSource.has(id)) idsInSource.set(id, []);
    if (!idsInSource.get(id).includes(rel)) idsInSource.get(id).push(rel);
  }
}

// 1. Every ID the source names has a row.
const undocumented = [...idsInSource.keys()].filter((id) => !liveIds.has(id));
if (undocumented.length) {
  fail(`${undocumented.length} verification ID(s) in the source have no row in ${matrixPath}:`,
    undocumented.map((id) => `${id} — ${idsInSource.get(id).join(', ')}`));
}

// 2. Every live row still has a subject.
const orphaned = [...liveIds].filter((id) => !idsInSource.has(id));
if (orphaned.length) {
  fail(`${orphaned.length} ID(s) in ${matrixPath} no longer appear anywhere in the source:`,
    orphaned.map((id) => `${id} — move it to "${RETIRED_HEADING}" or restore its suite`));
}

// 3. A retired ID is really gone.
const resurrected = [...retiredIds].filter((id) => idsInSource.has(id));
if (resurrected.length) {
  fail(`${resurrected.length} ID(s) are listed as retired or unassigned but appear in the source:`,
    resurrected.map((id) => `${id} — ${idsInSource.get(id).join(', ')}`));
}

// 4. Every path the matrix names exists.
const missingPaths = [];
for (const [, path] of matrixText.matchAll(PATH_RE)) {
  if (!path.endsWith('/') && !PATH_EXT.test(path)) continue;
  if (!existsSync(resolve(repoRoot, path.replace(/\/$/, '')))) missingPaths.push(path);
}
if (missingPaths.length) {
  fail(`${missingPaths.length} path(s) named in ${matrixPath} do not exist:`, [...new Set(missingPaths)]);
}

// 5. Every file that cites the matrix can still reach it.
const citing = sourceFiles.filter((f) => readFileSync(f, 'utf8').includes('TRACEABILITY_MATRIX.md'));
const badCitation = citing.filter((f) => !readFileSync(f, 'utf8').includes(matrixPath));
if (badCitation.length) {
  fail(`${badCitation.length} file(s) cite TRACEABILITY_MATRIX.md at a path other than ${matrixPath}:`,
    badCitation.map((f) => f.slice(repoRoot.length + 1)));
}

report.done(
  `${liveIds.size} live ID(s) across ${citing.length} citing file(s), ` +
  `${retiredIds.size} retired or unassigned, all reachable.`
);
