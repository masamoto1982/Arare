#!/usr/bin/env node
// Time the speed-bench cases on the WebAssembly build.
//
//   node scripts/bench/speed-bench-wasm.mjs [FILTER] [--json] [--module <dir>]
//
// The cases are `speed-bench-cases.json`, the same file
// `rust/examples/speed_bench.rs` times natively, so the two tables line up
// row for row. `--module` points at a wasm-pack output directory (default:
// the committed bundle in src/wasm/generated/), which is how two builds — say
// with and without wasm-opt — are compared on one machine.
//
// Each case is timed as `setup source` minus `setup` alone, both through the
// `bench_execute` export: a fresh unbounded interpreter whose result is never
// converted to JS. The two run as interleaved pairs and the median difference
// is reported with its spread (`paired-timing.mjs`); `--pairs` sets how many.

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { formatRow, pairedBodyMs } from "./paired-timing.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");

const args = process.argv.slice(2);
const json = args.includes("--json");
const moduleFlag = args.indexOf("--module");
const moduleDir = moduleFlag >= 0
  ? resolve(args[moduleFlag + 1])
  : join(repoRoot, "src", "wasm", "generated");
const filter = args.find(
  (a, i) => !a.startsWith("--") && args[i - 1] !== "--module" && args[i - 1] !== "--pairs",
);

const wasm = await import(join(moduleDir, "ajisai_core.js"));
await wasm.default({ module_or_path: readFileSync(join(moduleDir, "ajisai_core_bg.wasm")) });

const suite = JSON.parse(readFileSync(join(here, "speed-bench-cases.json"), "utf8"));
const pairsFlag = args.indexOf("--pairs");
const pairs = pairsFlag >= 0 ? Number(args[pairsFlag + 1]) : 15;
const run = (source) => wasm.bench_execute(source);

const rows = [];
for (const c of suite.cases) {
  if (filter && !c.name.includes(filter)) continue;
  const source = c.source.repeat(c.repeat ?? 1);
  const timing = await pairedBodyMs(run, c.setup, source, pairs);
  rows.push({
    name: c.name,
    ms: Number(timing.ms.toFixed(3)),
    iqrMs: Number(timing.iqr.toFixed(3)),
    noisy: timing.noisy,
    nsPerElem: Number(((timing.ms * 1e6) / c.elements).toFixed(2)),
  });
  if (!json) console.log(formatRow(c.name, timing, c.elements));
}
if (json) console.log(JSON.stringify(rows, null, 2));
