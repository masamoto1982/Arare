// AR-VER-303: switching between Run and step execution (AR-LLR-036): the
// switch decides what the Execute button does, says so on the button, and is
// remembered; and step mode runs the engine's own pieces, refusing BIND only
// where names do not outlive a step.

import { describe, expect, test } from 'vitest';
import {
    EXECUTION_MODE_STORAGE_KEY,
    createExecutionModeSwitch,
    describeExecutionMode,
    readExecutionMode
} from './execution-mode';
import { createStepExecutor, type StepToken } from './step-executor';

// Just enough of a button for the switch: attributes, text and click.
class FakeButton {
    textContent = '';
    title = '';
    readonly attributes = new Map<string, string>();
    private readonly listeners: Array<() => void> = [];
    setAttribute(name: string, value: string): void { this.attributes.set(name, value); }
    addEventListener(_: string, listener: () => void): void { this.listeners.push(listener); }
    click(): void { this.listeners.forEach((l) => l()); }
}

const setup = (stored: string | null) => {
    const store = new Map<string, string>(stored === null ? [] : [[EXECUTION_MODE_STORAGE_KEY, stored]]);
    const storage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => { store.set(k, v); } };
    const [runRadio, stepRadio, executeButton] = [new FakeButton(), new FakeButton(), new FakeButton()];
    const calls: string[] = [];
    const modeSwitch = createExecutionModeSwitch(
        { runRadio, stepRadio, executeButton } as unknown as Parameters<typeof createExecutionModeSwitch>[0],
        { run: () => calls.push('run'), step: () => calls.push('step') },
        storage
    );
    return { modeSwitch, runRadio, stepRadio, executeButton, calls, store };
};

describe('AR-VER-303: the Run/Step switch', () => {
    test('starts in the remembered mode, Run when there is none', () => {
        expect(readExecutionMode(null)).toBe('run');
        expect(readExecutionMode('step')).toBe('step');
        expect(readExecutionMode('walk')).toBe('run');
        expect(setup(null).modeSwitch.mode()).toBe('run');
        expect(setup('step').modeSwitch.mode()).toBe('step');
    });

    test('the button does what the mode says and is labelled so; the choice is remembered', () => {
        const { modeSwitch, runRadio, stepRadio, executeButton, calls, store } = setup(null);
        expect(executeButton.textContent).toBe('Run');
        expect(runRadio.attributes.get('aria-checked')).toBe('true');
        executeButton.click();
        stepRadio.click();
        expect(modeSwitch.mode()).toBe('step');
        expect(executeButton.textContent).toBe('Step');
        expect(executeButton.title).toBe(describeExecutionMode('step').title);
        expect(stepRadio.attributes.get('aria-checked')).toBe('true');
        expect(runRadio.attributes.get('aria-checked')).toBe('false');
        executeButton.click();
        modeSwitch.execute();
        expect(calls).toEqual(['run', 'step', 'step']);
        expect(store.get(EXECUTION_MODE_STORAGE_KEY)).toBe('step');
        runRadio.click();
        expect(store.get(EXECUTION_MODE_STORAGE_KEY)).toBe('run');
    });

    test('works without storage, and when storage throws', () => {
        const [runRadio, stepRadio, executeButton] = [new FakeButton(), new FakeButton(), new FakeButton()];
        const throwing = { getItem: () => { throw new Error('denied'); }, setItem: () => { throw new Error('denied'); } };
        const s = createExecutionModeSwitch(
            { runRadio, stepRadio, executeButton } as unknown as Parameters<typeof createExecutionModeSwitch>[0],
            { run: () => undefined, step: () => undefined },
            throwing
        );
        stepRadio.click();
        expect(s.mode()).toBe('step');
        expect(createExecutionModeSwitch(
            { runRadio, stepRadio, executeButton } as unknown as Parameters<typeof createExecutionModeSwitch>[0],
            { run: () => undefined, step: () => undefined },
            null
        ).mode()).toBe('run');
    });
});

const stepper = (source: string, split: StepToken[] | null, namesAcrossSteps: boolean) => {
    const executed: string[] = [];
    const info: string[] = [];
    const executor = createStepExecutor({
        extractEditorValue: () => source,
        showInfo: (text) => { info.push(text); },
        highlightSourceRange: () => undefined,
        executeSource: async (code) => { executed.push(code); return true; },
        splitSteps: () => split,
        namesAcrossSteps
    });
    return { executor, executed, info };
};

describe("AR-VER-303: step mode on the engine's pieces", () => {
    test('runs the pieces the engine gives, and BIND where names outlive a step', async () => {
        const source = "0.75 8 -2 QUANTIZE 'N' BIND N";
        const pieces = [[0, 18], [19, 22], [23, 27], [28, 29]].map(([start, end]) => ({ text: source.slice(start, end), start: start!, end: end! }));
        const { executor, executed, info } = stepper(source, pieces, true);
        for (let i = 0; i < 4; i++) await executor.executeStep();
        expect(executed).toEqual(['0.75 8 -2 QUANTIZE', "'N'", 'BIND', 'N']);
        expect(info.at(-1)).toBe('[DONE] Step mode completed');
    });

    test('refuses BIND where names do not outlive a step', async () => {
        const source = "5 'N' BIND N";
        const { executor, executed, info } = stepper(source, null, false);
        // A null split means a syntax error: the whole source runs once, and
        // reports it as Run does.
        await executor.executeStep();
        expect(executed).toEqual([source]);
        const plain = createStepExecutor({
            extractEditorValue: () => source,
            showInfo: (text) => { info.push(text); },
            highlightSourceRange: () => undefined,
            executeSource: async (code) => { executed.push(code); return true; }
        });
        await plain.executeStep();
        expect(info.at(-1)).toContain('Step mode cannot run a program that uses BIND');
        expect(executed).toEqual([source]);
    });
});
