#!/usr/bin/env node
// Generates the readable surfaces of the requirements registry:
//
//   - spec/REQUIREMENTS.md, every requirement in full;
//   - the generated block of docs/quality/TRACEABILITY_MATRIX.md, one row per
//     requirement with the theorems that verify it, grouped by file and
//     verification ID.
//
// spec/requirements.json is the source; neither output is edited by hand
// (CLAUDE.md, rule 5). `--check` fails when either is stale.

import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fatal, repoRoot, writeOrCheck } from './lib/common.mjs';
import { leanTheorems } from './lib/lean.mjs';
import { REGISTRY, classOf, requirements } from './lib/requirements.mjs';

const TAG = 'requirements-surfaces';
const MATRIX = 'docs/quality/TRACEABILITY_MATRIX.md';
const BEGIN = '<!-- BEGIN GENERATED: requirements (scripts/generate-requirements.mjs) -->';
const END = '<!-- END GENERATED: requirements -->';

const reqs = requirements();
const theorems = new Map(leanTheorems().map((t) => [t.name, t]));
const short = (name) => name.replace(/^Arare\./, '');
function verOf(file) {
  const ver = [...theorems.values()].find((t) => t.file === file)?.ids.find((id) => id.startsWith('AR-VER-'));
  if (!ver) fatal(TAG, `${file} names no AR-VER verification ID`);
  return ver;
}

/** The theorems a requirement cites, grouped by file: [{ file, ver, names }]. */
function proofGroups(r) {
  const groups = new Map();
  for (const name of r.verification?.lean ?? []) {
    const t = theorems.get(name);
    if (!t) fatal(TAG, `${r.id} cites ${name}, which formal/Arare/ does not declare (run check:requirements)`);
    if (!groups.has(t.file)) groups.set(t.file, { file: t.file, ver: verOf(t.file), names: [] });
    groups.get(t.file).names.push(short(name));
  }
  return [...groups.values()];
}

const CLASS_TITLE = {
  SYS: 'System requirements',
  HLR: 'High-level requirements',
  LLR: 'Low-level requirements',
  PERF: 'Performance requirements',
};

/** The AR-VER ID a test or harness file carries in its header. */
function verInFile(file) {
  const path = resolve(repoRoot, file);
  const ver = existsSync(path) ? readFileSync(path, 'utf8').match(/AR-VER-\d{3}/)?.[0] : null;
  if (!ver) fatal(TAG, `${file} carries no AR-VER ID`);
  return ver;
}

/** Tests and harnesses, as readable entries. */
function testEntries(r) {
  const tests = (r.verification?.tests ?? []).map(({ id, file }) => `\`${file}\` — **${id}**`);
  const byFile = new Map();
  for (const { name, file } of r.verification?.kani ?? []) {
    if (!byFile.has(file)) byFile.set(file, []);
    byFile.get(file).push(`\`${name}\``);
  }
  const kani = [...byFile].map(([file, names]) => `\`${file}\` — **${verInFile(file)}** (Kani): ${names.join(', ')}`);
  return [...tests, ...kani];
}

/** The low-level requirements that refine a requirement. */
const refinedBy = (id) => reqs.filter((r) => classOf(r.id) === 'LLR' && r.refines.includes(id)).map((r) => r.id);

// spec/REQUIREMENTS.md ------------------------------------------------------

const md = [
  '# Requirements',
  '',
  `Generated from \`${REGISTRY}\` by \`scripts/generate-requirements.mjs\`; do not edit by hand.`,
  'The identifier classes and the requirements standard are `docs/quality/STANDARDS.md`.',
  '',
];
for (const cls of ['SYS', 'HLR', 'LLR', 'PERF']) {
  const group = reqs.filter((r) => classOf(r.id) === cls);
  if (!group.length) continue;
  md.push(`## ${CLASS_TITLE[cls]}`, '');
  for (const r of group) {
    md.push(`### ${r.id} — ${r.title}`, '', r.statement, '');
    if (cls === 'SYS') {
      md.push(`- Source: ${r.source}`, '');
      continue;
    }
    md.push(`- Level: ${r.level}`);
    md.push(`- Refines: ${r.refines.join(', ')}`);
    if (r.derived) md.push(`- Derived: ${r.rationale}`);
    for (const g of proofGroups(r)) {
      md.push(`- Proved in \`${g.file}\` (${g.ver}): ${g.names.map((n) => `\`${n}\``).join(', ')}`);
    }
    if (r.implementation) md.push(`- Implemented in: ${r.implementation.map((f) => `\`${f}\``).join(', ')}`);
    if (cls === 'HLR' && refinedBy(r.id).length) md.push(`- Refined by: ${refinedBy(r.id).join(', ')}`);
    for (const entry of testEntries(r)) md.push(`- Tested in ${entry}`);
    if (r.verification?.formalExempt) md.push(`- Not proved formally: ${r.verification.formalExempt}`);
    if (r.verification?.planned) md.push(`- Planned: ${r.verification.planned}`);
    md.push('');
  }
}
const mdText = `${md.join('\n').replace(/\n+$/, '')}\n`;

// The matrix block ----------------------------------------------------------

const cell = (text) => text.replace(/\|/g, '\\|');
const rows = [
  '| Requirement | Statement | Implementation | Verification | QL |',
  '|---|---|---|---|---|',
];
for (const r of reqs) {
  const cls = classOf(r.id);
  const proofs = proofGroups(r).map((g) => `\`${g.file}\` — **${g.ver}**: ${g.names.map((n) => `\`${n}\``).join(', ')}`);
  const verification = [
    ...proofs,
    ...testEntries(r),
    ...(r.verification?.planned ? [`planned: ${r.verification.planned}`] : []),
  ].join('; ') || (cls === 'SYS' ? 'by the requirements that refine it' : '—');
  let implementation = '—';
  if (r.implementation) implementation = r.implementation.map((f) => `\`${f}\``).join(', ');
  else if (refinedBy(r.id).length) implementation = `via ${refinedBy(r.id).join(', ')}`;
  else if (cls !== 'SYS') implementation = 'not yet implemented';
  rows.push(`| **${r.id}** | ${cell(`${r.title}. ${r.statement}`)} | ${implementation} | ${cell(verification)} | ${cls === 'SYS' ? '—' : r.level} |`);
}

const matrixPath = resolve(repoRoot, MATRIX);
if (!existsSync(matrixPath)) fatal(TAG, `${MATRIX} is missing`);
const matrix = readFileSync(matrixPath, 'utf8');
const at = matrix.indexOf(BEGIN);
const until = matrix.indexOf(END);
if (at === -1 || until === -1 || until < at) fatal(TAG, `${MATRIX} needs the ${BEGIN} … ${END} markers`);
const matrixText = `${matrix.slice(0, at + BEGIN.length)}\n\n${rows.join('\n')}\n\n${matrix.slice(until)}`;

writeOrCheck(TAG, [
  { path: 'spec/REQUIREMENTS.md', content: mdText, stale: 'spec/REQUIREMENTS.md is stale; run node scripts/generate-requirements.mjs', missing: 'spec/REQUIREMENTS.md is missing; run node scripts/generate-requirements.mjs' },
  { path: MATRIX, content: matrixText, stale: `the generated block of ${MATRIX} is stale; run node scripts/generate-requirements.mjs` },
], { current: 'spec/REQUIREMENTS.md and the matrix block are current.', wrote: 'wrote spec/REQUIREMENTS.md and the matrix block.' });
