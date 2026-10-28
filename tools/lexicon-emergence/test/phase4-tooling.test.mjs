// Phase 4's tooling: training families that rotate by generation beside a
// fixed held-out set, the selection rule that ranks Words by how many tasks
// reach them, and the report's split between training and held-out tasks.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { heldOutSplit } from '../lib/analyze.mjs';
import { nextLexicon } from '../lib/evolve.mjs';
import { allFamilyNames, familyNames } from '../lib/rotation.mjs';

const design = {
  rotation: { train: [['text', 'orders'], ['prose', 'runs']], heldOut: ['messy', 'ledger'], fixed: ['fixed'] },
};

test('without rotation every generation solves the families and none is held out', () => {
  const names = familyNames({ families: ['runs', 'number'] }, 'uses', 2);
  assert.deepEqual(names, { train: ['runs', 'number'], heldOut: [], all: ['runs', 'number'] });
  assert.deepEqual(familyNames({}, 'solo', 0).train, ['stats', 'text']);
});

test('training families alternate by generation, and the held-out ones come every time', () => {
  assert.deepEqual(familyNames(design, 'uses', 0).train, ['text', 'orders']);
  assert.deepEqual(familyNames(design, 'uses', 1).train, ['prose', 'runs']);
  assert.deepEqual(familyNames(design, 'uses', '2').train, ['text', 'orders']);
  const { all, heldOut } = familyNames(design, 'uses', 1);
  assert.deepEqual(heldOut, ['messy', 'ledger']);
  // By name, so the held-out families are not always read last.
  assert.deepEqual(all, ['ledger', 'messy', 'prose', 'runs']);
});

test('a fixed condition always trains on the first set', () => {
  for (const g of [0, 1, 2, 3]) assert.deepEqual(familyNames(design, 'fixed', g).train, ['text', 'orders']);
});

test('a family both trained on and held out, or an empty set, is refused', () => {
  assert.throws(() => familyNames({ rotation: { train: [['messy']], heldOut: ['messy'] } }, 'uses', 0), /both trained on and held out/);
  assert.throws(() => familyNames({ rotation: { train: [[]] } }, 'uses', 0), /non-empty/);
});

test('analysis reads every family a rotation names', () => {
  assert.deepEqual(allFamilyNames(design), ['ledger', 'messy', 'orders', 'prose', 'runs', 'text']);
});

// A Word that solves one task outright, called by every subject, and a Word
// one subject reached from two different tasks.
const words = new Map([
  ['G0A.WHOLE', { name: 'G0A.WHOLE', body: "' ' TOKENIZE LENGTH", note: 'one task', origin: 'lexicon' }],
  ['G1B.WORDS', { name: 'G1B.WORDS', body: "' ' TOKENIZE", note: 'words', definedBy: 'G1B' }],
  ['G1C.HELD', { name: 'G1C.HELD', body: 'LENGTH', note: 'held-out only', definedBy: 'G1C' }],
]);
const classes = new Map([...words.keys()].map((name) => [name, { name, classId: name, link: 'self' }]));
const graded = [
  {
    agent: 'G1B',
    definitions: [words.get('G1B.WORDS')],
    results: [
      { task: 't.count', correct: true, uses: ['G0A.WHOLE'] },
      { task: 't.first', correct: true, uses: ['G1B.WORDS'] },
      { task: 't.last', correct: true, uses: ['G1B.WORDS'] },
    ],
  },
  {
    agent: 'G1C',
    definitions: [words.get('G1C.HELD')],
    results: [
      { task: 't.count', correct: true, uses: ['G0A.WHOLE'] },
      { task: 'h.a', heldOut: true, correct: true, uses: ['G1C.HELD'] },
      { task: 'h.b', heldOut: true, correct: true, uses: ['G1C.HELD'] },
      { task: 'h.c', heldOut: true, correct: true, uses: ['G1C.HELD'] },
    ],
  },
  { agent: 'G1D', definitions: [], results: [{ task: 't.count', correct: true, uses: ['G0A.WHOLE'] }] },
];

test('the uses rule keeps the whole-task Word every subject calls', () => {
  assert.deepEqual(nextLexicon(2, 1, graded, words, classes, 'uses').entries.map((e) => e.name), ['G0A.WHOLE']);
});

test('the breadth rule ranks the Word reached from more tasks first', () => {
  const next = nextLexicon(2, 1, graded, words, classes, 'breadth');
  assert.deepEqual(next.entries.map((e) => e.name), ['G1B.WORDS']);
  assert.equal(next.entries[0].tasks, 2);
  assert.equal(next.rule, 'breadth');
});

test('held-out results never put a Word in the next lexicon', () => {
  for (const rule of ['uses', 'reinvention', 'breadth']) {
    const names = nextLexicon(2, Infinity, graded, words, classes, rule).entries.map((e) => e.name);
    assert.ok(!names.includes('G1C.HELD'), `${rule}: ${names}`);
  }
});

test('the report splits training from held-out results', () => {
  const lexicon = { entries: [{ name: 'G0A.WHOLE', body: "' ' TOKENIZE LENGTH" }] };
  const sized = graded.map((g) => ({
    ...g,
    results: g.results.map((r) => ({ ...r, tokens: 1, expandedTokens: r.uses.includes('G0A.WHOLE') ? 4 : 2 })),
  }));
  const split = heldOutSplit({ lexicon, graded: sized });
  assert.deepEqual(split.train, { correct: '5/5', compression: 0.313, reaching: '3/5', tasksReaching: 1 });
  assert.deepEqual(split.heldOut, { correct: '3/3', compression: 0.5, reaching: '0/3', tasksReaching: 0 });
  assert.equal(heldOutSplit({ lexicon, graded: [graded[0]] }), null);
});
