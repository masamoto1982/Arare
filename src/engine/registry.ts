// The engines the Playground's shell can run, and which one it runs
// (AR-LLR-035, refining AR-HLR-033; WORK_ORDER.md D-13).
//
// Arare is the Playground's engine; Ajisai stays beside it as the oracle, and
// can be chosen to run instead. The choice is made once per page load —
// `?engine=` in the URL, else the last choice, else Arare — because every
// surface (the stack, the saved session, the worker pool) belongs to one
// engine, and a page reload is the one switch that leaves none of them
// half-moved. What an engine can do beyond the shared seam is said here, not
// probed on the instance: the shell asks its descriptor.

import type { PlaygroundEngine, WasmModule } from '../wasm-interpreter-types';
import { ArareEngine } from './arare-engine';

export type EngineKind = 'arare' | 'ajisai';

export interface EngineDescriptor {
    readonly kind: EngineKind;
    /** How the engine selector names it. */
    readonly label: string;
    /** The header badge. */
    readonly badge: string;
    /** Whether it has User Words (DEF, the Dictionary's User sheet, Export and Import). */
    readonly userWords: boolean;
    /**
     * Whether a name bound in one step of step mode is still bound in the
     * next: Arare runs a session's earlier programs again before each one
     * (ADR-0013), so it is; Ajisai runs each piece as a program of its own.
     */
    readonly namesAcrossSteps: boolean;
    /** Whether it infers error budgets, and so whether to compare it with the oracle. */
    readonly budgets: boolean;
    /** The Reference for it, in English or Japanese. */
    readonly reference: (lang: 'en' | 'ja') => string;
}

const AJISAI_COMMIT = '0bb45c6aaf7c9603c1ec5fec37bc8aadc4800bcc';

export const ENGINES: Readonly<Record<EngineKind, EngineDescriptor>> = {
    arare: {
        kind: 'arare',
        label: 'Arare',
        badge: 'Playground · Arare',
        userWords: false,
        namesAcrossSteps: true,
        budgets: true,
        reference: (lang) => `docs/${lang}/index.html`
    },
    ajisai: {
        kind: 'ajisai',
        label: 'Ajisai (oracle)',
        badge: 'Playground · Ajisai oracle',
        userWords: true,
        namesAcrossSteps: false,
        budgets: false,
        // Ajisai's own Reference at the commit the oracle bundle is built from
        // (src/wasm/ajisai-oracle/ORACLE.md).
        reference: (lang) =>
            `https://github.com/masamoto1982/Ajisai/blob/${AJISAI_COMMIT}/public/docs/${lang}/index.html`
    }
};

/** Where the last choice of engine is kept, for the next visit. */
export const ENGINE_STORAGE_KEY = 'arare-playground-engine';

const isEngineKind = (value: unknown): value is EngineKind =>
    value === 'arare' || value === 'ajisai';

/** The engine a page runs: `?engine=` in `search`, else `stored`, else Arare. */
export const selectEngineKind = (search: string, stored: string | null): EngineKind => {
    const asked = new URLSearchParams(search).get('engine');
    if (isEngineKind(asked)) return asked;
    if (isEngineKind(stored)) return stored;
    return 'arare';
};

/** The engine this page runs, read once from the URL and the saved choice. */
export const readEngineKind = (): EngineKind => {
    let stored: string | null = null;
    try {
        stored = localStorage.getItem(ENGINE_STORAGE_KEY);
    } catch {
        // Storage unavailable: the URL and the default still decide.
    }
    return selectEngineKind(window.location.search, stored);
};

/** Remember `kind` and reload the page running it. */
export const switchEngine = (kind: EngineKind): void => {
    try {
        localStorage.setItem(ENGINE_STORAGE_KEY, kind);
    } catch {
        // Not remembered; the URL below still carries the choice.
    }
    const url = new URL(window.location.href);
    url.searchParams.set('engine', kind);
    url.hash = '';
    window.location.assign(url.toString());
};

/** Where the engine's WebAssembly is served from. */
const wasmUrl = (kind: EngineKind): URL =>
    kind === 'arare'
        ? new URL('../wasm/arare/arare_wasm.wasm', import.meta.url)
        : new URL('../wasm/ajisai-oracle/ajisai_core_bg.wasm', import.meta.url);

/** Download and compile an engine's module, once per page and handed to every worker. */
export const compileEngine = async (kind: EngineKind): Promise<WebAssembly.Module> => {
    const url = wasmUrl(kind);
    try {
        return await WebAssembly.compileStreaming(fetch(url));
    } catch {
        const response = await fetch(url);
        if (!response.ok) throw new Error(`${url} answered ${response.status}`);
        return WebAssembly.compile(await response.arrayBuffer());
    }
};

/** An engine instance on a compiled module, on the main thread or in a worker. */
export const instantiateEngine = async (
    kind: EngineKind,
    module: WebAssembly.Module
): Promise<PlaygroundEngine> => {
    if (kind === 'arare') return new ArareEngine(module);
    const bindings = (await import('../wasm/ajisai-oracle/ajisai_core.js')) as unknown as WasmModule & {
        initSync: (options: { module: WebAssembly.Module }) => unknown;
    };
    bindings.initSync({ module });
    // Surface Rust panics as console.error with a real stack trace instead of
    // an opaque `RuntimeError: unreachable executed`.
    try {
        bindings.init_panic_hook?.();
    } catch (error) {
        console.warn('init_panic_hook unavailable.', error);
    }
    return new bindings.AjisaiInterpreter();
};
