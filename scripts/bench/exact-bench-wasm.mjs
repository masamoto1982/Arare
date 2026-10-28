#!/usr/bin/env node
// Time the exact-bench cases on the WebAssembly build.
//
//   node scripts/bench/exact-bench-wasm.mjs [FILTER] [--json] [--module <dir>] [--expect <file>]
//
// The cases are `exact-bench-cases.json`, the same file
// `rust/examples/exact_bench.rs` times natively. `--module` points at a
// wasm-pack `--target web` output directory (default: src/wasm/generated/,
// which `scripts/rebuild-wasm.sh` writes and the Playground deploy builds the
// same way).
//
// Each case is timed whole through `bench_execute` — a fresh interpreter with
// every priced ceiling lifted, its result never converted to JS — as the
// native side times it whole: one discarded warm-up, then the best of
// `repeats` runs.
//
// `--expect` names the native side's `--json` output. Each case's answer is
// then checked, untimed, against the native answer's DIGEST — its content
// identity, so a value too large to display, or too costly for the agent
// profile's ceilings, is still compared whole: `bench_execute` runs
// `1 'mismatch' <case> DIGEST '<native digest>' EQ SELECT 1 ADD`, which
// completes exactly when the two answers are one value and otherwise stops on
// adding 1 to a String. Without `--expect` the answers go unchecked.

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");

const args = process.argv.slice(2);
const asJson = args.includes("--json");
const moduleFlag = args.indexOf("--module");
const moduleDir = moduleFlag >= 0 ? resolve(args[moduleFlag + 1]) : join(repoRoot, "src", "wasm", "generated");
const filter = args.find((a, i) => !a.startsWith("--") && args[i - 1] !== "--module" && args[i - 1] !== "--expect");

const wasm = await import(join(moduleDir, "ajisai_core.js"));
await wasm.default({ module_or_path: readFileSync(join(moduleDir, "ajisai_core_bg.wasm")) });

const suite = JSON.parse(readFileSync(join(here, "exact-bench-cases.json"), "utf8"));
const expectFlag = args.indexOf("--expect");
const expected = expectFlag >= 0 ? JSON.parse(readFileSync(resolve(args[expectFlag + 1]), "utf8")) : null;

// Whether this build's answer is the value whose content identity is `digest`.
async function agrees(source, digest) {
  try {
    await wasm.bench_execute(`1 'mismatch' ${source} DIGEST '${digest}' EQ SELECT 1 ADD`);
    return true;
  } catch {
    return false;
  }
}

const rows = [];
for (const c of suite.cases) {
  if (filter && !c.name.includes(filter)) continue;
  const source = c.source.replaceAll("{n}", String(c.n));
  await wasm.bench_execute(source); // warm-up, discarded
  let best = Infinity;
  for (let i = 0; i < suite.repeats; i++) {
    const started = performance.now();
    await wasm.bench_execute(source);
    best = Math.min(best, performance.now() - started);
  }
  const digest = expected?.find((r) => r.name === c.name)?.digest;
  const matches = digest ? await agrees(source, digest) : null;
  rows.push({ name: c.name, n: c.n, ms: best, matchesNative: matches });
  if (!asJson) {
    const note = matches === null ? "  (answer unchecked)" : matches ? "" : "  (ANSWER DIFFERS FROM NATIVE)";
    console.log(`${c.name.padEnd(14)} n=${String(c.n).padEnd(7)} ${best.toFixed(2).padStart(10)} ms${note}`);
  }
}
if (asJson) console.log(JSON.stringify(rows));
