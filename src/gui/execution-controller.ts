import { WORKER_MANAGER } from '../workers/execution-worker-manager';
import type { AjisaiInterpreter, ExecuteResult } from '../wasm-interpreter-types';
import {
    createExecutionSnapshot,
    collectUserWords,
    describeFailedRunOutput,
    detectExecutionSurfaceChanges,
    isFailure,
    renderErrorReport,
    syncInterpreterState,
    resolveExecutionException,
    toError,
    type ExecutionStateView
} from './interpreter-execution-utils';
import { createStepExecutor, StepExecutor } from './step-executor';
import { EXECUTION_TIMEOUT_MS } from '../workers/execution-contract';
import type { ExecutionSurfaceChanges, ViewMode } from './gui-layout-state';

// What one trip through the worker came back with: the interpreter's result,
// or the exception that stopped the run before it could answer (the wall-clock
// guard, Abort, a broken worker). `before` is the snapshot the run was handed.
type ExecutionOutcome =
    | { readonly before: ExecutionStateView; readonly result: ExecuteResult }
    | { readonly error: unknown };

// A run that never answered changed nothing but the report in Output.
const OUTPUT_ONLY_CHANGE: ExecutionSurfaceChanges = {
    outputChanged: true,
    stackChanged: false,
    dictionaryChanged: false
};

const RUN_STATUS_TEXT =
    `Running… Escape stops it; the playground stops it after ${EXECUTION_TIMEOUT_MS / 1000} s.`;

export interface ExecutionCallbacks {
    readonly extractEditorValue: () => string;
    // The editor's text as it stands, read without tidying it: a Run clears
    // the editor only while it still holds the source that was submitted.
    readonly readEditorValue: () => string;
    readonly clearEditor: (switchView?: boolean) => void;
    readonly showInfo: (text: string, append: boolean) => void;
    readonly highlightSourceRange: (start: number, end: number) => void;
    readonly showDocumentation: (text: string) => void;
    readonly showError: (error: Error | string, precedingOutput?: string) => void;
    readonly showExecutionResult: (result: ExecuteResult) => void;
    // Redraw the surfaces `changes` names, from `after` — the post-run view
    // the controller has already read back, so the stack is not collected a
    // second time for the drawing.
    readonly updateDisplays: (after: ExecutionStateView, changes: ExecutionSurfaceChanges) => void;
    readonly saveState: () => Promise<void>;
    readonly fullReset: () => Promise<void>;
    readonly updateView: (mode: ViewMode) => void;
    readonly updateAfterExecution: (changes: ExecutionSurfaceChanges) => void;
    // Say on the Input surface that a run is in progress, or (`null`) that
    // none is. Input is the surface a run is started from, so it is the one
    // on screen while the run has not answered yet.
    readonly showRunStatus: (text: string | null) => void;
}

export interface ExecutionController {
    readonly executeCode: (code: string) => Promise<void>;
    readonly executeReset: () => Promise<void>;
    readonly executeStep: () => Promise<void>;
    readonly checkIsStepModeActive: () => boolean;
    readonly abortExecution: () => void;
    // Look `name` up in the dictionary and show the answer. Bound to
    // `Ctrl+Alt+L`, which supplies the word under the cursor — see
    // `lookupWord` below for why the answer always goes to Output.
    readonly lookupWord: (name: string) => void;
    // Run `task` once every Run, Step and host edit queued before it has
    // finished, so it neither runs against a snapshot a pending run will
    // overwrite nor is undone when that run commits. Stack clear, Delete,
    // Import and the startup restore all go through it.
    readonly runExclusive: <T>(task: () => Promise<T> | T) => Promise<T>;
}

