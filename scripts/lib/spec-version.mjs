// The specification's version lock: two digests over spec/ and the rule that
// ties them to `specificationVersion`.
//
// spec/README.md and the Status block of spec/language-semantics.md promise
// that a breaking change raises the specification version. Until this lock
// existed nothing held that promise: the version stayed 1.0.0-beta.1 through
// total division, the wheel of fractions, two truth domains and a dozen other
// changes to what programs answer. The lock records what the version was
// computed over, so a gate can see the specification move under a version
// that did not.
//
// Two digests, because the specification has two kinds of source:
//
//   definitionalDigest — the machine-read sources (grammar, vocabulary,
//     outcomes, termination, identity, retired names, host protocol), with
//     their prose keys removed. A change here changes what some program does
//     or what a host receives, so it always takes a new version.
//   normativeDigest — the two prose sources, language-semantics.md and
//     gui-semantics.md, whole. A change here may be editorial, so it takes a
//     changelog entry whose kind the author chooses; a `breaking` entry still
//     takes a new version.

import { createHash } from 'node:crypto';
import { readJson, readText } from './common.mjs';

export const LOCK_PATH = 'spec/version-lock.json';
export const CHANGELOG_PATH = 'spec/CHANGELOG.md';

export const DEFINITIONAL_SOURCES = [
  'spec/grammar.json',
  'spec/words.json',
  'spec/outcomes.json',
  'spec/termination.json',
  'spec/identity.json',
  'spec/retired-words.json',
  'spec/host-protocol.schema.json',
];

export const NORMATIVE_SOURCES = ['spec/language-semantics.md', 'spec/gui-semantics.md'];

/**
 * Keys whose value is prose about the definition rather than the definition:
 * a Word's `documentation`, a schema's `description`, a witness's `note`, an
 * argument's `why`. Rewording one is not a change to the language, so it is
 * removed before the digest is taken. Every other key counts.
 */
export const PROSE_KEYS = new Set([
  '$comment',
  'description',
  'documentation',
  'note',
  'why',
  'whyItMatters',
  'history',
  'consequence',
]);

/** A JSON value with prose keys removed and object keys in sorted order. */
export function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value === null || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.keys(value)
      .filter((key) => !PROSE_KEYS.has(key))
      .sort()
      .map((key) => [key, canonical(value[key])]),
  );
}

const sha256 = (text) => createHash('sha256').update(text).digest('hex');

/** The digests of the specification as it stands in the working tree. */
export function computeDigests(read = { json: readJson, text: readText }) {
  const definitional = DEFINITIONAL_SOURCES.map(
    (path) => `${path}\n${JSON.stringify(canonical(read.json(path)))}\n`,
  ).join('');
  const normative = NORMATIVE_SOURCES.map((path) => `${path}\n${read.text(path)}\n`).join('');
  return { definitionalDigest: sha256(definitional), normativeDigest: sha256(normative) };
}

export const ENTRY_KINDS = ['breaking', 'compatible', 'editorial'];

/**
 * spec/CHANGELOG.md as data: one section per `## <version>` heading, newest
 * first, each with the entries written as `- **<kind>** …` bullets. Text that
 * is not a heading or an entry bullet (a preamble, an entry's continuation
 * lines) is carried by the entry or ignored, so the file stays prose to read.
 */
export function parseChangelog(text) {
  const sections = [];
  for (const line of text.split('\n')) {
    const heading = line.match(/^## (\S+)\s*$/);
    if (heading) {
      sections.push({ version: heading[1], entries: [] });
      continue;
    }
    const entry = line.match(/^- \*\*([a-z]+)\*\*/);
    if (entry && sections.length > 0) sections.at(-1).entries.push({ kind: entry[1], line });
  }
  return sections;
}

/** How many entries of each kind a section holds. */
export function countKinds(section) {
  const counts = Object.fromEntries(ENTRY_KINDS.map((kind) => [kind, 0]));
  for (const { kind } of section?.entries ?? []) if (kind in counts) counts[kind] += 1;
  return counts;
}

/**
 * The rule the lock exists for, as a pure function so it can be tested
 * without git: given the lock and changelog on the base and in the working
 * tree, and the digests the working tree computes, the problems found.
 * `base` is null when there is no base to compare with (the lock's first
 * commit, or a run with no base ref), and then only the tree's own
 * consistency is checked.
 */
export function versionProblems({ lock, digests, changelog, base }) {
  const problems = [];
  if (lock.definitionalDigest !== digests.definitionalDigest || lock.normativeDigest !== digests.normativeDigest) {
    problems.push(
      `${LOCK_PATH} records digests the specification no longer has; record the change in ${CHANGELOG_PATH}, ` +
        'raise specificationVersion if it is breaking or definitional, and run `node scripts/check-spec-version.mjs --update`',
    );
  }
  const top = changelog[0];
  if (top === undefined) {
    problems.push(`${CHANGELOG_PATH} has no \`## <version>\` section`);
    return problems;
  }
  if (top.version !== lock.specificationVersion) {
    problems.push(
      `${CHANGELOG_PATH}'s newest section is ${top.version}, but ${LOCK_PATH} says ${lock.specificationVersion}`,
    );
  }
  for (const { kind, line } of top.entries) {
    if (!ENTRY_KINDS.includes(kind)) problems.push(`${CHANGELOG_PATH}: unknown entry kind "${kind}" in: ${line}`);
  }
  if (top.entries.length === 0) problems.push(`${CHANGELOG_PATH}'s section ${top.version} has no entries`);
  if (base === null) return problems;

  const moved = base.lock.specificationVersion !== lock.specificationVersion;
  const definitionalChanged = base.lock.definitionalDigest !== digests.definitionalDigest;
  const normativeChanged = base.lock.normativeDigest !== digests.normativeDigest;
  const baseTop = base.changelog.find((section) => section.version === top.version);
  const before = countKinds(baseTop);
  const after = countKinds(top);
  const added = Object.fromEntries(ENTRY_KINDS.map((kind) => [kind, after[kind] - before[kind]]));
  const addedAny = ENTRY_KINDS.some((kind) => added[kind] > 0);

  if (moved && baseTop !== undefined) {
    problems.push(`specificationVersion moved to ${lock.specificationVersion}, which ${CHANGELOG_PATH} already had on the base`);
  }
  if (definitionalChanged && !moved) {
    problems.push(
      'the machine-read specification changed (definitionalDigest) and specificationVersion did not; ' +
        `raise it and open a new section in ${CHANGELOG_PATH}`,
    );
  }
  if (!moved && added.breaking > 0) {
    problems.push(`a breaking entry was added to ${top.version}, which the base already released; raise specificationVersion`);
  }
  if ((definitionalChanged || normativeChanged) && !addedAny) {
    problems.push(`the specification changed and ${CHANGELOG_PATH} gained no entry for it`);
  }
  return problems;
}
