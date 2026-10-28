// Run, Step, the wall-clock guard, Abort and Lookup answer through one path
// (`applyExecutionResult` in execution-controller.ts): the report and its
// error go to Output, and the surfaces the answer changed are shown. These
// pin that each of them takes it, rather than a copy that drifts — a failing
// Step that printed no error report and never left the editor, and an Abort
// that could not stop a run, were each such a copy.
//
// The worker pool is replaced by a fake whose `execute` the test answers by
// hand; the interpreter is the minimal fake the snapshot round trip needs.

import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
    AjisaiInterpreter,
    ExecuteResult,
    UserWord,
    Value
} from '../wasm-interpreter-types';
import type { ExecutionSurfaceChanges } from './gui-layout-state';
import { ExecutionTimeoutError } from '../workers/execution-contract';
import { ExecutionAbortedError, type InterpreterSnapshot } from '../workers/execution-contract';
import { createExecutionController } from './execution-controller';
import { num } from '../test-support';

type Answer = (code: string, state: InterpreterSnapshot) => Promise<ExecuteResult>;

const pool = vi.hoisted(() => ({
    answer: null as null | ((code: string, state: any) => Promise<unknown>),
    abortAll: (): void => { /* replaced per test */ }
}));

vi.mock('../workers/execution-worker-manager', () => ({
    WORKER_MANAGER: {
        execute: (code: string, state: unknown) => pool.answer!(code, state),
        abortAll: () => pool.abortAll(),
        resetAllWorkers: async () => { /* not exercised */ }
    }
}));

// The main-thread interpreter: a stack and a User dictionary, restored from a
// result's snapshot the way the wasm one is.
const createFakeInterpreter = () => {
    let stack: Value[] = [];
    const words = new Map<string, string>();
    const fake: Partial<AjisaiInterpreter> = {
        collect_stack: () => stack,
        collect_user_words_info: () =>
            [...words.keys()].map(name => [name, false] as [string, boolean]),
        lookup_word_definition: (name: string) => words.get(name) ?? null,
        lookup_word_description: () => null,
        snapshot_stack: () => JSON.stringify(stack),
        restore_stack_snapshot: (snapshot: string) => { stack = JSON.parse(snapshot) as Value[]; },
        restore_user_words: (restored: UserWord[]) => {
            for (const word of restored) if (word.definition) words.set(word.name, word.definition);
            return [] as Array<[string, string]>;
        },
        reset: () => {
            stack = [];
            words.clear();
            return { status: 'OK' } as ExecuteResult;
        },
        set_max_execution_steps: () => { /* not modelled */ },
        resolve_host_lookup: (name: string) =>
            name === 'DIV' ? { kind: 'documentation', text: 'DIV — divide values' } : null
    };
    return {
        interpreter: fake as AjisaiInterpreter,
        setStack: (next: Value[]) => { stack = next; },
        words
    };
};

// An OK answer that leaves `stack` behind, as the worker's would.
const ok = (stack: Value[], overrides: Partial<ExecuteResult> = {}): ExecuteResult => ({
    status: 'OK',
    output: '',
    stackSnapshot: JSON.stringify(stack),
    userWords: [],
    ...overrides
});

const setup = () => {
    const fake = createFakeInterpreter();
    const log: string[] = [];
    const surfaces: ExecutionSurfaceChanges[] = [];
    const views: string[] = [];
    const runStatus: (string | null)[] = [];
    let editor = '';
    const controller = createExecutionController(fake.interpreter, {
        extractEditorValue: () => editor,
        readEditorValue: () => editor,
        clearEditor: () => { log.push('clearEditor'); },
        showInfo: (text, append) => { log.push(`${append ? 'info+' : 'info'}: ${text}`); },
        highlightSourceRange: () => { /* not observed */ },
        showDocumentation: (text) => { log.push(`doc: ${text}`); },
        showError: (error) => {
            log.push(`error: ${error instanceof Error ? error.message : error}`);
        },
        showExecutionResult: () => { log.push('result'); },
        updateDisplays: (_after, changes) => {
            log.push(`redraw: ${[
                changes.stackChanged ? 'stack' : null,
                changes.dictionaryChanged ? 'dictionary' : null
            ].filter(Boolean).join('+')}`);
        },
        saveState: async () => { log.push('save'); },
        fullReset: async () => { /* not observed */ },
        updateView: (mode) => { views.push(mode); },
        updateAfterExecution: (changes) => { surfaces.push(changes); },
        showRunStatus: (text) => { runStatus.push(text); }
    });
    return {
        ...fake,
        controller,
        log,
        surfaces,
        views,
        runStatus,
        setEditor: (text: string) => { editor = text; }
    };
};

