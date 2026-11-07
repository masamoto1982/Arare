// A WASI preview 1 host just large enough to run a command (AR-LLR-033).
//
// Arare's engine reaches the Playground as a `wasm32-wasip1` command, not as
// a library of exported functions (ADR-0013): it reads a request on standard
// input and writes its answer on standard output. A browser has no WASI, so
// this file is one: arguments, an empty environment, standard input from a
// buffer, standard output and error into buffers, and exit. Every other WASI
// function the module might import answers ENOSYS, so a build that starts
// calling one fails loudly in the tests rather than quietly in a browser.
//
// The same code runs under Node in the tests (`src/engine/arare-engine.test.ts`),
// so what the tests exercise is the host the Playground uses, not Node's own.

const ERRNO_SUCCESS = 0;
const ERRNO_BADF = 8;
const ERRNO_NOSYS = 52;

/** What a command answered: its exit status and everything it wrote. */
export interface CommandOutput {
    readonly exitCode: number;
    readonly stdout: string;
    readonly stderr: string;
}

// Thrown by `proc_exit` to unwind out of `_start`; caught below and nowhere
// else.
class ProcessExit {
    constructor(readonly code: number) {}
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/**
 * Run the command `module` once: instantiate it, hand it `args` (its name
 * first, as a shell would) and `stdin`, and call `_start`. Synchronous, so a
 * worker or the main thread can call it without awaiting: a compiled module
 * instantiates synchronously.
 */
export function runWasiCommand(
    module: WebAssembly.Module,
    args: readonly string[],
    stdin: string
): CommandOutput {
    const input = encoder.encode(stdin);
    let readAt = 0;
    const out: Uint8Array[] = [];
    const err: Uint8Array[] = [];
    const argv = args.map((a) => encoder.encode(`${a}\0`));
    let memory: WebAssembly.Memory | null = null;

    // Read again on every call: memory.grow replaces the buffer.
    const view = (): DataView => new DataView(memory!.buffer);
    const bytes = (): Uint8Array => new Uint8Array(memory!.buffer);

    const wasi: Record<string, (...a: number[]) => number> = {
        args_sizes_get(argcPtr, sizePtr) {
            view().setUint32(argcPtr!, argv.length, true);
            view().setUint32(sizePtr!, argv.reduce((n, a) => n + a.length, 0), true);
            return ERRNO_SUCCESS;
        },
        args_get(argvPtr, bufPtr) {
            let at = bufPtr!;
            argv.forEach((a, i) => {
                view().setUint32(argvPtr! + 4 * i, at, true);
                bytes().set(a, at);
                at += a.length;
            });
            return ERRNO_SUCCESS;
        },
        environ_sizes_get(countPtr, sizePtr) {
            view().setUint32(countPtr!, 0, true);
            view().setUint32(sizePtr!, 0, true);
            return ERRNO_SUCCESS;
        },
        environ_get() {
            return ERRNO_SUCCESS;
        },
        fd_read(fd, iovs, iovsLen, nreadPtr) {
            if (fd !== 0) return ERRNO_BADF;
            let total = 0;
            for (let i = 0; i < iovsLen!; i++) {
                const ptr = view().getUint32(iovs! + 8 * i, true);
                const len = view().getUint32(iovs! + 8 * i + 4, true);
                const chunk = input.subarray(readAt, readAt + len);
                bytes().set(chunk, ptr);
                readAt += chunk.length;
                total += chunk.length;
                if (chunk.length < len) break;
            }
            view().setUint32(nreadPtr!, total, true);
            return ERRNO_SUCCESS;
        },
        fd_write(fd, iovs, iovsLen, nwrittenPtr) {
            const sink = fd === 1 ? out : fd === 2 ? err : null;
            if (sink === null) return ERRNO_BADF;
            let total = 0;
            for (let i = 0; i < iovsLen!; i++) {
                const ptr = view().getUint32(iovs! + 8 * i, true);
                const len = view().getUint32(iovs! + 8 * i + 4, true);
                sink.push(bytes().slice(ptr, ptr + len));
                total += len;
            }
            view().setUint32(nwrittenPtr!, total, true);
            return ERRNO_SUCCESS;
        },
        proc_exit(code) {
            throw new ProcessExit(code!);
        }
    };

    const imports: Record<string, Record<string, (...a: number[]) => number>> = {};
    for (const { module: from, name } of WebAssembly.Module.imports(module)) {
        const table = (imports[from] ??= {});
        table[name] = (from === 'wasi_snapshot_preview1' && wasi[name]) || (() => ERRNO_NOSYS);
    }

    const instance = new WebAssembly.Instance(module, imports as WebAssembly.Imports);
    memory = instance.exports.memory as WebAssembly.Memory;
    let exitCode = 0;
    try {
        (instance.exports._start as () => void)();
    } catch (thrown) {
        if (!(thrown instanceof ProcessExit)) throw thrown;
        exitCode = thrown.code;
    }
    const text = (chunks: Uint8Array[]): string => {
        const all = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
        let at = 0;
        for (const c of chunks) {
            all.set(c, at);
            at += c.length;
        }
        return decoder.decode(all);
    };
    return { exitCode, stdout: text(out), stderr: text(err) };
}
