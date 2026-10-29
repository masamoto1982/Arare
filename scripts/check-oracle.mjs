#!/usr/bin/env node
// The oracle comparison (AR-VER-209, docs/quality/VERIFICATION_PLAN.md):
// every program in tests/oracle/corpus.json runs in Arare's reference
// interpreter and in the pinned Ajisai build, and Arare's result must lie
// within the case's tolerance of Ajisai's exact one.
//
// Ajisai runs from the Node build committed under tools/mcp-server/wasm/
// generated/ (the commit UPSTREAM.md records). A prelude gives it Arare's
// Words as their exact counterparts: QUANTIZE, RESCALE and NARROW keep the
// exact value, NEG is `-1 MUL`, SUM is a FOLD of ADD. Arare runs as
// `arare run --json` (ARARE_BIN, or rust/target/debug/arare).
//
// A case marked `divergence` is one where Arare differs from Ajisai on
// purpose (ADR-0007): Arare must answer exactly what the case records and
// Ajisai must still answer something else, so the divergence stays the one
// documented and does not drift.

import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { readJson, repoRoot, reporter } from './lib/common.mjs';

const report = reporter('oracle');
const require = createRequire(import.meta.url);
const ajisai = require(resolve(repoRoot, 'tools/mcp-server/wasm/generated/ajisai_core.js'));
const arareBin = process.env.ARARE_BIN ?? resolve(repoRoot, 'rust/target/debug/arare');
if (!existsSync(arareBin)) {
  report.fail(`${arareBin} does not exist; build it with cargo build -p arare-cli (in rust/) or set ARARE_BIN.`);
  process.exit(1);
}

const PRELUDE = [
  "[ 'ARARE_E' BIND 'ARARE_W' BIND ] 'QUANTIZE' DEF",
  "[ 'ARARE_S' BIND ] 'RESCALE' DEF",
  "[ 'ARARE_W' BIND ] 'NARROW' DEF",
  "[ -1 MUL ] 'NEG' DEF",
  "[ 0 [ ADD ] FOLD ] 'SUM' DEF",
].join('\n');

/** A rational from a string `p/q` or `p`, as BigInts. */
function rat(text) {
  const [p, q = '1'] = String(text).split('/');
  return { p: BigInt(p), q: BigInt(q) };
}

/** A protocol node in a shape both languages share. */
function norm(node) {
  switch (node.type) {
    case 'number': return { kind: 'number', p: BigInt(node.value.numerator), q: BigInt(node.value.denominator) };
    case 'boolean': return { kind: 'boolean', value: node.value };
    case 'nil': return { kind: 'nil' };
    case 'vector': return { kind: 'vector', items: node.value.map(norm) };
    default: return { kind: node.type };
  }
}

function show(v) {
  switch (v.kind) {
    case 'number': return `${v.p}/${v.q}`;
    case 'boolean': return v.value ? 'TRUE' : 'FALSE';
    case 'nil': return 'NIL';
    case 'vector': return `[ ${v.items.map(show).join(' ')} ]`;
    default: return v.kind;
  }
}

const abs = (x) => (x < 0n ? -x : x);

/** Whether Arare's value `a` is within `tol` of Ajisai's `j`. */
function within(a, j, tol) {
  if (a.kind !== j.kind) return false;
  switch (a.kind) {
    case 'number':
      if (a.q === 0n || j.q === 0n) return a.q === j.q && (a.p === 0n) === (j.p === 0n);
      // |a.p/a.q − j.p/j.q| ≤ tol.p/tol.q, all denominators positive.
      return abs(a.p * j.q - j.p * a.q) * tol.q <= tol.p * a.q * j.q;
    case 'boolean': return a.value === j.value;
    case 'nil': return true;
    case 'vector': return a.items.length === j.items.length && a.items.every((x, i) => within(x, j.items[i], tol));
    default: return false;
  }
}

function runArare(source) {
  const r = spawnSync(arareBin, ['run', '--json', '-'], { input: source, encoding: 'utf8' });
  const out = JSON.parse(r.stdout);
  if (!out.ok) throw new Error(`Arare refused it: ${out.error}`);
  return out.stack.map(norm);
}

async function runAjisai(source) {
  const out = JSON.parse(await ajisai.agent_compute(`${PRELUDE}\n${source}`));
  if (out.error) throw new Error(`Ajisai refused it: ${out.message ?? JSON.stringify(out.error)}`);
  return out.stack.map(norm);
}

const { cases } = readJson('tests/oracle/corpus.json');
let compared = 0;
let diverged = 0;
for (const c of cases) {
  let a;
  let j;
  try {
    [a, j] = [runArare(c.source), await runAjisai(c.source)];
  } catch (error) {
    report.fail(`${c.name}: ${error.message}`);
    continue;
  }
  const shown = `Arare ${a.map(show).join('  ')}; Ajisai ${j.map(show).join('  ')}`;
  if (c.divergence) {
    const expected = c.divergence.arare;
    const same = a.length === j.length && a.every((x, i) => within(x, j[i], rat('0')));
    if (a.map(show).join('  ') !== expected) report.fail(`${c.name} (${c.divergence.kind}): Arare should answer ${expected}; ${shown}`);
    else if (same) report.fail(`${c.name} (${c.divergence.kind}): no longer diverges from Ajisai; ${shown}`);
    else diverged++;
    continue;
  }
  const tol = rat(c.tolerance);
  if (a.length !== j.length || !a.every((x, i) => within(x, j[i], tol))) {
    report.fail(`${c.name}: outside ${c.tolerance}; ${shown}`);
  } else {
    compared++;
  }
}

report.done(`${compared} case(s) within tolerance of Ajisai, ${diverged} documented divergence(s) holding.`);