const answerWith = (answer: Answer): void => { pool.answer = answer; };

beforeEach(() => {
    pool.answer = null;
    pool.abortAll = () => { /* nothing running */ };
});

describe('Run', () => {
    it('says it is running on the Input surface until the run answers', async () => {
        const page = setup();
        answerWith(async () => ok([num(1)]));

        await page.controller.executeCode('1');

        expect(page.runStatus).toHaveLength(2);
        expect(page.runStatus[0]).toContain('Escape');
        expect(page.runStatus[1]).toBeNull();
    });
});

describe('Step', () => {
    it('reports a failing step with its error, keeps the step line, and shows Output', async () => {
        const page = setup();
        page.setEditor('1 FOO');
        let calls = 0;
        answerWith(async () => {
            calls += 1;
            if (calls === 1) return ok([num(1)]);
            return {
                status: 'ERROR',
                error: true,
                message: 'Unknown word: FOO',
                errorInfo: { category: 'unknownWord', word: 'FOO', line: 1, column: 3 }
            };
        });

        await page.controller.executeStep();
        await page.controller.executeStep();

        const failed = page.log.slice(page.log.indexOf('error: Unknown word: FOO'));
        expect(failed[1]).toBe('info+: [ERROR] unknownWord in FOO at line 1, column 3');
        expect(failed[2]).toMatch(/^info\+: \[>\] Step 2\/2/);
        expect(page.surfaces.at(-1)?.outputChanged).toBe(true);
        expect(page.controller.checkIsStepModeActive()).toBe(false);
    });

    it('writes the step line after the answer rather than before it', async () => {
        const page = setup();
        page.setEditor('1 2');
        answerWith(async () => ok([num(1)]));

        await page.controller.executeStep();

        expect(page.log.indexOf('result')).toBeLessThan(
            page.log.findIndex(line => line.startsWith('info+: [>] Step 1/2'))
        );
    });
});

describe('a run that never answered', () => {
    it('reports the wall-clock stop with its error and shows Output', async () => {
        const page = setup();
        answerWith(async () => { throw new ExecutionTimeoutError(5_000); });

        await page.controller.executeCode('loop');

        expect(page.log).toContain('error: Execution timed out after 5000 ms');
        expect(page.log.some(line => line.includes('[ERROR] executionTimeout'))).toBe(true);
        expect(page.surfaces.at(-1)).toEqual({
            outputChanged: true,
            stackChanged: false,
            dictionaryChanged: false
        });
        expect(page.runStatus.at(-1)).toBeNull();
    });

    it('stops on Abort through the pool and reports it the same way', async () => {
        const page = setup();
        answerWith(() => new Promise<ExecuteResult>((_, reject) => {
            pool.abortAll = () => reject(new ExecutionAbortedError());
        }));

        const run = page.controller.executeCode('loop');
        page.controller.abortExecution();
        await run;

        expect(page.log).toContain('info+: Execution aborted');
        expect(page.surfaces.at(-1)?.outputChanged).toBe(true);
    });
});

// The redraw and the save follow what the run changed: both panels used to
// be rebuilt and the session serialized on every run, including a failed one
// whose state was never applied.
describe('Redraw and save', () => {
    it('redraws only the stack and saves after a pure stack op', async () => {
        const page = setup();
        answerWith(async () => ok([num(1)]));

        await page.controller.executeCode('1');

        expect(page.log).toContain('redraw: stack');
        expect(page.log).toContain('save');
    });

    it('redraws the dictionary too when a word is defined', async () => {
        const page = setup();
        answerWith(async () => ok([], {
            output: 'Defined word: G\n',
            userWords: [{ name: 'G', definition: '2 MUL' }]
        }));

        await page.controller.executeCode("[ 2 MUL ] 'G' DEF");

        expect(page.log).toContain('redraw: dictionary');
        expect(page.log).toContain('save');
    });

    it('neither redraws nor saves after a failed run, whose state was not applied', async () => {
        const page = setup();
        answerWith(async () => ok([num(1)], { status: 'ERROR', error: true, message: 'boom' }));

        await page.controller.executeCode('1 NOPE');

        expect(page.log.some(line => line.startsWith('redraw'))).toBe(false);
        expect(page.log).not.toContain('save');
    });

    it('neither redraws nor saves after a run that only printed', async () => {
        const page = setup();
        answerWith(async () => ok([], { output: 'hello\n' }));

        await page.controller.executeCode("'hello' PRINT");

        expect(page.log.some(line => line.startsWith('redraw'))).toBe(false);
        expect(page.log).not.toContain('save');
    });

    it('neither redraws nor saves after a run the wall-clock guard stopped', async () => {
        const page = setup();
        answerWith(async () => { throw new ExecutionTimeoutError(5000); });

        await page.controller.executeCode('loop');

        expect(page.log.some(line => line.startsWith('redraw'))).toBe(false);
        expect(page.log).not.toContain('save');
    });
});

