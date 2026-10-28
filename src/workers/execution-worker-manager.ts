

import { isMobileViewport } from '../platform/viewport';
import type { ExecuteResult, WasmModule } from '../wasm-interpreter-types';
import {
    EXECUTION_TIMEOUT_MS,
    ExecutionAbortedError,
    ExecutionTimeoutError,
    type InterpreterSnapshot
} from './execution-contract';
import {
    detectParallelCapability,
    describeParallelCapability,
    type ParallelCapability,
} from '../platform/cross-origin-isolation';

// The wasm bundle is compiled once on the main thread and the compiled module
// is handed to every worker, so each worker skips the download and compile
// and only instantiates.
let wasmModule: WasmModule | null = null;
let compiledModule: WebAssembly.Module | null = null;

export async function initWasm(): Promise<WasmModule | null> {
    if (wasmModule) return wasmModule;

    try {
        if (!compiledModule) {
            const wasmUrl = new URL('../wasm/generated/ajisai_core_bg.wasm', import.meta.url);
            try {
                compiledModule = await WebAssembly.compileStreaming(fetch(wasmUrl));
            } catch {
                const response = await fetch(wasmUrl);
                const bytes = await response.arrayBuffer();
                compiledModule = await WebAssembly.compile(bytes);
            }
        }

        const module = await import('../wasm/generated/ajisai_core.js') as unknown as WasmModule;

        if (module.default) {
            await (module.default as (input?: unknown) => Promise<unknown>)({ module_or_path: compiledModule });
        }

        // Surface Rust panics as console.error with a real stack trace
        // instead of an opaque `RuntimeError: unreachable executed`.
        try {
            module.init_panic_hook?.();
        } catch (e) {
            console.warn('init_panic_hook unavailable; rebuild wasm to enable.', e);
        }

        wasmModule = module;
        return module;
    } catch (error) {
        console.error('Failed to load WASM:', error);
        return null;
    }
}

interface WorkerTask {
    id: string;
    code: string;
    state: InterpreterSnapshot;
    resolve: (result: any) => void;
    reject: (error: any) => void;
    timeoutHandle: ReturnType<typeof setTimeout> | null;
}

interface WorkerInstance {
    worker: Worker;
    busy: boolean;
    currentTaskId: string | null;
}

const MAX_MOBILE_WORKERS = 2;

export class WorkerManager {
    private workers: WorkerInstance[] = [];
    private taskQueue: WorkerTask[] = [];
    private activeTasks = new Map<string, WorkerTask>();
    private compiledModule: WebAssembly.Module | null = null;
    private maxWorkers = isMobileViewport()
        ? Math.min(navigator.hardwareConcurrency || 2, MAX_MOBILE_WORKERS)
        : navigator.hardwareConcurrency || 4;
    // Whether SharedArrayBuffer-backed wasm threading can run in this page
    // (implicit-parallelism roadmap Phase 5). Observational for now: the pool
    // still uses snapshot-copying Web Workers regardless. Once a threaded wasm
    // build ships, `recommendedThreads` drives the wasm-bindgen-rayon pool size
    // and `threadsAvailable` gates the SharedArrayBuffer snapshot transport.
    private parallelCapability: ParallelCapability = detectParallelCapability();

    async init(): Promise<void> {
        console.log('[WorkerManager] Initializing worker pool...');
        console.log(`[WorkerManager] parallel capability: ${describeParallelCapability(this.parallelCapability)}`);
        this.workers = [];

        this.compiledModule = compiledModule;

        if (!this.compiledModule) {
            console.warn('[WorkerManager] Compiled WASM module not available; workers will init independently');
        }


        for (let i = 0; i < this.maxWorkers; i++) {
            this.createWorker();
        }
    }

    private createWorker(): void {
        const worker = new Worker(new URL('./interpreter-execution-worker.ts', import.meta.url), { type: 'module' });
        const instance: WorkerInstance = { worker, busy: false, currentTaskId: null };

        worker.onmessage = (event) => this.resolveWorkerMessage(instance, event.data);
        worker.onerror = (error) => this.resolveWorkerError(instance, error);


        if (this.compiledModule) {
            worker.postMessage({ type: 'init', wasmModule: this.compiledModule });
        }

        this.workers.push(instance);
    }


    private ensureWorkers(): void {
        if (this.workers.length > 0) return;
        for (let i = 0; i < this.maxWorkers; i++) {
            this.createWorker();
        }
    }

    private resolveWorkerMessage(instance: WorkerInstance, message: any): void {
        const task = this.activeTasks.get(message.id);
        if (!task) return;

        switch (message.type) {
            case 'result':
                task.resolve(message.data);
                break;
            case 'error':
                task.reject(new Error(message.data));
                // The interpreter answers a failing program with a result, so
                // an exception out of it is a WebAssembly trap: memory ran
                // out, or Rust panicked. A trap abandons the instance mid-call
                // and Rust's state with it, and the next call into that
                // instance is refused ("recursive use of an object") while the
                // memory it grew to stays held, so the worker is never handed
                // another task.
                this.replaceWorker(instance);
                break;
        }
        this.completeTask(instance);
    }

