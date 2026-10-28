#!/usr/bin/env node
// The requirements registry is well formed and reaches the Lean model in both
// directions (docs/quality/STANDARDS.md, requirements standard).
//
//   - IDs are unique and of a known class; a requirement refines only
//     requirements that exist, and a high-level, low-level or performance
//     requirement refines at least one;
//   - every HLR, LLR and PERF names its quality level, and a derived one says why;
//   - every QL-A HLR is backed by a Lean theorem, or names why not;
//   - every theorem a requirement cites exists in formal/Arare/, and its file
//     cites that requirement's ID;
//   - every theorem in formal/Arare/ is cited by some requirement, so no proof
//     floats free of what it is for;
//   - a low-level requirement names the files that implement it, each of which
//     exists and cites it, and the tests and Kani harnesses that verify it,
//     each of which exists where it says; a QL-A one has at least one.
//
// Theorem existence is read from the source; whether each theorem is proved
// without gaps is scripts/check-formal-axioms.mjs's question.

import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { repoRoot, reporter } from './lib/common.mjs';
import { leanTheorems } from './lib/lean.mjs';
import { CLASS_RE, LEVELS, REGISTRY, classOf, requirements } from './lib/requirements.mjs';

const report = reporter('requirements');
const reqs = requirements();
const ids = new Set();

for (const r of reqs) {
  if (!CLASS_RE.test(r.id ?? '')) report.fail(`${r.id}: not an ID of a requirement class (STANDARDS.md)`);
  if (ids.has(r.id)) report.fail(`${r.id}: listed twice in ${REGISTRY}`);
  ids.add(r.id);
  for (const field of ['title', 'statement']) {
    if (typeof r[field] !== 'string' || r[field].trim() === '') report.fail(`${r.id}: no ${field}`);
  }
}

const theorems = new Map(leanTheorems().map((t) => [t.name, t]));
const cited = new Set();

for (const r of reqs) {
  const cls = classOf(r.id);
  if (cls === 'SYS') {
    if (!r.source) report.fail(`${r.id}: a system requirement names its source decision`);
    continue;
  }
  if (!LEVELS.includes(r.level)) report.fail(`${r.id}: level ${r.level} is not one of ${LEVELS.join(', ')}`);
  if (!Array.isArray(r.refines) || r.refines.length === 0) report.fail(`${r.id}: refines nothing`);
  for (const parent of r.refines ?? []) {
    if (!ids.has(parent)) report.fail(`${r.id}: refines ${parent}, which is not in ${REGISTRY}`);
  }
  if (typeof r.derived !== 'boolean') report.fail(`${r.id}: derived must be true or false`);
  if (r.derived && !r.rationale) report.fail(`${r.id}: derived without a rationale`);
  const lean = r.verification?.lean ?? [];
  const evidence = lean.length + (r.verification?.tests ?? []).length + (r.verification?.kani ?? []).length;
  if (evidence === 0 && !r.verification?.planned) report.fail(`${r.id}: no verification, proved, tested or planned`);
  for (const name of lean) {
    cited.add(name);
    const t = theorems.get(name);
    if (!t) { report.fail(`${r.id}: cites ${name}, which formal/Arare/ does not declare`); continue; }
    if (!t.ids.includes(r.id)) report.fail(`${r.id}: cites ${name}, but ${t.file} never names ${r.id}`);
  }
}

const textOf = (path) => (existsSync(resolve(repoRoot, path)) ? readFileSync(resolve(repoRoot, path), 'utf8') : null);

for (const r of reqs) {
  if (classOf(r.id) !== 'LLR') continue;
  const impl = r.implementation ?? [];
  if (impl.length === 0) report.fail(`${r.id}: a low-level requirement names the files that implement it`);
  for (const path of impl) {
    const text = textOf(path);
    if (text === null) report.fail(`${r.id}: implementation ${path} does not exist`);
    else if (!text.includes(r.id)) report.fail(`${r.id}: ${path} does not cite ${r.id}`);
  }
  const { tests = [], kani = [] } = r.verification ?? {};
  for (const { id, file } of tests) {
    const text = textOf(file);
    if (text === null) report.fail(`${r.id}: test file ${file} does not exist`);
    else if (!text.includes(id)) report.fail(`${r.id}: ${file} does not carry ${id}`);
  }
  for (const { name, file } of kani) {
    const text = textOf(file);
    if (text === null) report.fail(`${r.id}: harness file ${file} does not exist`);
    else if (!new RegExp(`#\\[kani::proof\\][^]*?fn ${name}\\(`).test(text)) report.fail(`${r.id}: ${file} has no Kani harness ${name}`);
  }
  if (r.level === 'QL-A' && tests.length + kani.length === 0) report.fail(`${r.id}: a QL-A low-level requirement needs tests or a Kani harness`);
}

// A QL-A high-level requirement is backed by a Lean theorem, or says in
// verification.formalExempt why a proof is not the right evidence for it.
for (const r of reqs) {
  if (classOf(r.id) !== 'HLR' || r.level !== 'QL-A') continue;
  const { lean = [], formalExempt } = r.verification ?? {};
  if (lean.length === 0 && !formalExempt) {
    report.fail(`${r.id}: a QL-A requirement needs a Lean theorem or a formalExempt reason`);
  }
  if (lean.length > 0 && formalExempt) report.fail(`${r.id}: cites theorems and claims an exemption`);
}

for (const [name, t] of theorems) {
  if (!cited.has(name)) report.fail(`${name} (${t.file}) is cited by no requirement in ${REGISTRY}`);
}

report.done(`${reqs.length} requirement(s); ${cited.size} of ${theorems.size} theorem(s) cited, all present.`);
