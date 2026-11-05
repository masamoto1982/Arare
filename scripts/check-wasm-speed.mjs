#!/usr/bin/env node
// AR-PERF-007 (verification case AR-VER-226): the WebAssembly SIMD128 build
// of the compiled route within 3 times the native build. Both builds of
// rust/crates/arare-timing time the same compiled kernels on the same inputs
// and print `name nanoseconds` per kernel; this runs each build twice,
// alternating, keeps every kernel's best time, and fails when a kernel's
// WebAssembly time exceeds 3 times its native one. The native build is the
// one the AR-PERF requirements are measured on (target x86-64-v3 in CI).
//
// Usage:
//   node scripts/check-wasm-speed.mjs --native <arare-timing> --wasm <arare-timing.wasm>

import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { fatal, repoRoot } from './lib/common.mjs';

const TAG = 'wasm-speed';
const LIMIT = 3;
const flag = (name) => {
  const at = process.argv.indexOf(name);
  return at === -1 ? undefined : process.argv[at + 1];
};
const native = flag('--native');
const wasm = flag('--wasm');
if (!native || !wasm) fatal(TAG, 'usage: node scripts/check-wasm-speed.mjs --native <binary> --wasm <module.wasm>');

function times(command, args) {
  const r = spawnSync(command, args, { encoding: 'utf8', maxBuffer: 1 << 20 });
  if (r.status !== 0) fatal(TAG, `${command} ${args.join(' ')} failed:\n${r.stderr}`);
  return new Map(
    r.stdout
      .trim()
      .split('\n')
      .map((line) => line.split(' '))
      .map(([name, ns]) => [name, Number(ns)]),
  );
}

const runner = resolve(repoRoot, 'scripts/lib/run-wasi.mjs');
const best = { native: new Map(), wasm: new Map() };
for (let round = 0; round < 2; round++) {
  const runs = [
    ['native', times(native, [])],
    ['wasm', times(process.execPath, ['--no-warnings', runner, wasm])],
  ];
  for (const [build, t] of runs) {
    for (const [name, ns] of t) best[build].set(name, Math.min(best[build].get(name) ?? Infinity, ns));
  }
}

let failed = false;
console.log('| Kernel | native (µs) | WebAssembly SIMD128 (µs) | ratio | limit |');
console.log('|---|---:|---:|---:|---|');
for (const [name, n] of best.native) {
  const w = best.wasm.get(name);
  if (w === undefined) fatal(TAG, `the WebAssembly build did not time ${name}`);
  const ratio = w / n;
  const ok = ratio <= LIMIT;
  failed ||= !ok;
  console.log(`| ${name} | ${(n / 1e3).toFixed(3)} | ${(w / 1e3).toFixed(3)} | ${ratio.toFixed(2)}× | ≤ ${LIMIT} ${ok ? 'ok' : 'MISSED'} |`);
}
if (failed) fatal(TAG, `AR-PERF-007: a kernel's WebAssembly build is more than ${LIMIT} times its native one`);
console.error(`[${TAG}] AR-PERF-007: every kernel's WebAssembly SIMD128 build is within ${LIMIT} times its native one`);
