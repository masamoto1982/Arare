import {
    applyInterpreterSnapshot,
    ExecutionAbortedError,
    ExecutionTimeoutError,
    type InterpreterSnapshot
} from '../workers/execution-contract';
import { getPlatform } from '../platform';
import type {
    AjisaiInterpreter,
    ExecuteResult,
    ProtocolError,
    UserWord,
    Value
} from '../wasm-interpreter-types';
import type { ExecutionSurfaceChanges } from './gui-layout-state';

/** What was thrown, as an Error: a thrown non-Error is wrapped, never cast. */
export const toError = (thrown: unknown): Error =>
    thrown instanceof Error ? thrown : new Error(String(thrown));

// A Record crosses the protocol as two aligned arrays of nodes
// (LANG.RECORDS.STRUCTURE); either may be missing or malformed on an untrusted
// payload, and then it reads as empty.
export const readRecordParts = (value: unknown): { keys: Value[]; values: Value[] } => {
    const record = value as { keys?: Value[]; values?: Value[] } | null;
    return {
        keys: Array.isArray(record?.keys) ? record.keys : [],
        values: Array.isArray(record?.values) ? record.values : []
    };
};

// ── Error report ────────────────────────────────────────────────────────────
// What an ERROR carries beside its message, written under it: the category a
// host branches on, the ceiling that refused the run, the Word it is
// attributed to and where in the source the run was. The wall-clock stop
// renders through here too, so every refusal reads the same way.

export const renderErrorReport = (error: ProtocolError): string => {
    const category = error.resource ? `${error.category} (${error.resource})` : error.category;
    const word = error.word ? ` in ${error.word}` : '';
    const at =
        typeof error.line === 'number'
            ? ` at line ${error.line}${typeof error.column === 'number' ? `, column ${error.column}` : ''}`
            : '';
    return `[ERROR] ${category ?? 'unknown'}${word}${at}`;
};

// ── Snapshot, sync and the run's own explanations ───────────────────────────

// Every User word with its definition and description, looked up by name.
// `restore_user_words` skips a definition-less word, so a lookup that missed
// here would run the worker without the user's words.
export const collectUserWords = (interpreter: AjisaiInterpreter): UserWord[] =>
    interpreter.collect_user_words_info().map(([name]) => ({
        name,
        definition: interpreter.lookup_word_definition(name),
        description: interpreter.lookup_word_description(name)
    }));

// The one reading of a result's success. The host sets `status` and `error`
// together; either says the run did not complete.
export const isFailure = (result: ExecuteResult): boolean =>
    result.status !== 'OK' || Boolean(result.error);

// What the worker needs to start a run from the main thread's interpreter.
// `userWords` may be passed in when the caller has just collected them for
// its own before/after view, so the dictionary is read once per run.
export const createExecutionSnapshot = (
    interpreter: AjisaiInterpreter,
    userWords: UserWord[] = collectUserWords(interpreter)
): InterpreterSnapshot => ({
    // Carry the lossless snapshot into the worker so exact values on the
    // stack (CodeBlock, ExactScalar) are not flattened by the observation
    // format before this run executes (LANG.OBSERVATION.FIREWALL).
    stackSnapshot: interpreter.snapshot_stack(),
    userWords,
    // Host-configured step budget (LANG.MACHINE.LIMITS water level); undefined
    // keeps the interpreter's own default (`DEFAULT_MAX_EXECUTION_STEPS`).
    stepLimit: getPlatform().executionConfig.stepLimit
});

// What a failed run printed, with its dictionary claims corrected.
//
// `syncInterpreterState` below ignores an ERROR result, so the session keeps
// its pre-run dictionary and every `DEF` the failed run reached is discarded —
// but each of those printed `Defined word: X` on its way through, and those
// lines are still in the output the error path shows. A reader who believes
// them finds out only when `LOOKUP` answers `Unknown word` for something the
// log says exists. The correction goes below them, where it cancels what they
// claimed.
export const describeFailedRunOutput = (result: ExecuteResult): string => {
    const output = result.output || '';
    const discarded = result.discardedDictionaryChanges ?? [];
    if (discarded.length === 0) return output;
    const plural = discarded.length === 1 ? '' : 's';
    const correction =
        `Rolled back ${discarded.length} dictionary change${plural}: ${discarded.join(', ')}. ` +
        'The run failed, so the dictionary is unchanged and the lines above it do not hold.';
    return output ? `${output.replace(/\n*$/, '')}\n${correction}` : correction;
};

// The wall-clock stop is the one refusal the interpreter never gets to
// explain: the playground terminates the worker where it stands. Its guard is
// the host's, not the language's, so it names the host rather than a Word.
export const describeTimeoutDiagnosis = (limitMs: number): string =>
    `${renderErrorReport({ category: 'executionTimeout', word: 'playground' })}\n`
    + `limit executionTimeoutMs: ${limitMs} (wall clock, this host only)`;

