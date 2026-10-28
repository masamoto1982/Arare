

import type {
    AjisaiInterpreter,
    ExecuteResult,
} from '../wasm-interpreter-types';
import { applyInterpreterSnapshot } from './execution-contract';

let interpreter: AjisaiInterpreter | null = null;


const bindingsPromise = import('../wasm/generated/ajisai_core.js');


async function initFromCompiledModule(wasmModule: WebAssembly.Module): Promise<boolean> {
    try {
        const bindings = await bindingsPromise;
        bindings.initSync({ module: wasmModule });
        interpreter = new bindings.AjisaiInterpreter() as unknown as AjisaiInterpreter;
        console.log('[Worker] Initialized from pre-compiled module');
        return true;
    } catch (e) {
        console.error('[Worker] Failed to init from pre-compiled module:', e);
        return false;
    }
}


async function initFallback(): Promise<boolean> {
    if (interpreter) return true;
    try {
        const bindings = await bindingsPromise;
        await bindings.default({});
        interpreter = new bindings.AjisaiInterpreter() as unknown as AjisaiInterpreter;
        console.log('[Worker] Initialized via fallback (default init)');
        return true;
    } catch (e) {
        console.error('[Worker] Fallback initialization failed:', e);
        return false;
    }
}

self.onmessage = async (event: MessageEvent) => {
    const { type, id } = event.data;

    if (type === 'init') {

        if (event.data.wasmModule instanceof WebAssembly.Module) {
            await initFromCompiledModule(event.data.wasmModule);
        }
        return;
    }

    // There is no `abort` message: the interpreter runs synchronously, so one
    // would not be read until the run was over. The pool stops a run by
    // terminating this worker (execution-worker-manager.ts, `stopActiveTask`).
    if (type !== 'execute') return;


    if (!interpreter) {
        const success = await initFallback();
        if (!success) {
            self.postMessage({ type: 'error', id, data: 'Interpreter not initialized' });
            return;
        }
    }

    try {

        applyInterpreterSnapshot(interpreter!, event.data.state);

        const result: ExecuteResult = await interpreter!.execute(event.data.code);

        // Attach the lossless stack snapshot (LANG.OBSERVATION.FIREWALL): it is the format the
        // main thread restores from, so exact post-run values (CodeBlock,
        // ExactScalar) survive instead of the lossy observation `stack`. The
        // interpreter still holds the post-execute state here, so this captures
        // the result stack exactly.
        result.stackSnapshot = interpreter!.snapshot_stack();

        self.postMessage({ type: 'result', id, data: result });

    } catch (error: any) {
        self.postMessage({ type: 'error', id, data: error.toString() });
    }
};
