// The two sides of the oracle comparison (ADR-0007), shared by the corpus
// (scripts/check-oracle.mjs, AR-VER-209) and the generated programs
// (scripts/check-oracle-random.mjs, AR-VER-217).
//
// Ajisai runs from the Node build committed under tools/mcp-server/wasm/
// generated/ (the commit UPSTREAM.md records). A prelude gives it Arare's
// Words as their exact counterparts: QUANTIZE, RESCALE and NARROW keep the
// exact value, NEG is `-1 MUL`, SUM is a FOLD of ADD. Arare runs as
// `arare check --lines` (ARARE_BIN, or rust/target/debug/arare): one process
// for every program, each answered with its stack and the error budget the
// checker infers for every value (AR-HLR-027).

import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { repoRoot } from './common.mjs';

const require = createRequire(import.meta.url);
const ajisai = require(resolve(repoRoot, 'tools/mcp-server/wasm/generated/ajisai_core.js'));

export const arareBin = process.env.ARARE_BIN ?? resolve(repoRoot, 'rust/target/debug/arare');

const PRELUDE = [
  "[ 'ARARE_E' BIND 'ARARE_W' BIND ] 'QUANTIZE' DEF",
  "[ 'ARARE_S' BIND ] 'RESCALE' DEF",
  "[ 'ARARE_W' BIND ] 'NARROW' DEF",
  "[ -1 MUL ] 'NEG' DEF",
  "[ 0 [ ADD ] FOLD ] 'SUM' DEF",
].join('\n');

/** A rational from a string `p/q` or `p`, as BigInts. */
export function rat(text) {
  const [p, q = '1'] = String(text).split('/');
  return { p: BigInt(p), q: BigInt(q) };
}

/** A protocol node in a shape both languages share; Arare's carries its budget. */
export function norm(node) {
  const budget = node.budget === undefined ? undefined : node.budget === null ? null : rat(node.budget);
  switch (node.type) {
    case 'number': return { kind: 'number', p: BigInt(node.value.numerator), q: BigInt(node.value.denominator), budget };
    case 'boolean': return { kind: 'boolean', value: node.value, budget };
    case 'nil': return { kind: 'nil', budget };
    case 'vector': return { kind: 'vector', items: node.value.map(norm), budget };
    default: return { kind: node.type, budget };
  }
}

export function show(v) {
  switch (v.kind) {
    case 'number': return `${v.p}/${v.q}`;
    case 'boolean': return v.value ? 'TRUE' : 'FALSE';
    case 'nil': return 'NIL';
    case 'vector': return `[ ${v.items.map(show).join(' ')} ]`;
    default: return v.kind;
  }
}

export const showRat = (r) => `${r.p}/${r.q}`;

const abs = (x) => (x < 0n ? -x : x);

/** Whether `a ≤ b` for two rationals with positive denominators. */
export const ratLe = (a, b) => a.p * b.q <= b.p * a.q;

/** Whether Arare's value `a` is within `tol` of Ajisai's `j`. */
export function within(a, j, tol) {
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

/** Arare's answers to many programs, one `arare check --lines` run. */
export function runArare(sources) {
  if (!existsSync(arareBin)) {
    throw new Error(`${arareBin} does not exist; build it with cargo build -p arare-cli (in rust/) or set ARARE_BIN.`);
  }
  for (const s of sources) if (s.includes('\n')) throw new Error(`a program must be one line: ${s}`);
  const r = spawnSync(arareBin, ['check', '--lines', '-'], {
    input: sources.join('\n') + '\n',
    encoding: 'utf8',
    maxBuffer: 1 << 30,
  });
  if (r.status !== 0) throw new Error(`arare check failed: ${r.stderr}`);
  const lines = r.stdout.trimEnd().split('\n');
  if (lines.length !== sources.length) throw new Error(`arare answered ${lines.length} of ${sources.length} programs`);
  return lines.map((l) => {
    const out = JSON.parse(l);
    return out.ok ? { stack: out.stack.map(norm), field: out.field } : { error: out.error };
  });
}

/** Ajisai's answer to one program, or its refusal. */
export async function runAjisai(source) {
  const out = JSON.parse(await ajisai.agent_compute(`${PRELUDE}\n${source}`));
  if (out.error) return { error: out.message ?? JSON.stringify(out.error) };
  return { stack: out.stack.map(norm) };
}
