// Adversarial robustness for snapshot application. The parameter is an
// explicitly partial/untrusted snapshot, so malformed objects must be
// tolerated rather than throw and abort the whole restore.

import { describe, expect, test, vi } from 'vitest';
import { applyInterpreterSnapshot } from './execution-contract';
import type { AjisaiInterpreter } from '../wasm-interpreter-types';

const makeMock = () => {
    const fns = {
        reset: vi.fn(() => ({})),
        restore_stack_snapshot: vi.fn(),
        restore_user_words: vi.fn(() => []),
        set_max_execution_steps: vi.fn(),
    };
    return { fns, interpreter: fns as unknown as AjisaiInterpreter };
};

describe('applyInterpreterSnapshot robustness', () => {
    const malformed: unknown[] = [
        null, undefined, {},
        { userWords: 'not-an-array' },
        { stackSnapshot: 123 },
    ];
    for (const snapshot of malformed) {
        test(`never throws on ${JSON.stringify(snapshot)}`, () => {
            const { interpreter } = makeMock();
            expect(() => applyInterpreterSnapshot(interpreter, snapshot as never)).not.toThrow();
        });
    }



    test('applies a positive integer stepLimit', () => {
        const { fns, interpreter } = makeMock();
        applyInterpreterSnapshot(interpreter, { stepLimit: 1_000_000 } as never);
        expect(fns.set_max_execution_steps).toHaveBeenCalledWith(1_000_000);
    });

    for (const stepLimit of [0, -1, 1.5, NaN, Infinity, '100000', null]) {
        test(`ignores invalid stepLimit ${String(stepLimit)}`, () => {
            const { fns, interpreter } = makeMock();
            applyInterpreterSnapshot(interpreter, { stepLimit } as never);
            expect(fns.set_max_execution_steps).not.toHaveBeenCalled();
        });
    }



    // The session reset keeps the cross-reset artifact cache, so an unchanged
    // user word's compiled plan is reused instead of recompiled.
    test('resets the session before restoring', () => {
        const { fns, interpreter } = makeMock();
        applyInterpreterSnapshot(interpreter, null);
        expect(fns.reset).toHaveBeenCalledTimes(1);
    });
});

// The worker round-trip must be lossless: the `stackSnapshot` string is the one
// format restored, and the lossy observation `stack` is never read back. A
// snapshot without one leaves the stack empty rather than downgrading.
describe('applyInterpreterSnapshot restores only the lossless snapshot', () => {
    test('restores from stackSnapshot and ignores the observation stack', () => {
        const { fns, interpreter } = makeMock();
        applyInterpreterSnapshot(interpreter, {
            stack: [{ type: 'nil' }] as never,
            stackSnapshot: '[{"v":{"h":"unassigned","d":{"t":"Nil"}},"r":"unassigned"}]',
        } as never);
        expect(fns.restore_stack_snapshot).toHaveBeenCalledTimes(1);
        expect(fns.restore_stack_snapshot).toHaveBeenCalledWith(
            '[{"v":{"h":"unassigned","d":{"t":"Nil"}},"r":"unassigned"}]'
        );
    });

    test('restores no stack when no snapshot string is present', () => {
        const { fns, interpreter } = makeMock();
        applyInterpreterSnapshot(interpreter, { stack: [{ type: 'nil' }] } as never);
        expect(fns.restore_stack_snapshot).not.toHaveBeenCalled();
    });

    test('ignores a non-string stackSnapshot', () => {
        const { fns, interpreter } = makeMock();
        applyInterpreterSnapshot(interpreter, {
            stack: [{ type: 'nil' }] as never,
            stackSnapshot: 123,
        } as never);
        expect(fns.restore_stack_snapshot).not.toHaveBeenCalled();
    });
});
