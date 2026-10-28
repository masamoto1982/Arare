// The rule of scripts/lib/spec-version.mjs, exercised without git: each case
// states a base, a change, and whether the gate must refuse it.
//
//   npm run test:scripts
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { canonical, parseChangelog, versionProblems } from './lib/spec-version.mjs';

const D = { definitionalDigest: 'd0', normativeDigest: 'n0' };
const lockAt = (version, digests = D) => ({ specificationVersion: version, ...digests });
const log = (...sections) => sections.map(([version, ...kinds]) =>
  `## ${version}\n\n${kinds.map((kind) => `- **${kind}** LANG.X — something`).join('\n')}\n`).join('\n');
const base = (version, text) => ({ lock: lockAt(version), changelog: parseChangelog(text) });
const problems = ({ version, digests = D, text, baseVersion = '1.0.0-beta.2', baseText }) =>
  versionProblems({
    lock: lockAt(version, digests),
    digests,
    changelog: parseChangelog(text),
    base: baseText === undefined ? null : base(baseVersion, baseText),
  });

test('an unchanged specification passes', () => {
  const text = log(['1.0.0-beta.2', 'breaking']);
  assert.deepEqual(problems({ version: '1.0.0-beta.2', text, baseText: text }), []);
});

test('a definitional change under the same version is refused', () => {
  const text = log(['1.0.0-beta.2', 'breaking', 'compatible']);
  const result = problems({
    version: '1.0.0-beta.2',
    digests: { ...D, definitionalDigest: 'd1' },
    text,
    baseText: log(['1.0.0-beta.2', 'breaking']),
  });
  assert.ok(result.some((p) => p.includes('specificationVersion did not')), result.join('\n'));
});

test('a definitional change with a new version and a new section passes', () => {
  const result = problems({
    version: '1.0.0-beta.3',
    digests: { ...D, definitionalDigest: 'd1' },
    text: log(['1.0.0-beta.3', 'breaking'], ['1.0.0-beta.2', 'breaking']),
    baseText: log(['1.0.0-beta.2', 'breaking']),
  });
  assert.deepEqual(result, []);
});

test('a prose change needs an entry, and an editorial one is enough', () => {
  const baseText = log(['1.0.0-beta.2', 'breaking']);
  const changed = { ...D, normativeDigest: 'n1' };
  const silent = problems({ version: '1.0.0-beta.2', digests: changed, text: baseText, baseText });
  assert.ok(silent.some((p) => p.includes('gained no entry')), silent.join('\n'));
  const noted = problems({
    version: '1.0.0-beta.2',
    digests: changed,
    text: log(['1.0.0-beta.2', 'breaking', 'editorial']),
    baseText,
  });
  assert.deepEqual(noted, []);
});

test('a breaking entry added to a released version is refused', () => {
  const result = problems({
    version: '1.0.0-beta.2',
    digests: { ...D, normativeDigest: 'n1' },
    text: log(['1.0.0-beta.2', 'breaking', 'breaking']),
    baseText: log(['1.0.0-beta.2', 'breaking']),
  });
  assert.ok(result.some((p) => p.includes('raise specificationVersion')), result.join('\n'));
});

test('a stale lock is refused even with no base', () => {
  const result = versionProblems({
    lock: lockAt('1.0.0-beta.2'),
    digests: { ...D, definitionalDigest: 'd1' },
    changelog: parseChangelog(log(['1.0.0-beta.2', 'breaking'])),
    base: null,
  });
  assert.ok(result.some((p) => p.includes('no longer has')), result.join('\n'));
});

test('the newest changelog section must be the locked version', () => {
  const result = problems({ version: '1.0.0-beta.3', text: log(['1.0.0-beta.2', 'breaking']) });
  assert.ok(result.some((p) => p.includes("newest section")), result.join('\n'));
});

test('rewording a Word summary does not move the definitional digest', () => {
  const a = { entries: [{ name: 'ADD', documentation: { summary: 'old' }, errorWhen: ['nonNumeric'] }] };
  const b = { entries: [{ name: 'ADD', documentation: { summary: 'new' }, errorWhen: ['nonNumeric'] }] };
  const c = { entries: [{ name: 'ADD', documentation: { summary: 'old' }, errorWhen: [] }] };
  assert.equal(JSON.stringify(canonical(a)), JSON.stringify(canonical(b)));
  assert.notEqual(JSON.stringify(canonical(a)), JSON.stringify(canonical(c)));
});
