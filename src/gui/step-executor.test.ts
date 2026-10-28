// Step mode's hold on the editor's selection: it selects the piece about to
// run, and collapses that selection when it ends. Run and Reset also end step
// mode, and used to collapse the selection whether or not step mode had drawn
// one — which moved the caret to the start of the text on every Run, so a Run
// that failed left its text in place but the caret at the top of it.

import { describe, expect, test } from 'vitest';
import { createStepExecutor, tokenizeWithOffsets } from './step-executor';

const setup = (source: string, completes: boolean = true) => {
    const highlights: Array<[number, number]> = [];
    const executed: string[] = [];
    const info: string[] = [];
    const executor = createStepExecutor({
        extractEditorValue: () => source,
        showInfo: (text) => { info.push(text); },
        highlightSourceRange: (start, end) => { highlights.push([start, end]); },
        executeSource: async (code) => { executed.push(code); return completes; }
    });
    return { executor, highlights, executed, info };
};

describe('createStepExecutor', () => {
    test('reset outside step mode leaves the selection alone', () => {
        const { executor, highlights } = setup('1 2 ADD');
        executor.reset();
        executor.abort();
        expect(highlights).toEqual([]);
        expect(executor.isActive()).toBe(false);
    });

    test('selects each piece before running it and collapses the selection at the end', async () => {
        const { executor, highlights, executed } = setup('1 [ 2 ] ADD');
        await executor.executeStep();
        expect(executor.isActive()).toBe(true);
        await executor.executeStep();
        await executor.executeStep();
        expect(executed).toEqual(['1', '[ 2 ]', 'ADD']);
        expect(highlights).toEqual([[0, 1], [2, 7], [8, 11], [0, 0]]);
        expect(executor.isActive()).toBe(false);
    });

    test('a Run during step mode ends it and collapses its selection', async () => {
        const { executor, highlights } = setup('1 2');
        await executor.executeStep();
        executor.reset();
        expect(highlights).toEqual([[0, 1], [0, 0]]);
        expect(executor.isActive()).toBe(false);
    });

    test('a step that fails ends step mode and collapses its selection', async () => {
        const { executor, highlights } = setup('1 DIV', false);
        await executor.executeStep();
        expect(highlights).toEqual([[0, 1], [0, 0]]);
        expect(executor.isActive()).toBe(false);
    });
});

// ── merged from step-tokens.test.ts ──

// Step mode reports which piece it is about to run, and the editor points at
// it. That only works if each piece carries where it sits in the source: a
// piece's text can repeat, so searching the source for it would land on the
// wrong occurrence.
//
// The other half of what these tests hold is that a piece can be *executed on
// its own*. Step mode runs each piece separately against the persisted state,
// so a split that cuts a vector in half does not merely look odd — it hands
// the interpreter an unclosed bracket and ends step mode on a source error.


describe('tokenizeWithOffsets', () => {
    test('splits bracket-free source on whitespace, as step mode always has', () => {
        expect(tokenizeWithOffsets('1 2 ADD').map((t) => t.text)).toEqual(['1', '2', 'ADD']);
    });

    test('each token carries the range it occupies', () => {
        const tokens = tokenizeWithOffsets('1 2 ADD');
        expect(tokens.map((t) => [t.start, t.end])).toEqual([
            [0, 1],
            [2, 3],
            [4, 7]
        ]);
    });

    test('a repeated token gets its own occurrence, not the first one', () => {
        const tokens = tokenizeWithOffsets('ADD 1 ADD');
        expect(tokens[2]).toEqual({ text: 'ADD', start: 6, end: 9 });
    });

    test('newlines and runs of spaces are boundaries like any other whitespace', () => {
        const tokens = tokenizeWithOffsets('1\n\n  2');
        expect(tokens).toEqual([
            { text: '1', start: 0, end: 1 },
            { text: '2', start: 5, end: 6 }
        ]);
    });

    test('empty and whitespace-only source yields no tokens', () => {
        expect(tokenizeWithOffsets('')).toEqual([]);
        expect(tokenizeWithOffsets('  \n ')).toEqual([]);
    });

    // `[ 1 ] [ 2 ] ADD` must not split to `[`, `1`, `]`, …: the first step
    // alone would be `Unclosed '[': expected ']'`, which resets step mode
    // before anything runs. `[ 42 ]` is the idiomatic scalar, so this is very
    // nearly every real program.
    test('a vector is one step, not a bracket and its contents', () => {
        expect(tokenizeWithOffsets('[ 1 ] [ 2 ] ADD').map((t) => t.text)).toEqual([
            '[ 1 ]',
            '[ 2 ]',
            'ADD'
        ]);
    });

    test('a nested vector is still one step', () => {
        expect(tokenizeWithOffsets('[ [ 1 ] [ 2 3 ] ] LENGTH').map((t) => t.text)).toEqual([
            '[ [ 1 ] [ 2 3 ] ]',
            'LENGTH'
        ]);
    });

    test('a code block is a vector, so a definition steps in three', () => {
        expect(tokenizeWithOffsets("[ [ 1 ] ADD ] 'INC' DEF").map((t) => t.text)).toEqual([
            '[ [ 1 ] ADD ]',
            "'INC'",
            'DEF'
        ]);
    });

    test('a multi-line vector is one step, interior line breaks and all', () => {
        const tokens = tokenizeWithOffsets('[ 1\n  2 ]\nLENGTH');
        expect(tokens.map((t) => t.text)).toEqual(['[ 1\n  2 ]', 'LENGTH']);
    });

    test('a string holds its own whitespace rather than splitting', () => {
        expect(tokenizeWithOffsets("'hello world' PRINT").map((t) => t.text)).toEqual([
            "'hello world'",
            'PRINT'
        ]);
    });

    test('a bracket inside a string is text, not nesting', () => {
        expect(tokenizeWithOffsets("'[ 1' PRINT").map((t) => t.text)).toEqual(["'[ 1'", 'PRINT']);
    });

    test('a comment is not a step', () => {
        expect(tokenizeWithOffsets('# note\n1 2 ADD').map((t) => t.text)).toEqual(['1', '2', 'ADD']);
    });

    test('a hash glued to a name is part of that name, not a comment', () => {
        expect(tokenizeWithOffsets('a#b 1').map((t) => t.text)).toEqual(['a#b', '1']);
    });

    test('a quote glued to a name is part of that name, not a string', () => {
        expect(tokenizeWithOffsets("a'b c").map((t) => t.text)).toEqual(["a'b", 'c']);
    });

    // Malformed source is passed through rather than repaired: the reader gets
    // the same error a plain run would give, against the text they wrote.
    test('an unclosed bracket becomes one final step, not a silent repair', () => {
        expect(tokenizeWithOffsets('1 [ 2 3').map((t) => t.text)).toEqual(['1', '[ 2 3']);
    });

    test('a stray closing bracket is its own step', () => {
        expect(tokenizeWithOffsets('1 ] 2').map((t) => t.text)).toEqual(['1', ']', '2']);
    });

    test('offsets still address the exact occurrence for a repeated vector', () => {
        const tokens = tokenizeWithOffsets('[ 1 ] [ 1 ]');
        expect(tokens[1]).toEqual({ text: '[ 1 ]', start: 6, end: 11 });
    });

    test('every piece is exactly the source it points at', () => {
        const code = "[ 1 ] [ [ 2 ] 'x' ] ADD";
        for (const token of tokenizeWithOffsets(code)) {
            expect(code.slice(token.start, token.end)).toBe(token.text);
        }
    });
});

