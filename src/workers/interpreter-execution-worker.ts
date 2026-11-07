// One worker of the pool: it runs a program on its own instance of the
// page's engine (src/engine/registry.ts, AR-LLR-035), restored from the
// snapshot the page hands it, and answers with the result and the snapshot
// the run left.

import type { ExecuteResult, PlaygroundEngine } from '../wasm-interpreter-types';
import { compileEngine, instantiateEngine, type EngineKind } from '../engine/registry';
import { applyInterpreterSnapshot } from './execution-contract';

let interpreter: PlaygroundEngine | null = null;
let ready: Promise<PlaygroundEngine | null> | null = null;

const start = async (kind: EngineKind, module: WebAssembly.Module | null): Promise<PlaygroundEngine | null> => {
    try {
        interpreter = await instantiateEngine(kind, module ?? await compileEngine(kind));
        return interpreter;
    } catch (e) {
        console.error(`[Worker] Failed to start the ${kind} engine:`, e);
        return null;
    }
};

self.onmessage = async (event: MessageEvent) => {
    const { type, id } = event.data;

    if (type === 'init') {
        const module = event.data.wasmModule instanceof WebAssembly.Module ? event.data.wasmModule : null;
        ready = start(event.data.engine as EngineKind, module);
        return;
    }

    // There is no `abort` message: the engine runs synchronously, so one
    // would not be read until the run was over. The pool stops a run by
    // terminating this worker (execution-worker-manager.ts, `stopActiveTask`).
    if (type !== 'execute') return;

    if (!interpreter) await ready;
    if (!interpreter) {
        self.postMessage({ type: 'error', id, data: 'Interpreter not initialized' });
        return;
    }

    try {
        applyInterpreterSnapshot(interpreter, event.data.state);
        const result: ExecuteResult = await interpreter.execute(event.data.code);
        // The lossless snapshot is the format the main thread restores from,
        // so the post-run state survives the trip exactly.
        result.stackSnapshot = interpreter.snapshot_stack();
        self.postMessage({ type: 'result', id, data: result });
    } catch (error: any) {
        self.postMessage({ type: 'error', id, data: error.toString() });
    }
};
