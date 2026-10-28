// A generation's lexicon written as a phrasebook reaches subjects through
// `words`, and every result names the Words it defined by the digest the
// file recorded.
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { connect } from '../lib/ajisai.mjs';
import { preludeMatches, writeLexiconPhrasebook } from '../lib/lexicon-phrasebook.mjs';
import { subjectPrompt } from '../lib/prompt.mjs';

const lexicon = {
  entries: [
    // Listed before the Word it calls, so the writer must reorder.
    { name: 'G0A.MEAN', body: "'V' BIND V G0A.SUM V LENGTH DIV", note: 'mean' },
    { name: 'G0A.SUM', body: '0 [ ADD ] FOLD', note: 'sum' },
  ],
};

test('a written lexicon defines through words, digests matching', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'lexicon-'));
  const path = join(dir, 'phrasebook.ajisai');
  const plain = await connect();
  let served;
  try {
    const { digests, names } = await writeLexiconPhrasebook(plain, lexicon, path);
    assert.deepEqual(names, ['G0A.SUM', 'G0A.MEAN']);
    assert.match(readFileSync(path, 'utf8'), /#:digest G0A\.MEAN #[0-9a-f]{64}/);
    served = await connect({ phrasebook: path });
    const result = await served.compute('[ 2 4 9 ] G0A.MEAN', ['G0A.MEAN']);
    assert.equal(result.status, 'ok', JSON.stringify(result));
    assert.deepEqual(result.stackDisplay, ['5/1']);
    assert.deepEqual(result.prelude.map((p) => p.name), ['G0A.SUM', 'G0A.MEAN']);
    assert.ok(preludeMatches(result.prelude, digests));
  } finally {
    await plain.close();
    if (served) await served.close();
    rmSync(dir, { recursive: true, force: true });
  }
});

test('the harness prompt points at words, not at pasted definitions', () => {
  const meta = { run: 'r', condition: 'bottleneck', generation: 1, agent: 'G1A' };
  const prompt = subjectPrompt(meta, lexicon, [], 'harness');
  assert.match(prompt, /`words: \["G0A\.MEAN", "G0A\.SUM"\]`/);
  assert.doesNotMatch(prompt, /'G0A\.SUM' DEF/);
  const pasted = subjectPrompt(meta, lexicon, [], 'subagent');
  assert.match(pasted, /'G0A\.SUM' DEF/);
});
