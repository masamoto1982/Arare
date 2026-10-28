// Phase 5's tooling: the selection rule that ranks Words by how many task
// families reach them, one place per class in every rule, a cap on how many
// Words a subject may define, and the held-out table that sets what a subject
// shares with its own training tasks beside what it inherits.
import assert from 'node:assert/strict';
import { after, before, test } from 'node:test';
import { connect } from '../lib/ajisai.mjs';
import { heldOutFamilies } from '../lib/analyze.mjs';
import { nextLexicon } from '../lib/evolve.mjs';
import { grade } from '../lib/grade.mjs';
import { subjectPrompt } from '../lib/prompt.mjs';

const self = (names) => new Map(names.map((name) => [name, { name, classId: name, link: 'self' }]));

// A Word that reaches four tasks of one family, and one that reaches two
// tasks in two families.
const wide = new Map([
  ['G1C.CODE', { name: 'G1C.CODE', body: 'CHARS', note: 'one family', definedBy: 'G1C' }],
  ['G1A.ARGMAX', { name: 'G1A.ARGMAX', body: '-1 MUL ORDER 0 GET', note: 'two families', definedBy: 'G1A' }],
]);
const wideGraded = [
  {
    agent: 'G1C',
    definitions: [...wide.values()],
    results: [
      ...['a', 'b', 'c', 'd'].map((t) => ({ task: `brackets.${t}`, correct: true, uses: ['G1C.CODE'] })),
      { task: 'ledger.top', correct: true, uses: ['G1A.ARGMAX'] },
      { task: 'sensor.peak', correct: true, uses: ['G1A.ARGMAX'] },
    ],
  },
];

test('the breadth rule ranks the Word reaching the most tasks first', () => {
  const next = nextLexicon(2, 1, wideGraded, wide, self(wide.keys()), 'breadth');
  assert.deepEqual(next.entries.map((e) => e.name), ['G1C.CODE']);
});

test('the families rule ranks the Word reaching the most families first, and records the count', () => {
  const next = nextLexicon(2, 1, wideGraded, wide, self(wide.keys()), 'families');
  assert.deepEqual(next.entries.map((e) => e.name), ['G1A.ARGMAX']);
  assert.equal(next.entries[0].families, 2);
  assert.equal(next.entries[0].tasks, 2);
});

// Phase 4's duplicate mean: a mean chosen first, a deviation whose own mean
// joined that mean's class at D1 only, and a second deviation that calls the
// chosen mean.
const means = new Map([
  ['G1A.MEAN', { name: 'G1A.MEAN', body: "'M' BIND M 0 [ ADD ] FOLD M LENGTH DIV", note: 'mean', definedBy: 'G1A' }],
  ['G1B.MEAN', { name: 'G1B.MEAN', body: "'V' BIND V 0 [ ADD ] FOLD V LENGTH DIV", note: 'mean', definedBy: 'G1B' }],
  ['G1B.DEV', { name: 'G1B.DEV', body: "'D' BIND D D G1B.MEAN SUB", note: 'deviations', definedBy: 'G1B' }],
  ['G1D.DEV', { name: 'G1D.DEV', body: "'E' BIND E E G1A.MEAN SUB", note: 'deviations', definedBy: 'G1D' }],
]);
const meanClasses = new Map([
  ['G1A.MEAN', { name: 'G1A.MEAN', classId: 'G1A.MEAN', link: 'self' }],
  ['G1B.MEAN', { name: 'G1B.MEAN', classId: 'G1A.MEAN', link: 'd1' }],
  ['G1B.DEV', { name: 'G1B.DEV', classId: 'G1B.DEV', link: 'self' }],
  ['G1D.DEV', { name: 'G1D.DEV', classId: 'G1B.DEV', link: 'd1' }],
]);
const meanGraded = [
  {
    agent: 'G1A',
    definitions: [means.get('G1A.MEAN')],
    results: ['a', 'b', 'c'].map((t) => ({ task: `stats.${t}`, correct: true, uses: ['G1A.MEAN'] })),
  },
  {
    agent: 'G1B',
    definitions: [means.get('G1B.MEAN'), means.get('G1B.DEV')],
    results: [
      { task: 'stats.dev', correct: true, uses: ['G1B.DEV', 'G1B.MEAN'] },
      { task: 'stats.var', correct: true, uses: ['G1B.DEV', 'G1B.MEAN'] },
    ],
  },
  { agent: 'G1D', definitions: [means.get('G1D.DEV')], results: [{ task: 'stats.dev', correct: true, uses: ['G1D.DEV', 'G1A.MEAN'] }] },
];

test('no rule gives a class two places: a member that would carry a second mean gives way to one that would not', () => {
  for (const rule of ['uses', 'reinvention', 'breadth', 'families']) {
    const names = nextLexicon(2, 8, meanGraded, means, meanClasses, rule).entries.map((e) => e.name);
    assert.deepEqual(names.sort(), ['G1A.MEAN', 'G1D.DEV'], rule);
  }
});

