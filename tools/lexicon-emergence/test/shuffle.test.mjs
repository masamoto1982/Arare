// C-shuffled renames a lexicon so its names carry no meaning, and the renamed
// Words still compute what the originals did.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { connect } from '../lib/ajisai.mjs';
import { subjectPrompt } from '../lib/prompt.mjs';
import { nonsenseName, shuffleLexicon } from '../lib/shuffle.mjs';
import { prelude } from '../lib/source.mjs';

const lexicon = {
  generation: 1,
  entries: [
    { name: 'G0A.SUM', body: '0 [ ADD ] FOLD', note: 'sum of a Vector' },
    { name: 'G0B.MEAN', body: "'V' BIND V G0A.SUM V LENGTH DIV", note: 'arithmetic mean' },
  ],
};

test('every name is replaced, references follow it, and notes are gone', () => {
  const shuffled = shuffleLexicon(lexicon, 'X1');
  const [sum, mean] = shuffled.entries;
  assert.match(sum.name, /^X1\.[A-Z]{6}$/);
  assert.equal(sum.shuffledFrom, 'G0A.SUM');
  assert.equal(mean.body, `'V' BIND V ${sum.name} V LENGTH DIV`);
  assert.deepEqual(shuffled.entries.map((e) => e.note), ['', '']);
  assert.equal(nonsenseName('G0A.SUM', 'X1'), sum.name, 'the name is fixed by the original');
});

test('a name already under the prefix is kept, so a lexicon can be shuffled again', () => {
  const once = shuffleLexicon(lexicon, 'X1');
  const twice = shuffleLexicon(
    { entries: [...once.entries, { name: 'R1A.MAX', body: 'SORT -1 GET', note: 'largest' }] },
    'X1',
  );
  assert.deepEqual(twice.entries.slice(0, 2).map((e) => e.name), once.entries.map((e) => e.name));
  assert.notEqual(twice.entries[2].name, 'R1A.MAX');
});

test('the renamed Words answer as the originals do', async () => {
  const ajisai = await connect();
  try {
    const shuffled = shuffleLexicon(lexicon, 'X1');
    const before = await ajisai.compute(`${prelude(lexicon.entries)}\n[ 1 2 6 ] G0B.MEAN`);
    const after = await ajisai.compute(`${prelude(shuffled.entries)}\n[ 1 2 6 ] ${shuffled.entries[1].name}`);
    assert.deepEqual(after.stackDisplay, before.stackDisplay);
  } finally {
    await ajisai.close();
  }
});

test('the cli route tells a subject to name inherited Words with --words, not to paste them', () => {
  const families = [{ inputKind: 'a Vector', inputs: ['[ 1 2 ]'], tasks: [{ id: 't.one', prompt: 'Do it.' }] }];
  const text = subjectPrompt({ run: 'r', condition: 'bottleneck', generation: 1, agent: 'G1A' }, lexicon, families, 'cli');
  assert.match(text, /cli\.mjs compute r bottleneck 1 G1A/);
  assert.match(text, /--words G0A\.SUM,G0B\.MEAN/);
  assert.doesNotMatch(text, /' DEF\n/, 'no definition text to paste');
});