export const syncInterpreterState = (
    interpreter: AjisaiInterpreter,
    result: ExecuteResult
): void => {
    if (isFailure(result)) return;
    applyInterpreterSnapshot(interpreter, {
        // The worker's lossless snapshot is what restores the post-run stack
        // into the main-thread interpreter, so it keeps its exact values
        // (LANG.OBSERVATION.FIREWALL).
        stackSnapshot: result.stackSnapshot,
        userWords: result.userWords
    });
};

export const resolveExecutionException = (
    context: string,
    error: unknown,
    showInfo: (text: string, append: boolean) => void,
    showError: (error: Error | string) => void
): void => {
    console.error(`[${context}] Execution failed:`, error);
    if (error instanceof ExecutionAbortedError) {
        showInfo('Execution aborted', true);
        return;
    }
    showError(toError(error));
    // The one refusal the interpreter never gets to explain: it is stopped from
    // outside, so its report is written here instead of arriving with the
    // result.
    if (error instanceof ExecutionTimeoutError) {
        showInfo(describeTimeoutDiagnosis(error.limitMs), true);
    }
};

// ── What a run changed ──────────────────────────────────────────────────────

const toJson = (value: unknown): string => JSON.stringify(value ?? null);

// Whether the stack changed, decided without building a string of it.
//
// A stack is not small by construction: `1 200000 RANGE` is a legal program
// whose one value holds two hundred thousand elements, and serializing it
// twice on every run would cost the better part of a second of frozen main
// thread for an answer that a length mismatch settles immediately. A
// structural walk with an early exit allocates nothing and stops at the first
// difference — which for a run that produced anything is usually the first
// slot it looks at.
const checkValuesEqual = (left: unknown, right: unknown): boolean => {
    if (left === right) return true;
    // null and undefined are the same absence here (as they are in JSON), so
    // the pair is equal rather than a change.
    if (left === null || left === undefined) return right === null || right === undefined;
    if (right === null || right === undefined) return false;
    if (typeof left !== 'object' || typeof right !== 'object') return false;

    if (Array.isArray(left) || Array.isArray(right)) {
        if (!Array.isArray(left) || !Array.isArray(right)) return false;
        if (left.length !== right.length) return false;
        return left.every((element, index) => checkValuesEqual(element, right[index]));
    }

    const leftKeys = Object.keys(left as object);
    const rightKeys = Object.keys(right as object);
    if (leftKeys.length !== rightKeys.length) return false;
    return leftKeys.every(key =>
        Object.prototype.hasOwnProperty.call(right, key)
        && checkValuesEqual(
            (left as Record<string, unknown>)[key],
            (right as Record<string, unknown>)[key]
        ));
};

// Order-insensitive identity of the user dictionary. The pre-execution
// snapshot and the post-execution read-back can enumerate words in different
// orders (a synced interpreter rebuilds its dictionaries from scratch), so the
// set is sorted by name before comparison — otherwise a pure stack op like
// `2 3 ADD` would look like a dictionary change whenever any user word exists,
// and wrongly pull the right column to the Words sheet.
//
// The description is part of the comparison: a `DEF` under a `#:contract`
// line can leave the body as it was and change only the text the Dictionary
// shows for it, and that run still changed the dictionary — the surface is
// redrawn and the session saved only when this says so.
const normalizeUserWords = (words: readonly UserWord[]): string =>
    toJson(
        [...words]
            .map(word => ({
                name: word.name,
                definition: word.definition ?? null,
                description: word.description ?? null
            }))
            .sort((a, b) => a.name.localeCompare(b.name))
    );

// A view of the surfaces an execution can touch, read from one interpreter
// instance so before/after are directly comparable.
export interface ExecutionStateView {
    readonly stack: Value[];
    readonly userWords: UserWord[];
}

export const detectExecutionSurfaceChanges = (
    before: ExecutionStateView,
    after: ExecutionStateView,
    result: ExecuteResult
): ExecutionSurfaceChanges => {
    const userWordsChanged = normalizeUserWords(before.userWords) !== normalizeUserWords(after.userWords);

    // Errors and diagnostics render into the Output surface, so a failed run
    // changes Output even when the program emitted no text of its own.
    const hasError = isFailure(result);

    // Any emission changes Output, an empty or whitespace-only one included:
    // `'' PRINT` writes a line, so the surface it wrote to is shown
    // (spec/gui-semantics.md, "a Run shows each surface it changed").
    return {
        outputChanged: hasError || Boolean(result.output),
        stackChanged: !checkValuesEqual(before.stack, after.stack),
        dictionaryChanged: userWordsChanged
    };
};

