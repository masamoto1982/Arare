// Ajisai as the oracle beside Arare in the Playground (AR-LLR-037, refining
// AR-HLR-035; WORK_ORDER.md D-13): the same session's programs, run by the
// pinned Ajisai bundle (src/wasm/ajisai-oracle/) with the prelude that gives
// it Arare's Words as their exact counterparts — the comparison CI makes
// (scripts/lib/oracle.mjs, ADR-0007), made in the page.
//
// Loaded on first use only: a session that never runs Arare never downloads
// Ajisai twice.

import { PRELUDE } from './oracle-compare.mjs';
import { compileEngine } from './registry';

/** Ajisai's answer: its stack, as protocol nodes, or why it refused. */
export type OracleAnswer =
    | { readonly ok: true; readonly stack: unknown[] }
    | { readonly ok: false; readonly error: string };

type Compute = (source: string) => Promise<string>;

let compute: Promise<Compute> | null = null;

const load = async (): Promise<Compute> => {
    const module = await compileEngine('ajisai');
    const bindings = (await import('../wasm/ajisai-oracle/ajisai_core.js')) as unknown as {
        initSync: (options: { module: WebAssembly.Module }) => unknown;
        agent_compute: Compute;
    };
    bindings.initSync({ module });
    return (source) => bindings.agent_compute(source);
};

/** Read Ajisai's JSON answer to one program (its agent `compute` envelope). */
export const readOracleAnswer = (json: string): OracleAnswer => {
    const out = JSON.parse(json) as { stack?: unknown[]; error?: unknown; message?: string };
    if (out.error || !Array.isArray(out.stack)) {
        return { ok: false, error: out.message ?? JSON.stringify(out.error ?? 'no stack') };
    }
    return { ok: true, stack: out.stack };
};

/** The source Ajisai runs for a session: the prelude, then every program in order. */
export const oracleSource = (programs: readonly string[]): string =>
    [PRELUDE, ...programs].join('\n');

/** Run a session's programs in Ajisai. */
export const runOracle = async (programs: readonly string[], run?: Compute): Promise<OracleAnswer> => {
    try {
        const go = run ?? await (compute ??= load());
        return readOracleAnswer(await go(oracleSource(programs)));
    } catch (error) {
        compute = null;
        return { ok: false, error: error instanceof Error ? error.message : String(error) };
    }
};
