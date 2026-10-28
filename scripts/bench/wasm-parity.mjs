#!/usr/bin/env node
// Differential check between two WebAssembly builds of the Core.
//
//   node scripts/bench/wasm-parity.mjs <reference-dir> <candidate-dir>
//
// Each directory is a wasm-pack `--target web` output (the four files
// rebuild-wasm.sh copies). Every program in the corpus is run through
// `agent_compute` on both, and the JSON envelopes must be identical apart
// from wall-clock fields. This is the evidence a post-link optimizer
// (wasm-opt) needs before its output ships: the optimizer once miscompiled
// this module (commit 89a0c7b), and a miscompile shows up here as a differing
// stack, error, trace or meter on some program.
//
// Corpus: every cell of docs/semantics-table.json (each Core Word applied to
// each operand domain, ~11k programs), the MCP golden and eval cases, the
// outcome witnesses, and the speed-bench cases at reduced size.

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "..", "..");
const [refDir, candDir] = process.argv.slice(2).map((p) => resolve(p));
if (!refDir || !candDir) {
  console.error("usage: wasm-parity.mjs <reference-dir> <candidate-dir>");
  process.exit(2);
}

async function load(dir) {
  // A query string keeps the two imports of one file name distinct modules.
  const wasm = await import(`${join(dir, "ajisai_core.js")}?dir=${encodeURIComponent(dir)}`);
  await wasm.default({ module_or_path: readFileSync(join(dir, "ajisai_core_bg.wasm")) });
  return wasm;
}

const readJson = (p) => JSON.parse(readFileSync(join(repoRoot, p), "utf8"));

function corpus() {
  const programs = [];
  const table = readJson("docs/semantics-table.json");
  const domain = new Map(table.domains.map((d) => [d.id, d.source]));
  for (const cell of table.cells) {
    programs.push([...cell.inputs.map((id) => domain.get(id)), cell.word].join(" "));
  }
  for (const c of readJson("tools/mcp-server/golden/cases.json").cases) programs.push(c.source);
  for (const c of readJson("tools/mcp-server/eval/cases.json").cases) {
    if (c.arguments?.source) programs.push(c.arguments.source);
  }
  for (const w of readJson("spec/outcome-witnesses.json").witnesses) programs.push(w.source);
  for (const c of readJson("scripts/bench/speed-bench-cases.json").cases) {
    // Shrink the bench operands so the corpus stays fast.
    const small = (s) => s.replace(/\b(9{4,})\b/g, "999").replace(/\b(4{1}9{4,})\b/g, "499");
    programs.push(`${small(c.setup)} ${c.source.repeat(Math.min(c.repeat ?? 1, 50))}`);
  }
  return programs;
}

// Wall-clock readings differ run to run on one build; everything else is a
// function of the program.
const VOLATILE = /(elapsed|wall|millis|Ms$|duration|timestamp|now)/i;
function stable(json) {
  return JSON.stringify(JSON.parse(json), (k, v) => (VOLATILE.test(k) ? undefined : v));
}

const ref = await load(refDir);
const cand = await load(candDir);
const programs = corpus();
let mismatches = 0;
for (const source of programs) {
  const a = stable(await ref.agent_compute(source, undefined));
  const b = stable(await cand.agent_compute(source, undefined));
  if (a !== b) {
    mismatches++;
    if (mismatches <= 10) {
      console.error(`MISMATCH: ${source}\n  reference: ${a.slice(0, 400)}\n  candidate: ${b.slice(0, 400)}`);
    }
  }
}
console.log(`wasm-parity: ${programs.length} programs, ${mismatches} mismatches`);
process.exit(mismatches === 0 ? 0 : 1);
