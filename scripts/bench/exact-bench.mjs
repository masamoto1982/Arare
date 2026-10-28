#!/usr/bin/env node
// Exact-arithmetic comparison: Ajisai against FLINT, python-flint, SymPy and
// Python's fractions.Fraction on the cases of exact-bench-cases.json.
//
//   node scripts/bench/exact-bench.mjs [--json]
//
// Environment:
//   PYTHON         interpreter with sympy and python-flint installed (default python3)
//   FLINT_TIMEOUT  seconds one FLINT case may take before it is reported as
//                  not finishing (default 60)
//   AJISAI_WASM    wasm-pack `--target web` output to time as "Ajisai (WASM)"
//                  (default src/wasm/generated/; build it from the current
//                  source with scripts/rebuild-wasm.sh, as the Playground
//                  deploy does, or the column times whatever was committed)
//
// Every system computes the same exact value; a time is reported only for a
// system whose answer matches Ajisai's, compared as text in Ajisai's own
// rendering (`p/q`, `[ a b ]`, `1/1+sqrt(2)`). FLINT's Calcium answers the
// square-root sum only in decimal, so that row is compared on the leading
// fractional digits Ajisai's FORMAT and Calcium both carry. A system that
// cannot express a case, or is not installed, is shown as "—".
//
// Ajisai is the native release build, timed in-process (rust/examples/
// exact_bench.rs), and the WebAssembly build under Node (exact-bench-wasm.mjs),
// whose answers are checked against the native ones by DIGEST; FLINT is C compiled here with -O2 when a C compiler and the
// FLINT headers are present; the Python systems are timed in-process too. No
// system's start-up is timed. Times are the best of `repeats` runs on this
// machine and say nothing about another.

import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..', '..');
const asJson = process.argv.includes('--json');
const suite = JSON.parse(readFileSync(join(here, 'exact-bench-cases.json'), 'utf8'));
const python = process.env.PYTHON ?? 'python3';
const flintTimeout = Number(process.env.FLINT_TIMEOUT ?? 60);
const COMPARED_DIGITS = 25;

function log(message) {
  process.stderr.write(`${message}\n`);
}

function ajisai() {
  log('[exact-bench] Ajisai (cargo run --release --example exact_bench)');
  const out = execFileSync('cargo', ['run', '-q', '--release', '--example', 'exact_bench', '--', '--json'], {
    cwd: join(repoRoot, 'rust'),
    encoding: 'utf8',
    maxBuffer: 1 << 30,
  });
  return JSON.parse(out);
}

function pythonSystems() {
  log(`[exact-bench] Python systems (${python})`);
  const run = spawnSync(python, [join(here, 'exact_bench.py'), '--json'], { encoding: 'utf8', maxBuffer: 1 << 30 });
  if (run.status !== 0) {
    log(`[exact-bench] Python side failed; skipping it\n${run.stderr}`);
    return { versions: {}, rows: [] };
  }
  if (run.stderr) log(run.stderr.trim());
  return JSON.parse(run.stdout);
}

