// What passes between the page and the worker that runs a program: the
// interpreter state a run starts from, and the wall-clock guard it runs under.
//
// Kept apart from the worker pool that enforces the guard because both halves
// are also read where no pool exists: the worker restores from the snapshot,
// the profile badge discloses the guard beside the interpreter's own ceilings,
// and the diagnosis for a stopped run is written from it — none of which
// should have to construct a worker pool, and none of which runs in a browser
// during the tests.

import type { AjisaiInterpreter, UserWord } from '../wasm-interpreter-types';

// Per-task wall-clock cap on worker execution. The recursion guard returns an
// AjisaiError immediately for blown-stack programs; this is the second line of
// defence for "still running" non-recursive loops that neither hit the
// execution-step cap fast enough nor produce a recursion error. Set well above
// the longest legitimate run so a legal program never trips it.
export const EXECUTION_TIMEOUT_MS = 5_000;

/**
 * A run stopped by the wall-clock guard rather than by anything the
 * interpreter decided.
 *
 * Distinguished by type because it is the one refusal that carries no
 * diagnosis from the language: the worker is terminated where it stands, so
 * the Rust side never builds one and never gets to name a ceiling. A host that
 * cannot tell this apart from an ordinary failure can only print the sentence
 * and leave the reader to guess whether their program is wrong or merely slow.
 */
export class ExecutionTimeoutError extends Error {
    readonly limitMs: number;

    constructor(limitMs: number) {
        super(`Execution timed out after ${limitMs} ms`);
        this.name = 'ExecutionTimeoutError';
        this.limitMs = limitMs;
    }
}

/** A run stopped from outside (Escape), before the interpreter could answer. */
export class ExecutionAbortedError extends Error {
    constructor() {
        super('Execution aborted');
        this.name = 'ExecutionAbortedError';
    }
}

export interface InterpreterSnapshot {
    // The lossless snapshot (opaque string from `snapshot_stack`) and the only
    // format the worker round-trip restores from. Reusing the lossy observation
    // format silently changed exact values on every execution — a CodeBlock
    // came back as nil, √2 as its rational approximation. See LANG.OBSERVATION.FIREWALL.
    //
    // The observation-format stack used to travel beside it "for display on
    // the main thread", and the worker never read it: the main thread keeps
    // its own view of the stack (`ExecutionStateView`) for the before/after
    // comparison, so the copy only cost a structured clone of every value on
    // the stack per run.
    readonly stackSnapshot?: string;
    readonly userWords: UserWord[];
    /**
     * Host override for the execution step budget (water level, LANG.MACHINE.LIMITS).
     * A positive integer; omitted keeps the interpreter's own default
     * (`DEFAULT_MAX_EXECUTION_STEPS`).
     * Runtime safety control, not a language semantic.
     */
    readonly stepLimit?: number;
}

export const applyInterpreterSnapshot = (
    interpreter: AjisaiInterpreter,
    snapshot?: Partial<InterpreterSnapshot> | null
): void => {
    // A session reset reinitializes the session but keeps the cross-reset
    // compiled-artifact cache, so an unchanged user word's compiled plan is
    // reused across runs instead of recompiled. Reuse is content-identity keyed
    // and observationally transparent.
    interpreter.reset();
    if (!snapshot) return;

    // The lossless snapshot is the only accepted stack format, so exact values
    // (CodeBlock, ExactScalar) survive the worker round-trip (LANG.OBSERVATION.FIREWALL). A
    // snapshot without one restores an empty stack rather than silently
    // downgrading through the observation format.
    if (typeof snapshot.stackSnapshot === 'string') {
        interpreter.restore_stack_snapshot(snapshot.stackSnapshot);
    }
    if (snapshot.userWords) {
        interpreter.restore_user_words(snapshot.userWords);
    }
    // Untrusted partial snapshot: only a positive finite integer is a valid
    // budget; anything else keeps the interpreter default (the wasm side
    // ignores non-positive values as a second line of defence).
    if (typeof snapshot.stepLimit === 'number'
        && Number.isInteger(snapshot.stepLimit)
        && snapshot.stepLimit > 0) {
        interpreter.set_max_execution_steps(snapshot.stepLimit);
    }
};