    // Out of the pool, terminated, and a fresh worker in its place, so the
    // pool size stays constant. Out of the pool before the queue is looked
    // at: completing a task drains the queue, and a worker still in the pool
    // would be handed the next task.
    private replaceWorker(instance: WorkerInstance): void {
        const index = this.workers.indexOf(instance);
        if (index > -1) this.workers.splice(index, 1);
        instance.worker.terminate();
        this.createWorker();
    }

    private resolveWorkerError(instance: WorkerInstance, error: ErrorEvent): void {
        console.error('[WorkerManager] Worker error:', error.message);
        // Replaced before the task completes: a broken worker still in the
        // pool was handed the next task, which then only answered at the
        // wall-clock guard.
        this.replaceWorker(instance);
        if (instance.currentTaskId) {
            const task = this.activeTasks.get(instance.currentTaskId);
            task?.reject(new Error(`Worker error: ${error.message}`));
        }
        this.completeTask(instance);
    }

    private completeTask(instance: WorkerInstance): void {
        if (instance.currentTaskId) {
            const task = this.activeTasks.get(instance.currentTaskId);
            if (task) {
                if (task.timeoutHandle !== null) {
                    clearTimeout(task.timeoutHandle);
                    task.timeoutHandle = null;
                }
            }
            this.activeTasks.delete(instance.currentTaskId);
        }
        instance.busy = false;
        instance.currentTaskId = null;
        this.processQueue();
    }

    private handleTaskTimeout(taskId: string): void {
        this.stopActiveTask(taskId, new ExecutionTimeoutError(EXECUTION_TIMEOUT_MS));
        this.processQueue();
    }

    // Stop a running task where it stands. The interpreter runs synchronously
    // inside its worker, so a message asking it to stop is not read until the
    // run is over; terminating the worker is the only stop that takes effect.
    // A terminated worker cannot be reused, so we drop it from the pool and
    // spawn a replacement immediately to keep the pool size constant. The
    // wall-clock guard and Abort both stop a task this way.
    private stopActiveTask(taskId: string, error: Error): void {
        const task = this.activeTasks.get(taskId);
        if (!task) return;
        if (task.timeoutHandle !== null) {
            clearTimeout(task.timeoutHandle);
            task.timeoutHandle = null;
        }

        const instance = this.workers.find(w => w.currentTaskId === taskId);
        if (instance) this.replaceWorker(instance);
        else this.createWorker();

        this.activeTasks.delete(taskId);

        task.reject(error);
    }

    private processQueue(): void {
        // Drain as many queued tasks as there are idle workers.
        while (this.taskQueue.length > 0) {
            const availableWorker = this.workers.find(w => !w.busy);
            if (!availableWorker) break;
            const nextTask = this.taskQueue.shift()!;
            this.assignTaskToWorker(availableWorker, nextTask);
        }
    }

    private assignTaskToWorker(instance: WorkerInstance, task: WorkerTask): void {
        instance.busy = true;
        instance.currentTaskId = task.id;
        this.activeTasks.set(task.id, task);

        task.timeoutHandle = setTimeout(
            () => this.handleTaskTimeout(task.id),
            EXECUTION_TIMEOUT_MS
        );

        instance.worker.postMessage({
            type: 'execute',
            id: task.id,
            code: task.code,
            state: task.state
        });
    }

    private createTaskId(): string {

        if (typeof crypto !== 'undefined' && crypto.randomUUID) {
            return crypto.randomUUID();
        }

        return `${Date.now()}-${Math.random().toString(36).substring(2, 11)}`;
    }

    execute(code: string, state: InterpreterSnapshot): Promise<ExecuteResult> {
        this.ensureWorkers();
        return new Promise((resolve, reject) => {
            const shared = { settled: false };
            const wrapResolve = (result: ExecuteResult): void => {
                if (shared.settled) return;
                shared.settled = true;
                resolve(result);
            };
            const wrapReject = (error: Error): void => {
                if (shared.settled) return;
                shared.settled = true;
                reject(error);
            };

            // One execution path: a run is one task on one worker.
            this.taskQueue.push({
                id: this.createTaskId(),
                code,
                state,
                resolve: wrapResolve,
                reject: wrapReject,
                timeoutHandle: null
            });
            this.processQueue();
        });
    }

    abortAll(): void {
        console.log('[WorkerManager] Aborting all tasks...');

        const abortError = new ExecutionAbortedError();
        for (const task of this.taskQueue) {
            task.reject(abortError);
        }
        this.taskQueue = [];

        for (const id of [...this.activeTasks.keys()]) {
            this.stopActiveTask(id, abortError);
        }
    }

    async resetAllWorkers(): Promise<void> {
        this.abortAll();
        this.workers.forEach(w => w.worker.terminate());
        await this.init();
    }
}

export const WORKER_MANAGER = new WorkerManager();
