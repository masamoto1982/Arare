#!/usr/bin/env node
// Every theorem in formal/ rests only on Lean's three standard axioms.
//
// `lake build` accepts a proof that uses `sorry` — it warns and still
// succeeds — and it accepts a theorem that leans on a declared `axiom` or on
// `native_decide`'s trust in the compiler. Any of those would let the Lean job
// go green while proving less than docs/quality/FORMAL_VERIFICATION.md says.
// So this gate asks Lean itself, with `#print axioms`, what each theorem
// depends on, and fails on anything beyond propext, Classical.choice and
// Quot.sound (sorry shows up as sorryAx, native_decide as Lean.ofReduceBool).
//
// Run after `lake build`, from anywhere: node scripts/check-formal-axioms.mjs

import { execFileSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { repoRoot, reporter } from './lib/common.mjs';
import { leanTheorems } from './lib/lean.mjs';

const ALLOWED = new Set(['propext', 'Classical.choice', 'Quot.sound']);
const report = reporter('formal-axioms');

const theorems = leanTheorems();
if (theorems.length === 0) {
  report.fail('formal/Arare/ declares no theorem; the Lean model has nothing to check.');
  process.exit(1);
}

const dir = mkdtempSync(join(tmpdir(), 'arare-axioms-'));
const probe = join(dir, 'Probe.lean');
writeFileSync(probe, ['import Arare', ...theorems.map((t) => `#print axioms ${t.name}`), ''].join('\n'));

let output;
try {
  output = execFileSync('lake', ['env', 'lean', probe], {
    cwd: resolve(repoRoot, 'formal'),
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
} catch (error) {
  report.fail(`Lean could not check the axioms:\n${error.stdout ?? ''}${error.stderr ?? ''}`);
  process.exit(1);
} finally {
  rmSync(dir, { recursive: true, force: true });
}

// Lean prints one message per #print, each starting with the quoted name.
const seen = new Map();
for (const block of output.split(/\n(?=')/)) {
  const name = block.match(/^'([^']+)'/)?.[1];
  if (!name) continue;
  const list = block.match(/depends on axioms: \[([^\]]*)\]/s)?.[1];
  seen.set(name, list ? list.split(',').map((s) => s.trim()).filter(Boolean) : []);
}

for (const { name, file } of theorems) {
  if (!seen.has(name)) {
    report.fail(`${name} (${file}): Lean printed no axiom report for it; the name the reader derived may be wrong.`);
    continue;
  }
  const extra = seen.get(name).filter((axiom) => !ALLOWED.has(axiom));
  if (extra.length) report.fail(`${name} (${file}) depends on ${extra.join(', ')}.`);
}

report.done(`${theorems.length} theorem(s) rest only on ${[...ALLOWED].join(', ')}.`);
