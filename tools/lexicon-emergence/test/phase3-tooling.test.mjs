// Phase 3's tooling: the selection rule that ranks reinvented Words first,
// and sealing a graded generation's solutions out of the tree.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { nextLexicon } from '../lib/evolve.mjs';

// One inherited Word every subject leans on, and one Word two subjects each
// wrote for themselves, used once apiece.
const words = new Map([
  ['G0A.SUM', { name: 'G0A.SUM', body: '0 [ ADD ] FOLD', note: 'sum', origin: 'lexicon' }],
  ['G1B.CAP', { name: 'G1B.CAP', body: "CHARS 'C' BIND C 0 GET UPPER C 1 DROP JOIN 2 COLLECT JOIN", note: 'cap', definedBy: 'G1B' }],
  ['G1C.CAP', { name: 'G1C.CAP', body: "CHARS 'C' BIND C 0 GET UPPER C 1 DROP JOIN 2 COLLECT JOIN", note: 'cap', definedBy: 'G1C' }],
]);
const classes = new Map([
  ['G0A.SUM', { name: 'G0A.SUM', classId: 'G0A.SUM', link: 'self' }],
  ['G1B.CAP', { name: 'G1B.CAP', classId: 'G1B.CAP', link: 'self' }],
  ['G1C.CAP', { name: 'G1C.CAP', classId: 'G1B.CAP', link: 'd0' }],
]);
const results = (own) => [
  ...Array.from({ length: 5 }, (_, i) => ({ task: `t.sum${i}`, correct: true, uses: ['G0A.SUM'] })),
  { task: 't.cap', correct: true, uses: [own] },
];
const graded = [
  { agent: 'G1B', definitions: [words.get('G1B.CAP')], results: results('G1B.CAP') },
  { agent: 'G1C', definitions: [words.get('G1C.CAP')], results: results('G1C.CAP') },
];

test('the uses rule keeps the inherited Word every subject leans on', () => {
  const next = nextLexicon(2, 1, graded, words, classes, 'uses');
  assert.deepEqual(next.entries.map((e) => e.name), ['G0A.SUM']);
  assert.equal(next.rule, 'uses');
});

test('the reinvention rule ranks the Word two subjects wrote themselves first', () => {
  const next = nextLexicon(2, 1, graded, words, classes, 'reinvention');
  assert.equal(next.entries.length, 1);
  assert.match(next.entries[0].name, /CAP$/);
});

test('an unknown rule is refused', () => {
  assert.throws(() => nextLexicon(2, 1, graded, words, classes, 'popularity'), /unknown selection rule/);
});

test('seal moves a generation out of the tree and unseal brings it back', () => {
  const cli = fileURLToPath(new URL('../cli.mjs', import.meta.url));
  const run = `seal-test-${process.pid}`;
  const gen = fileURLToPath(new URL(`../runs/${run}/bottleneck/gen0/`, import.meta.url));
  const stash = mkdtempSync(join(tmpdir(), 'lexicon-stash-'));
  try {
    mkdirSync(join(gen, 'submissions'), { recursive: true });
    writeFileSync(join(gen, 'submissions', 'G0A.json'), '{}');
    writeFileSync(join(gen, 'lexicon.json'), '{"entries":[]}');
    const env = { ...process.env, LEXICON_STASH: stash };
    execFileSync('node', [cli, 'seal', run, 'bottleneck', '0'], { env });
    assert.ok(!existsSync(join(gen, 'submissions')), 'solutions leave the tree');
    assert.ok(existsSync(join(gen, 'lexicon.json')), 'the lexicon stays');
    assert.ok(existsSync(join(stash, run, 'bottleneck', 'gen0', 'submissions', 'G0A.json')));
    execFileSync('node', [cli, 'unseal', run], { env });
    assert.ok(existsSync(join(gen, 'submissions', 'G0A.json')), 'and come back');
    assert.ok(!existsSync(join(stash, run)));
  } finally {
    rmSync(fileURLToPath(new URL(`../runs/${run}/`, import.meta.url)), { recursive: true, force: true });
    rmSync(stash, { recursive: true, force: true });
  }
});