// Ctrl+Enter pressed while a piece is still running used to read the index
// that piece had not advanced yet, and ran it a second time; and a piece still
// running when Run ended its step mode advanced the step mode started after.
describe('a step still running', () => {
    const harness = (source: () => string) => {
        const executed: string[] = [];
        const held: Array<(completed: boolean) => void> = [];
        let holding = true;
        const executor = createStepExecutor({
            extractEditorValue: source,
            showInfo: () => { /* not observed */ },
            highlightSourceRange: () => { /* not observed */ },
            executeSource: (code) => {
                executed.push(code);
                return holding ? new Promise<boolean>(resolve => held.push(resolve)) : Promise.resolve(true);
            }
        });
        return { executor, executed, held, release: () => { holding = false; } };
    };

    test('ignores Ctrl+Enter until it has answered', async () => {
        const h = harness(() => '1 2 ADD');
        const first = h.executor.executeStep();
        const pressedAgain = h.executor.executeStep();
        h.held.forEach(resolve => resolve(true));
        await Promise.all([first, pressedAgain]);
        h.release();
        await h.executor.executeStep();
        await h.executor.executeStep();
        expect(h.executed).toEqual(['1', '2', 'ADD']);
    });

    test('does not advance a step mode started after its own ended', async () => {
        let source = '10 20';
        const h = harness(() => source);
        const old = h.executor.executeStep();
        h.executor.reset();
        source = '1 2 3';
        const fresh = h.executor.executeStep();
        h.held[0]!(true);
        await old;
        h.held[1]!(true);
        await fresh;
        h.release();
        await h.executor.executeStep();
        expect(h.executed).toEqual(['10', '1', '2']);
    });
});

// Step mode runs each piece as a program of its own, so it must not give a
// program another meaning than Run does.
describe('what a stepped program means', () => {
    // A binding lasts for the rest of its frame, and the frame ends with the
    // piece: stepped, `N` was an unknown word by the time it was read.
    test('refuses a program that binds a name, rather than stepping it', async () => {
        const { executor, executed, info } = setup("5 'N' BIND N N ADD");
        await executor.executeStep();
        expect(executed).toEqual([]);
        expect(executor.isActive()).toBe(false);
        expect(info.at(-1)).toMatch(/BIND/);
    });

    test('a BIND inside a vector or a string is not a binding at the top level', async () => {
        const { executor, executed } = setup("[ 'N' BIND ] 'BIND' PRINT");
        await executor.executeStep();
        expect(executed).toEqual(["[ 'N' BIND ]"]);
    });

    test('runs a #:contract line with the DEF it describes', async () => {
        const source = "#:contract DOUBLE inputs=1 outputs=1 field=closed\n[ [ 2 ] MUL ] 'DOUBLE' DEF";
        const { executor, executed } = setup(source);
        for (let step = 0; step < 3; step++) await executor.executeStep();
        expect(executed).toEqual([
            '[ [ 2 ] MUL ]',
            "'DOUBLE'",
            '#:contract DOUBLE inputs=1 outputs=1 field=closed\nDEF'
        ]);
    });

    test('a plain comment still is not a step, and travels with nothing', () => {
        const tokens = tokenizeWithOffsets("# note\n[ 1 ] 'ONE' DEF");
        expect(tokens.map((t) => t.text)).toEqual(['[ 1 ]', "'ONE'", 'DEF']);
        expect(tokens.every((t) => t.contracts === undefined)).toBe(true);
    });
});
