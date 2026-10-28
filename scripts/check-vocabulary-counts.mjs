// LANG.AUTHORITY.IDENTITY — the counts half of the clause.
//
// The Specification states how many Words the vocabulary has, and says
// `docs/word-manifest.json` is the count of record. Prose that restates a
// count is a copy of that record, and nothing regenerated the copies: when
// BOTH retired, the manifest went to 77 and 49 while the Specification kept
// saying 78 and 50, and the Japanese Reference kept one "78" beside three
// "77"s. Every other gate passed, because every other gate reads the registry
// and none reads the prose.
//
// This gate reads the prose. Each pattern below is one way a reading surface
// states a count, tied to the manifest count it restates; a number written
// there that differs from the record fails the check. The sweep patterns then
// catch a count stated in a phrasing no explicit pattern names yet, so a new
// sentence cannot carry a stale number past the gate either.
//
// Generated surfaces (SKILL.md, docs/word-reference.md, the marked word index
// in the Reference) take their counts from the manifest and are gated by their
// own generators; the MCP compute description and quickstart are gated by
// tools/mcp-server/tool-description.test.js. This gate covers the hand-written
// rest.

import { readJson, readText, reporter } from './lib/common.mjs';

const counts = readJson('docs/word-manifest.json').counts;
const RECORD = {
  canonical: counts.canonicalWords,
  kernel: counts.semanticKernelWords,
  standard: counts.standardWords,
};

const SURFACES = [
  'README.md',
  'spec/language-semantics.md',
  'spec/gui-semantics.md',
  'public/docs/index.html',
  'public/docs/en/index.html',
  'public/docs/ja/index.html',
  'tools/mcp-server/README.md',
];

// [pattern, which count it restates]. Each pattern's first group is the number.
const STATED = [
  // the Specification's count table and dictionary clause
  [/<td>(\d+)<\/td><td>Canonical Words/g, 'canonical'],
  [/<td>(\d+)<\/td><td>Semantic Kernel Words/g, 'kernel'],
  [/<td>(\d+)<\/td><td>Standard Words/g, 'standard'],
  [/Words, within the (\d+)/g, 'canonical'],
  // the Reference, English
  [/These (\d+) Words divide/g, 'canonical'],
  [/(\d+)-Word Semantic Kernel/g, 'kernel'],
  [/(\d+) Standard Words/g, 'standard'],
  [/All (\d+) are ordinary Core Words/g, 'canonical'],
  [/vocabulary is (\d+) Words/g, 'canonical'],
  [/The (\d+) Words of the canonical vocabulary/g, 'canonical'],
  // the Reference, Japanese
  [/この(\d+)個のワード/g, 'canonical'],
  [/(\d+)語のSemantic Kernel/g, 'kernel'],
  [/(\d+)語のStandard/g, 'standard'],
  [/(\d+)個すべて/g, 'canonical'],
  [/語彙は(\d+)個のワード/g, 'canonical'],
  // the sweep: any count of the whole vocabulary, however the sentence runs
  [/(\d+) (?:canonical|Core) Words/g, 'canonical'],
  [/(\d+)\s+(?:Semantic\s+)?Kernel\s+Words/g, 'kernel'],
  [/(\d+)個のCoreワード/g, 'canonical'],
];

const report = reporter('vocabulary-counts');
let stated = 0;

for (const path of SURFACES) {
  const text = readText(path);
  for (const [pattern, which] of STATED) {
    for (const match of text.matchAll(pattern)) {
      stated += 1;
      const written = Number(match[1]);
      if (written === RECORD[which]) continue;
      const line = text.slice(0, match.index).split('\n').length;
      report.fail(
        `${path}:${line} states ${written} (${which}) in "${match[0]}", but docs/word-manifest.json records ${RECORD[which]}`,
      );
    }
  }
}

if (stated === 0) report.fail('no count was found on any surface; the patterns have drifted from the prose');
if (report.failed) {
  console.error('[vocabulary-counts] Restate the count from docs/word-manifest.json (`counts`), the count of record.');
}
report.done(
  `${stated} stated counts on ${SURFACES.length} surfaces agree with the manifest (${RECORD.canonical} Words: ${RECORD.kernel} Kernel, ${RECORD.standard} Standard).`,
);
