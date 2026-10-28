// Runs inside a worker_threads Worker owned by `WasmWorkerBackend`. Off the
// stdio server's main thread by construction — the parent never imports the
// WASM module or awaits it synchronously (see wasm-worker.js).
//
// One fresh WebAssembly instance per call, mirroring the native backend's
// one-process-per-call model. The Worker itself is kept and reused, because
// starting a Worker is what a call used to spend its time on (50-100 ms,
// against well under a millisecond for the operation); what the isolation
// actually rests on is the instance. Each call's instance has its own linear
// memory and its own copy of the wasm-bindgen glue's module-level state, and
// it has run nothing but the fixed warm-up below, exactly as the Worker used
// to. The compiled module is shared: it is code, not state.
//
// The response-size ceiling is checked here, on the JSON *text* the WASM entry
// point returns — the same bytes the native backend's `execFile` measures on
// stdout, so both backends reject at the same threshold on the same units.
// Checking it here also means an oversized envelope is never structured-cloned
// across the thread boundary.

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { parentPort, workerData } from "node:worker_threads";

// The first operation in a fresh instance pays for the engine's one-time
// tables (`OnceLock`s: the Core Word registry, builtin specs); the warm-up
// pays that instead, ahead of the call. It cannot change what the call
// answers: every operation builds a fresh interpreter, and what the warm-up
// leaves behind is those immutable tables.
const WARM_UP = "[ 1 2 3 ] [ 2 MUL 1 ADD ] MAP 'X' BIND X LENGTH 1/3 ADD";

// The glue wasm-pack emits for `--target nodejs` is CommonJS that, when its
// body runs, reads `ajisai_core_bg.wasm`, compiles it and instantiates it into
// module-level variables. Running that body again is what gives a fresh
// instance; it is handed the bytes and the compiled module it would otherwise
// read and compile again, through the two names it reaches them by (`fs` and
// `WebAssembly.Module`). If a later wasm-bindgen reaches them some other way,
// the body still runs correctly and only loses the reuse.
function loadInstanceFactory(gluePath) {
  const glueSource = readFileSync(gluePath, "utf8");
  const wasmBytes = readFileSync(join(dirname(gluePath), "ajisai_core_bg.wasm"));
  const compiled = new WebAssembly.Module(wasmBytes);
  const nodeRequire = createRequire(gluePath);
  const fsWithBytes = { ...nodeRequire("fs"), readFileSync: () => wasmBytes };
  const glueRequire = (id) => (id === "fs" || id === "node:fs" ? fsWithBytes : nodeRequire(id));
  const webAssembly = Object.create(WebAssembly, {
    // `new WebAssembly.Module(bytes)` in the glue: a constructor that returns
    // an object yields that object, so this answers the already-compiled one.
    Module: { value: function Module() { return compiled; } },
  });
  const body = new Function(
    "require", "module", "exports", "__dirname", "__filename", "WebAssembly",
    glueSource,
  );
  return () => {
    const module = { exports: {} };
    body(glueRequire, module, module.exports, dirname(gluePath), gluePath, webAssembly);
    return module.exports;
  };
}

let freshInstance;
try {
  freshInstance = loadInstanceFactory(workerData.wasmModulePath);
} catch (error) {
  freshInstance = () => { throw error; };
}

// The instance the next operation will run in, prepared ahead of it. A
// preparation failure is reported to that operation, as a load failure was
// when the load happened after it arrived.
function prepare() {
  return (async () => {
    const wasm = freshInstance();
    await wasm.agent_compute(WARM_UP, undefined);
    return { wasm };
  })().catch((error) => ({ error }));
}

let next = prepare();

async function run({ op, source, stepLimit, responseBytes }) {
  const { wasm, error } = await next;
  if (error) throw error;
  let json;
  if (op === "compute") {
    json = await wasm.agent_compute(source, stepLimit ?? undefined);
  } else if (op === "check") {
    json = wasm.agent_check(source);
  } else if (op === "inferContracts") {
    json = wasm.agent_infer_contracts(source);
  } else {
    return {
      error: {
        code: "backendFailure",
        message: "The Ajisai backend failed to produce a result.",
        detail: `unknown wasm-worker operation: ${op}`,
      },
    };
  }
  if (typeof responseBytes === "number" && Buffer.byteLength(json, "utf8") > responseBytes) {
    return {
      error: {
        code: "responseTooLarge",
        message: `The result exceeds the ${responseBytes}-byte response limit. Reduce the size of the value left on the stack.`,
      },
    };
  }
  return { envelope: JSON.parse(json) };
}

// The backend hands a Worker one operation at a time and waits for its
// answer before handing it another; the answer is posted before the next
// instance is prepared, so preparing it is off the caller's critical path.
parentPort.on("message", async (job) => {
  let answer;
  try {
    answer = await run(job);
  } catch (error) {
    answer = {
      error: {
        code: "backendFailure",
        message: "The Ajisai backend failed to produce a result.",
        detail: error?.message ?? String(error),
      },
    };
  }
  parentPort.postMessage(answer);
  next = prepare();
});