export const createExecutionController = (
    interpreter: AjisaiInterpreter,
    callbacks: ExecutionCallbacks
): ExecutionController => {
    const {
        extractEditorValue,
        readEditorValue,
        clearEditor,
        showInfo,
        highlightSourceRange,
        showDocumentation,
        showError,
        showExecutionResult,
        updateDisplays,
        saveState,
        fullReset,
        updateView,
        updateAfterExecution,
        showRunStatus
    } = callbacks;

    // Answer a lookup from the dictionary for `name`, without running
    // anything. Always answers to the Output area, whether `name` is a Core
    // word (its reference text) or a User word (its reconstructed `DEF`
    // source, shown as read-only reference rather than loaded for editing).
    //
    // The trigger is `Ctrl+Alt+L` at the cursor, which can be anywhere inside
    // a program still being written, so overwriting the Input area here would
    // risk unsaved work; Output is the only destination that is always safe.
    const lookupWord = (name: string): void => {
        if (!name) return;
        const found = interpreter.resolve_host_lookup(name);
        // An unknown word is an answer too, and it goes where every answer
        // goes (spec/gui-semantics.md, Lookup).
        if (found) {
            showDocumentation(found.text);
        } else {
            showError(`Unknown word: ${name}`);
        }
        updateView('output');
    };

    // The report half of a run's answer: what it printed or the error it
    // stopped with, and under the error its category, Word and position —
    // written *under* the error rather than before it, because `showError`
    // clears the area and would erase anything drawn ahead of it.
    const reportExecutionResult = (result: ExecuteResult): void => {
        if (isFailure(result)) {
            // Keep whatever the run printed before it failed: the host reports
            // it on the error path, and the error is written below it.
            showError(result.message || 'Unknown error', describeFailedRunOutput(result));
            if (result.errorInfo) showInfo(renderErrorReport(result.errorInfo), true);
            return;
        }
        showExecutionResult(result);
    };

    // Everything a run's answer does to the page, in one place: the report in
    // Output, the redraw, the move to the surfaces
    // the run changed, and the save. Run and Step both come through here, and
    // so does a run that never answered — stopped by the wall-clock guard or by
    // Abort — so each of them is reported and surfaced the same way.
    //
    // The post-run surfaces are read back from the SAME interpreter instance
    // (already updated by syncInterpreterState) so they are compared
    // like-for-like with the snapshot the run was handed. Comparing against the
    // worker's `result` instead skews the dictionary comparison across
    // instances and misfires on every run.
    //
    // The redraw and the save follow that comparison: a surface the run did
    // not change is not drawn again, and a run that changed neither the stack
    // nor the dictionary — a failed one, whose state `syncInterpreterState`
    // never applied, or one that only printed — has nothing to save. Every
    // run used to rebuild both panels and serialize the session regardless.
    //
    // `submittedSource` is the editor text a Run was started from. A run that
    // succeeds clears the editor only if it still holds that text: whatever
    // was typed while the run was going is the next program, not this one.
    const applyExecutionResult = async (
        context: string,
        outcome: ExecutionOutcome,
        submittedSource: string | null
    ): Promise<boolean> => {
        let changes: ExecutionSurfaceChanges = OUTPUT_ONLY_CHANGE;
        let after: ExecutionStateView | null = null;
        let succeeded = false;

        try {
            if ('error' in outcome) throw outcome.error;
            const { before, result } = outcome;
            try {
                syncInterpreterState(interpreter, result);
            } catch (error) {
                console.error(`[${context}] Failed to sync state:`, error);
                showError(toError(error));
            }
            after = {
                stack: interpreter.collect_stack(),
                userWords: collectUserWords(interpreter)
            };
            reportExecutionResult(result);
            succeeded = !isFailure(result);
            if (succeeded && submittedSource !== null && readEditorValue() === submittedSource) {
                clearEditor(false);
            }
            changes = detectExecutionSurfaceChanges(before, after, result);
        } catch (error) {
            resolveExecutionException(context, error, showInfo, showError);
        }

        if (after && (changes.stackChanged || changes.dictionaryChanged)) {
            updateDisplays(after, changes);
            await saveState();
        }
        updateAfterExecution(changes);
        return succeeded;
    };

    // The session is used by one operation at a time. A run is handed a
    // snapshot of the session and its answer replaces the session whole, so a
    // second run started before the first answered was handed the same
    // snapshot and its answer overwrote the first's, and a Stack clear,
    // Delete or Import made in between was undone when the run committed.
    // Each operation therefore starts once the one before it has finished —
    // at once when there is none.
    let sessionTail: Promise<void> = Promise.resolve();
    let pendingOperations = 0;
    const runExclusive = <T>(task: () => Promise<T> | T): Promise<T> => {
        const start = async (): Promise<T> => task();
        const operation = pendingOperations === 0 ? start() : sessionTail.then(start);
        pendingOperations += 1;
        sessionTail = operation
            .then(() => undefined, () => undefined)
            .then(() => { pendingOperations -= 1; });
        return operation;
    };

    // Abort and Reset also drop the runs still waiting for their turn: Escape
    // stops what was asked for, not only the part of it already started.
    let runGeneration = 0;

    // One execution path: a run is one task on one worker, and its answer —
    // or the exception that stood in for one — goes to `applyExecutionResult`.
    const executeSource = (
        context: string,
        code: string,
        submittedSource: string | null
    ): Promise<boolean> => {
        const generation = runGeneration;
        return runExclusive(async () => {
            if (generation !== runGeneration) return false;
            let outcome: ExecutionOutcome;
            showRunStatus(RUN_STATUS_TEXT);
            try {
                // The main thread's own view of the run's start, for the
                // before/after comparison; the worker is handed the lossless
                // snapshot instead, and the dictionary is read once for both.
                const before: ExecutionStateView = {
                    stack: interpreter.collect_stack(),
                    userWords: collectUserWords(interpreter)
                };
                const snapshot = createExecutionSnapshot(interpreter, before.userWords);
                outcome = { before, result: await WORKER_MANAGER.execute(code, snapshot) };
            } catch (error) {
                outcome = { error };
            } finally {
                showRunStatus(null);
            }
            return applyExecutionResult(context, outcome, submittedSource);
        });
    };

    const stepExecutor: StepExecutor = createStepExecutor({
        extractEditorValue,
        showInfo,
        highlightSourceRange,
        executeSource: (code) => executeSource('StepExecutor', code, null)
    });

    const executeCode = async (code: string): Promise<void> => {
        if (!code) return;

        stepExecutor.reset();
        showInfo('Executing...', false);
        await executeSource('ExecController', code, code);
    };

    const executeReset = async (): Promise<void> => {
        try {
            console.log('[ExecController] Executing full reset');
            runGeneration += 1;
            stepExecutor.reset();
            await WORKER_MANAGER.resetAllWorkers();
            const result = interpreter.reset();

            if (!isFailure(result)) {
                clearEditor(true);
                await fullReset();
                updateView('input');
            } else {
                showError(result.message || 'RESET execution failed');
            }
        } catch (error) {
            console.error('[ExecController] Reset failed:', error);
            showError(toError(error));
        }
    };

    // Abort stops the run where it stands — the worker carrying it is
    // replaced, exactly as the wall-clock guard does — and the run answers
    // through `applyExecutionResult` like any other that did not complete.
    const abortExecution = (): void => {
        runGeneration += 1;
        WORKER_MANAGER.abortAll();
        stepExecutor.abort();
    };

    return {
        executeCode,
        executeReset,
        executeStep: stepExecutor.executeStep,
        checkIsStepModeActive: stepExecutor.isActive,
        abortExecution,
        lookupWord,
        runExclusive
    };
};
