#!/usr/bin/env node
// Runs every golden case against both the native-CLI and WASM-worker backends
// directly (bypassing the server's own backend auto-selection) and asserts they
// agree on every stable semantic field. `runtimeMetrics` and the `mcp`
// provenance block are excluded: they carry host/engine metadata and execution
// counters, not language semantics.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { LIMITS, resolveAjisaiBin } from "../index.js";
import { limitCases } from "../golden/limit-cases.js";
import { NativeCliBackend } from "./native-cli.js";
import { WasmWorkerBackend } from "./wasm-worker.js";

const here = dirname(fileURLToPath(import.meta.url));

// The same discovery the server uses, so the parity test compares the binary
// the server would actually run.
const bin = resolveAjisaiBin();
if (!bin) {
  console.error(
    "backend parity test requires a built native `ajisai` binary; run " +
      "`cargo build --manifest-path rust/Cargo.toml --bin ajisai` first, or set AJISAI_BIN.",
  );
  process.exit(1);
}
if (!WasmWorkerBackend.isAvailable()) {
  console.error(
    "backend parity test requires the packaged WASM bundle; run `npm run build:mcp-wasm` first.",
  );
  process.exit(1);
}

const native = new NativeCliBackend({
  bin,
  wallTimeMs: LIMITS.wallTimeMs,
  responseBytes: LIMITS.responseBytes,
  executionSteps: LIMITS.executionSteps,
});
const wasm = new WasmWorkerBackend({
  wallTimeMs: LIMITS.wallTimeMs,
  executionSteps: LIMITS.executionSteps,
  responseBytes: LIMITS.responseBytes,
});

// Every field of the result envelope except the host-metrics/provenance
// fields a backend's own process/thread model incidentally affects.
const STABLE_FIELDS = [
  "schemaVersion",
  "status",
  "stack",
  "stackDisplay",
  "output",
  "outcome",
  "message",
  "error",
  "contractDecls",
];

function stableView(envelope) {
  const view = {};
  for (const field of STABLE_FIELDS) view[field] = envelope[field];
  return view;
}

const golden = JSON.parse(readFileSync(join(here, "..", "golden", "cases.json"), "utf8"));

let failures = 0;
for (const goldenCase of golden.cases) {
  const [nativeEnvelope, wasmEnvelope] = await Promise.all([
    native.compute(goldenCase.source),
    wasm.compute(goldenCase.source),
  ]);
  const left = JSON.stringify(stableView(nativeEnvelope));
  const right = JSON.stringify(stableView(wasmEnvelope));
  const pass = left === right;
  console.log(`${pass ? "PASS" : "FAIL"}  ${goldenCase.name}`);
  if (!pass) {
    failures += 1;
    console.error(`  native: ${left}`);
    console.error(`  wasm:   ${right}`);
  }
}

// Limit parity. A ceiling that only one backend applies is invisible to the
// golden cases above — none of them produces an oversized response — which is
// exactly how `responseBytes` came to be declared for both backends and
// enforced by one. Comparing the *outcome class* at each declared boundary is
// what makes the declaration true of whichever backend answered.
async function outcome(backend, source) {
  try {
    const envelope = await backend.compute(source);
    return `ok:${envelope.status}`;
  } catch (error) {
    return `hostError:${error.code}`;
  }
}

for (const limit of limitCases()) {
  for (const probe of limit.probes) {
    const [left, right] = await Promise.all([
      outcome(native, probe.source),
      outcome(wasm, probe.source),
    ]);
    const pass = left === right;
    console.log(`${pass ? "PASS" : "FAIL"}  limit ${limit.name} (${probe.edge})`);
    if (!pass) {
      failures += 1;
      console.error(`  native: ${left}`);
      console.error(`  wasm:   ${right}`);
    }
  }
}

if (failures) {
  console.error(`${failures} case(s) diverged between the native and WASM backends.`);
  process.exit(1);
}
console.log("native and WASM backends agree on every golden case and every declared limit boundary.");
