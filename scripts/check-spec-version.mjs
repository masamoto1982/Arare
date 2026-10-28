#!/usr/bin/env node
// Gate: the specification version moves when the specification does.
//
// Reads spec/version-lock.json and spec/CHANGELOG.md, recomputes the lock's
// digests, and applies scripts/lib/spec-version.mjs's rule against a base
// ref. Also holds every surface that states the version to the lock's value.
//
// Usage:
//   node scripts/check-spec-version.mjs [--base <git-ref>]   # check
//   node scripts/check-spec-version.mjs --update             # rewrite the lock's digests
//
// `--base` is the commit the change is measured against (CI passes the pull
// request's base, or the push's previous head). With no base, or a base that
// predates the lock, only the working tree's own consistency is checked.
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { readJson, readText, repoRoot, reporter } from './lib/common.mjs';
import {
  CHANGELOG_PATH,
  LOCK_PATH,
  computeDigests,
  parseChangelog,
  versionProblems,
} from './lib/spec-version.mjs';

const args = process.argv.slice(2);
const report = reporter('spec-version', { countSummary: true });
const lock = readJson(LOCK_PATH);
const digests = computeDigests();

if (args.includes('--update')) {
  const next = { ...lock, ...digests };
  writeFileSync(resolve(repoRoot, LOCK_PATH), `${JSON.stringify(next, null, 2)}\n`);
  console.log(`[spec-version] wrote ${LOCK_PATH} for ${lock.specificationVersion}`);
  process.exit(0);
}

function readAtBase(ref, path) {
  try {
    return execFileSync('git', ['show', `${ref}:${path}`], { cwd: repoRoot, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
  } catch {
    return null;
  }
}

const baseFlag = args.indexOf('--base');
const baseRef = baseFlag === -1 ? '' : (args[baseFlag + 1] ?? '');
let base = null;
if (baseRef !== '' && !/^0+$/.test(baseRef)) {
  const baseLock = readAtBase(baseRef, LOCK_PATH);
  if (baseLock !== null) {
    base = { lock: JSON.parse(baseLock), changelog: parseChangelog(readAtBase(baseRef, CHANGELOG_PATH) ?? '') };
  }
}

for (const problem of versionProblems({ lock, digests, changelog: parseChangelog(readText(CHANGELOG_PATH)), base })) {
  report.fail(problem);
}

// Every clause an entry names is a clause the specification has.
const clauses = new Set(readText('spec/language-semantics.md').match(/LANG\.[A-Z]+\.[A-Z]+/g));
for (const section of parseChangelog(readText(CHANGELOG_PATH))) {
  for (const { line } of section.entries) {
    for (const clause of line.match(/LANG\.[A-Z]+\.[A-Z]+/g) ?? []) {
      if (!clauses.has(clause)) report.fail(`${CHANGELOG_PATH} ${section.version} cites ${clause}, which spec/language-semantics.md does not define`);
    }
  }
}

// Every surface that states the specification version states this one.
const version = lock.specificationVersion;
const escaped = version.replaceAll('.', '\\.');
const surfaces = [
  ['spec/language-semantics.md', new RegExp(`Specification version: <strong>${escaped}</strong>`)],
  ['README.md', new RegExp(`<td>Specification version</td><td>${escaped}(?![0-9A-Za-z.])`)],
];
for (const [path, pattern] of surfaces) {
  if (!pattern.test(readText(path))) report.fail(`${path} does not state specification version ${version}`);
}

report.done(
  `specification ${version}; ${base === null ? 'no base lock to compare with' : `measured against ${baseRef.slice(0, 12)}`}.`,
);