describe('Lookup', () => {
    it('answers a known word in Output', () => {
        const page = setup();
        page.controller.lookupWord('DIV');
        expect(page.log).toEqual(['doc: DIV — divide values']);
        expect(page.views).toEqual(['output']);
    });

    it('answers an unknown word in Output too', () => {
        const page = setup();
        page.controller.lookupWord('NOPE');
        expect(page.log).toEqual(['error: Unknown word: NOPE']);
        expect(page.views).toEqual(['output']);
    });
});

// The session is used by one operation at a time. Every run used to be handed
// the snapshot of the moment it was asked for, and its answer replaced the
// session whole: a second Run before the first answered lost the first's
// effect, and a Stack clear or Import made meanwhile was undone when the run
// answered. Here the pool runs each task against the snapshot it was handed
// and answers when the test releases it.
describe('Overlapping operations', () => {
    const tick = () => new Promise(resolve => setTimeout(resolve, 0));
    const holdAnswers = () => {
        const held: Array<() => void> = [];
        answerWith((code, state) => new Promise(resolve => held.push(() => {
            const before = JSON.parse(state.stackSnapshot ?? '[]') as Value[];
            resolve(ok([...before, num(Number(code))]));
        })));
        return held;
    };

    it('runs a second Run after the first has answered, from what it left', async () => {
        const page = setup();
        const held = holdAnswers();

        const first = page.controller.executeCode('1');
        const second = page.controller.executeCode('2');
        await tick();
        expect(held).toHaveLength(1);
        held.shift()!();
        await first;
        await tick();
        held.shift()!();
        await second;

        expect(page.interpreter.collect_stack()).toEqual([num(1), num(2)]);
    });

    it('applies a host edit made during a run after the run, not under it', async () => {
        const page = setup();
        page.setStack([num(7)]);
        const held = holdAnswers();

        const run = page.controller.executeCode('9');
        await tick();
        const edits = Promise.all([
            page.controller.runExclusive(() => page.setStack([])),
            page.controller.runExclusive(() => { page.words.set('IMPORTED', '1'); })
        ]);
        held.shift()!();
        await run;
        await edits;

        expect(page.interpreter.collect_stack()).toEqual([]);
        expect([...page.words.keys()]).toEqual(['IMPORTED']);
    });

    it('keeps text typed into the editor while the run was going', async () => {
        const page = setup();
        const held = holdAnswers();
        page.setEditor('1');

        const run = page.controller.executeCode('1');
        await tick();
        page.setEditor('2 3 ADD');
        held.shift()!();
        await run;

        expect(page.log).not.toContain('clearEditor');
    });

    it('clears the editor after a run when it still holds the submitted source', async () => {
        const page = setup();
        answerWith(async () => ok([num(1)]));
        page.setEditor('1');

        await page.controller.executeCode('1');

        expect(page.log).toContain('clearEditor');
    });

    it('drops the runs still waiting when Abort stops the one in flight', async () => {
        const page = setup();
        const dispatched: string[] = [];
        answerWith((code) => new Promise<ExecuteResult>((_, reject) => {
            dispatched.push(code);
            pool.abortAll = () => reject(new ExecutionAbortedError());
        }));

        const first = page.controller.executeCode('1');
        const second = page.controller.executeCode('2');
        page.controller.abortExecution();
        await Promise.all([first, second]);

        expect(dispatched).toEqual(['1']);
    });
});