test('a class none of whose members fits without a second place takes only the capacity left over', () => {
  const only = new Map([...means].filter(([n]) => n !== 'G1D.DEV'));
  const classes = new Map([...meanClasses].filter(([n]) => n !== 'G1D.DEV'));
  // A third class, ranked below the deviation, that the deviation's duplicate mean must not displace.
  only.set('G1A.MAX', { name: 'G1A.MAX', body: '[ MAX ] FOLD', note: 'max', definedBy: 'G1A' });
  classes.set('G1A.MAX', { name: 'G1A.MAX', classId: 'G1A.MAX', link: 'self' });
  const graded = [meanGraded[0], meanGraded[1], { agent: 'G1C', definitions: [], results: [{ task: 'stats.max', correct: true, uses: ['G1A.MAX'] }] }];
  const pick = (k) => nextLexicon(2, k, graded, only, classes, 'uses').entries.map((e) => e.name);
  assert.deepEqual(pick(2), ['G1A.MEAN', 'G1A.MAX']);
  assert.deepEqual(pick(4), ['G1A.MEAN', 'G1A.MAX', 'G1B.MEAN', 'G1B.DEV']);
  assert.deepEqual(pick(Infinity), pick(4));
});

test('the prompt forbids or caps definitions as the budget says', () => {
  const meta = { run: 'r', condition: 'probe', generation: 1, agent: 'P1A' };
  const lexicon = { entries: [{ name: 'G0A.SUM', body: '0 [ ADD ] FOLD', note: 'sum' }] };
  const families = [{ inputKind: 'a Vector', inputs: ['[ 1 2 ]'], tasks: [{ id: 'stats.sum', prompt: 'the sum' }] }];
  const none = subjectPrompt(meta, lexicon, families, 'cli', { callBudget: 0, definitionBudget: 0 });
  assert.match(none, /may not define Words of your own/);
  assert.match(none, /and the inherited Words below/);
  assert.match(none, /"definitions": \[\],/);
  assert.doesNotMatch(none, /You may define your own Words/);
  const bare = subjectPrompt(meta, { entries: [] }, families, 'cli', { callBudget: 0, definitionBudget: 0 });
  assert.match(bare, /with Core Words\n {2}only;/);
  const capped = subjectPrompt(meta, lexicon, families, 'cli', { callBudget: 0, definitionBudget: 3 });
  assert.match(capped, /Only the first 3 definitions in your list count/);
  const open = subjectPrompt(meta, lexicon, families, 'cli', { callBudget: 0 });
  assert.doesNotMatch(open, /Only the first|may not define/);
});

let ajisai;
before(async () => {
  ajisai = await connect();
});
after(async () => {
  await ajisai?.close();
});

test('definitions past the budget are dropped before grading, and named', async () => {
  const families = [
    {
      family: 'stats',
      inputs: ['[ 1 2 3 ]', '[ 4 ]'],
      tasks: [
        { id: 'stats.sum', reference: '0 [ ADD ] FOLD' },
        { id: 'stats.len', reference: 'LENGTH' },
      ],
    },
  ];
  const submission = {
    agent: 'P1A',
    definitions: [{ name: 'P1A.SUM', body: '0 [ ADD ] FOLD', note: 'sum' }],
    solutions: { 'stats.sum': 'P1A.SUM', 'stats.len': 'LENGTH' },
  };
  const free = await grade(ajisai, submission, { entries: [] }, families);
  assert.deepEqual(free.results.map((r) => r.correct), [true, true]);
  assert.equal(free.definitionsDropped, undefined);
  const banned = await grade(ajisai, submission, { entries: [] }, families, [], { definitionBudget: 0 });
  assert.deepEqual(banned.results.map((r) => r.correct), [false, true]);
  assert.deepEqual(banned.definitions, []);
  assert.deepEqual(banned.definitionsDropped, ['P1A.SUM']);
});

test('the held-out table counts what a subject shares with its own training beside what it inherits', () => {
  const gen = {
    lexicon: { entries: [{ name: 'G0A.SUM' }] },
    graded: [
      {
        agent: 'U1A',
        results: [
          { task: 'stats.sum', correct: true, uses: ['G0A.SUM'] },
          { task: 'stats.mean', correct: true, uses: ['U1A.MEAN'] },
          { task: 'ledger.total', heldOut: true, correct: true, uses: ['G0A.SUM'] },
          { task: 'ledger.avg', heldOut: true, correct: true, uses: ['U1A.MEAN'] },
          { task: 'series.median', heldOut: true, correct: true, uses: ['U1A.MID'] },
          { task: 'series.mode', heldOut: true, correct: false, uses: [] },
        ],
      },
    ],
  };
  assert.deepEqual(heldOutFamilies(gen), [
    { family: 'ledger', correct: 2, attempted: 2, shared: 2, inherited: 1 },
    { family: 'series', correct: 1, attempted: 2, shared: 0, inherited: 0 },
  ]);
  assert.equal(heldOutFamilies({ lexicon: { entries: [] }, graded: [{ results: [{ task: 'stats.sum', correct: true, uses: [] }] }] }), null);
});