function flint() {
  const dir = mkdtempSync(join(tmpdir(), 'exact-bench-flint-'));
  const binary = join(dir, 'exact_bench_flint');
  try {
    const cc = spawnSync('cc', ['-O2', join(here, 'exact_bench_flint.c'), '-o', binary, '-lflint', '-lgmp'], {
      encoding: 'utf8',
    });
    if (cc.status !== 0) {
      log(`[exact-bench] FLINT not built (needs a C compiler and the FLINT headers); skipping it\n${cc.stderr ?? ''}`);
      return { version: null, rows: [] };
    }
    const rows = [];
    for (const c of suite.cases) {
      log(`[exact-bench] FLINT ${c.name}`);
      const run = spawnSync(binary, [c.kind, String(c.n), String(suite.repeats)], {
        encoding: 'utf8',
        maxBuffer: 1 << 30,
        timeout: flintTimeout * 1000,
      });
      if (run.error?.code === 'ETIMEDOUT' || run.signal) {
        rows.push({ system: 'FLINT', name: c.name, n: c.n, timedOut: true });
        continue;
      }
      if (run.status !== 0) continue;
      rows.push({ system: 'FLINT', name: c.name, n: c.n, ...JSON.parse(run.stdout) });
    }
    const version = rows.find((r) => r.version)?.version;
    return { version: version ? `FLINT ${version} (C, -O2)` : null, rows };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// The fractional digits both renderings carry, without the last two digits of
// either, so a correctly rounded last digit on one side and a differently
// rounded one on the other cannot decide the comparison.
function ajisaiWasm(nativeRows) {
  const moduleDir = resolve(process.env.AJISAI_WASM ?? join(repoRoot, 'src', 'wasm', 'generated'));
  log(`[exact-bench] Ajisai WebAssembly (${moduleDir})`);
  const dir = mkdtempSync(join(tmpdir(), 'exact-bench-wasm-'));
  try {
    const expect = join(dir, 'native.json');
    writeFileSync(expect, JSON.stringify(nativeRows));
    const run = spawnSync(
      process.execPath,
      [join(here, 'exact-bench-wasm.mjs'), '--json', '--module', moduleDir, '--expect', expect],
      { encoding: 'utf8', maxBuffer: 1 << 30 },
    );
    if (run.status !== 0) {
      log(`[exact-bench] WebAssembly side failed; skipping it\n${run.stderr}`);
      return [];
    }
    return JSON.parse(run.stdout).map((r) => ({ system: 'Ajisai (WASM)', ...r }));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function fractionalDigits(text) {
  const [whole, fraction = ''] = text.trim().split('.');
  return `${whole}.${fraction.slice(0, COMPARED_DIGITS)}`;
}

function agrees(row, reference) {
  if ('matchesNative' in row) return row.matchesNative === true;
  if (row.answer === reference.answer) return true;
  if (reference.decimal && /^-?\d+\.\d+$/.test(row.answer ?? '')) {
    return fractionalDigits(row.answer) === fractionalDigits(reference.decimal);
  }
  return false;
}

const ajisaiRows = ajisai();
const wasmRows = ajisaiWasm(ajisaiRows);
const py = pythonSystems();
const fl = flint();

const systems = ['Ajisai', 'Ajisai (WASM)', 'FLINT', 'python-flint', 'SymPy', 'Fraction'];
const versions = {
  Ajisai: `Ajisai ${JSON.parse(readFileSync(join(repoRoot, 'package.json'), 'utf8')).version} (native release build)`,
  ...(wasmRows.length ? { 'Ajisai (WASM)': 'the same engine compiled to WebAssembly, under Node ' + process.versions.node } : {}),
  ...(fl.version ? { FLINT: fl.version } : {}),
  ...py.versions,
};
const others = [...wasmRows, ...fl.rows, ...py.rows];

const table = suite.cases.map((c) => {
  const reference = ajisaiRows.find((r) => r.name === c.name);
  const cells = { Ajisai: { ms: reference.ms } };
  for (const system of systems.slice(1)) {
    const row = others.find((r) => r.system === system && r.name === c.name);
    if (!row) cells[system] = { absent: true };
    else if (row.timedOut) cells[system] = { timedOut: true };
    else if (!agrees(row, reference)) cells[system] = { differs: true };
    else cells[system] = { ms: row.ms };
  }
  return { name: c.name, n: c.n, what: c.what, cells };
});

if (asJson) {
  console.log(JSON.stringify({ versions, repeats: suite.repeats, table }, null, 2));
} else {
  const cell = (x, base) => {
    if (x.absent) return '—';
    if (x.timedOut) return `> ${flintTimeout} s`;
    if (x.differs) return 'answer differs';
    const ms = x.ms >= 100 ? x.ms.toFixed(0) : x.ms >= 10 ? x.ms.toFixed(1) : x.ms.toFixed(2);
    return x === base ? `${ms} ms` : `${ms} ms (×${(x.ms / base.ms).toFixed(1)})`;
  };
  const present = systems.filter((s) => versions[s]);
  console.log(`| case | n | ${present.join(' | ')} |`);
  console.log(`|---|---:|${present.map(() => '---:').join('|')}|`);
  for (const row of table) {
    console.log(`| ${row.name} | ${row.n} | ${present.map((s) => cell(row.cells[s], row.cells.Ajisai)).join(' | ')} |`);
  }
  console.log('');
  console.log(`Best of ${suite.repeats} runs. (×k) is the time relative to Ajisai: above 1 is slower than Ajisai.`);
  for (const s of present) console.log(`- ${s}: ${versions[s]}`);
}
