// A worker that errors is replaced, and the task queued behind the one it was
// running goes to the replacement. It used to go to the broken worker, which
// was still in the pool when the queue was drained, and answered only at the
// wall-clock guard, reported as a program that ran too long.
//
// The same holds for a worker whose interpreter threw: the throw is a
// WebAssembly trap (out of memory, a Rust panic), and the instance it leaves
// refuses every later call. It used to stay in the pool and fail every task
// it was given after that.
//
// `Worker` is a fake that records what it is sent; the pool holds one worker.

import { afterEach, expect, it, vi } from 'vitest';

interface FakeWorkerShape {
    onmessage: ((event: { data: unknown }) => void) | null;
    onerror: ((event: { message: string }) => void) | null;
    posted: Array<{ type: string; id?: string; code?: string }>;
    terminated: boolean;
}

const env = vi.hoisted(() => {
    const workers: FakeWorkerShape[] = [];
    class FakeWorker {
        onmessage: ((event: { data: unknown }) => void) | null = null;
        onerror: ((event: { message: string }) => void) | null = null;
        posted: Array<{ type: string; id?: string; code?: string }> = [];
        terminated = false;
        constructor() { workers.push(this); }
        postMessage(message: { type: string; id?: string; code?: string }) { this.posted.push(message); }
        terminate() { this.terminated = true; }
    }
    (globalThis as any).window = { innerWidth: 1200 };
    (globalThis as any).Worker = FakeWorker;
    // Node 20 (CI) has no global `navigator`; later versions do.
    Object.defineProperty(globalThis, 'navigator', {
        value: { hardwareConcurrency: 1 },
        configurable: true,
        writable: true
    });
    return { workers };
});

import { WorkerManager } from './execution-worker-manager';

const executed = (worker: FakeWorkerShape): string[] =>
    worker.posted.filter((message) => message.type === 'execute').map((message) => message.code!);

afterEach(() => {
    vi.useRealTimers();
    env.workers.length = 0;
});

it('hands the task queued behind an errored worker to its replacement', async () => {
    vi.useFakeTimers();
    const manager = new WorkerManager();
    await manager.init();
    const [broken] = env.workers;
    const settled: string[] = [];
    manager.execute('A', { userWords: [] }).catch((error: Error) => settled.push(`A: ${error.message}`));
    void manager.execute('B', { userWords: [] }).then(() => settled.push('B ok'));

    broken!.onerror!({ message: 'boom' });
    await Promise.resolve();

    expect(executed(broken!)).toEqual(['A']);
    expect(broken!.terminated).toBe(true);
    expect(executed(env.workers[1]!)).toEqual(['B']);
    expect(settled).toEqual(['A: Worker error: boom']);
});

it('replaces a worker whose interpreter threw, and hands the queued task to the replacement', async () => {
    vi.useFakeTimers();
    const manager = new WorkerManager();
    await manager.init();
    const [trapped] = env.workers;
    const settled: string[] = [];
    manager.execute('A', { userWords: [] }).catch((error: Error) => settled.push(`A: ${error.message}`));
    void manager.execute('B', { userWords: [] }).then(() => settled.push('B ok'));

    const id = trapped!.posted.find((message) => message.type === 'execute')!.id;
    trapped!.onmessage!({ data: { type: 'error', id, data: 'RuntimeError: unreachable' } });
    await Promise.resolve();

    expect(trapped!.terminated).toBe(true);
    expect(executed(trapped!)).toEqual(['A']);
    expect(executed(env.workers[1]!)).toEqual(['B']);
    expect(settled).toEqual(['A: RuntimeError: unreachable']);
});
