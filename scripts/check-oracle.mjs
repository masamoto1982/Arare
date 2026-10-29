#!/usr/bin/env node
// The oracle corpus (AR-VER-209, docs/quality/VERIFICATION_PLAN.md): every
// program in tests/oracle/corpus.json runs in Arare and in the pinned Ajisai
// build (scripts/lib/oracle.mjs), and Arare's result must lie within the
// error budget the checker infers (AR-HLR-027) of Ajisai's exact one.
//
// A case's `budget` is the largest budget the checker may infer for any of
// its values, worked out by hand from the rounding points the program passes
// through: the checker must be at least that tight, and must infer a budget
// at all. The comparison itself uses the inferred budget, so the corpus checks
// the checker's soundness on these programs (ADR-0007, ADR-0008).
//
// A case marked `divergence` is one where Arare differs from Ajisai on
// purpose (ADR-0007): Arare must answer exactly what the case records and
// Ajisai must still answer something else, so the divergence stays the one
// documented and does not drift. For an `overflow` divergence the checker
// must claim no budget: it cannot bound a value that left the widest width.

import { readJson, reporter } from './lib/common.mjs';
import { rat, ratLe, runAjisai, runArare, show, showRat, within } from './lib/oracle.mjs';

const report = reporter('oracle');
const { cases } = readJson('tests/oracle/corpus.json');

let arare;
try {
  arare = runArare(cases.map((c) => c.source));
} catch (error) {
  report.fatal(error.message);
}

let compared = 0;
let diverged = 0;
for (const [i, c] of cases.entries()) {
  const a = arare[i];
  const j = await runAjisai(c.source);
  if (a.error) { report.fail(`${c.name}: Arare refused it: ${a.error}`); continue; }
  if (j.error) { report.fail(`${c.name}: Ajisai refused it: ${j.error}`); continue; }
  const shown = `Arare ${a.stack.map(show).join('  ')}; Ajisai ${j.stack.map(show).join('  ')}`;
  if (c.divergence) {
    const expected = c.divergence.arare;
    const same = a.stack.length === j.stack.length && a.stack.every((x, k) => within(x, j.stack[k], rat('0')));
    if (a.stack.map(show).join('  ') !== expected) report.fail(`${c.name} (${c.divergence.kind}): Arare should answer ${expected}; ${shown}`);
    else if (same) report.fail(`${c.name} (${c.divergence.kind}): no longer diverges from Ajisai; ${shown}`);
    else if (c.divergence.kind === 'overflow' && a.stack.some((x) => x.budget !== null)) {
      report.fail(`${c.name}: the checker claims a budget for a value that overflowed; ${shown}`);
    } else diverged++;
    continue;
  }
  const most = rat(c.budget);
  if (a.stack.length !== j.stack.length) { report.fail(`${c.name}: stacks differ; ${shown}`); continue; }
  let ok = true;
  for (const [k, x] of a.stack.entries()) {
    if (x.budget === null) { report.fail(`${c.name}: no budget inferred for value ${k + 1}; ${shown}`); ok = false; continue; }
    if (!ratLe(x.budget, most)) {
      report.fail(`${c.name}: value ${k + 1} has budget ${showRat(x.budget)}, looser than ${c.budget}; ${shown}`);
      ok = false;
    }
    if (!within(x, j.stack[k], x.budget)) {
      report.fail(`${c.name}: value ${k + 1} outside its budget ${showRat(x.budget)}; ${shown}`);
      ok = false;
    }
  }
  if (ok) compared++;
}

report.done(`${compared} case(s) within the inferred budget of Ajisai, ${diverged} documented divergence(s) holding.`);
