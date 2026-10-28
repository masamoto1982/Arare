// AQ-VER-004-A: detectRuntimeKind MC/DC for QL-A boolean decisions.
//
// DUT: src/platform/index.ts, detectRuntimeKind()
//
//     export function detectRuntimeKind(): RuntimeKind {
//         if (typeof __AJISAI_TARGET__ !== 'undefined' && __AJISAI_TARGET__ === 'tauri') {
//             return 'tauri';                                                       // (1)
//         }
//         if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
//             return 'tauri';                                                       // (2)
//         }
//         return 'web';                                                             // (3)
//     }
//
// Two sequential 2-condition AND decisions that route a runtime-kind
// classifier across three reachable outcomes.
//
// Decision (1) — build-time injection guard:
//   A1 = (typeof __AJISAI_TARGET__ !== 'undefined')
//   A2 = (__AJISAI_TARGET__ === 'tauri')
//
// Decision (2) — runtime DOM-detection fallback (only reached when A1&&A2 is F):
//   B1 = (typeof window !== 'undefined')
//   B2 = ('__TAURI_INTERNALS__' in window)
//
// MC/DC for A1 && A2:
//   row 1: (A1=T, A2=T)  -> 'tauri'  (decision 1 taken)
//   row 2: (A1=F, A2=*)  -> falls through to decision 2
//   row 3: (A1=T, A2=F)  -> falls through to decision 2
//   Pair (1, 2) with A2 held T: A1 flips T->F -> outcome flips.
//   Pair (1, 3) with A1 held T: A2 flips T->F -> outcome flips.
//
// MC/DC for B1 && B2 (under decision-1 fall-through):
//   row 4: (B1=T, B2=T)  -> 'tauri'
//   row 5: (B1=F, B2=*)  -> 'web'   (B2 short-circuits on B1=F)
//   row 6: (B1=T, B2=F)  -> 'web'
//   Pair (4, 5) with B2 held T: B1 flips T->F -> outcome flips.
//   Pair (4, 6) with B1 held T: B2 flips T->F -> outcome flips.
//
// Trace: docs/quality/TRACEABILITY_MATRIX.md, requirement AQ-REQ-004.

import { afterEach, beforeEach, describe, expect, test } from 'vitest';
import { detectRuntimeKind } from './index';

// `__AJISAI_TARGET__` is declared as a build-time const in index.ts
// but the runtime check uses `typeof __AJISAI_TARGET__ !== 'undefined'`,
// which falls back to a global lookup at runtime. Tests can therefore
// drive A1 by setting / deleting properties on globalThis. We use Reflect
// to bypass DOM lib's strict typing of `window` (defined as
// `Window & typeof globalThis`) which would otherwise make raw assignment
// or `delete` calls fail under tsc --strict.
function setGlobal(name: string, value: unknown): void {
    Reflect.set(globalThis, name, value);
}
function deleteGlobal(name: string): void {
    Reflect.deleteProperty(globalThis, name);
}
function hasGlobal(name: string): boolean {
    return Reflect.has(globalThis, name);
}
function readGlobal(name: string): unknown {
    return Reflect.get(globalThis, name);
}

describe('AQ-VER-004-A detectRuntimeKind classifier', () => {
    let originalTarget: unknown;
    let originalWindow: unknown;
    let hadTarget: boolean;
    let hadWindow: boolean;

    beforeEach(() => {
        hadTarget = hasGlobal('__AJISAI_TARGET__');
        hadWindow = hasGlobal('window');
        originalTarget = readGlobal('__AJISAI_TARGET__');
        originalWindow = readGlobal('window');
        deleteGlobal('__AJISAI_TARGET__');
        deleteGlobal('window');
    });

    afterEach(() => {
        if (hadTarget) {
            setGlobal('__AJISAI_TARGET__', originalTarget);
        } else {
            deleteGlobal('__AJISAI_TARGET__');
        }
        if (hadWindow) {
            setGlobal('window', originalWindow);
        } else {
            deleteGlobal('window');
        }
    });

    describe('build-time injection guard (A1 && A2)', () => {
        test('row 1 (A1=T, A2=T) -> tauri via build-time injection', () => {
            setGlobal('__AJISAI_TARGET__', 'tauri');
            expect(detectRuntimeKind()).toBe('tauri');
        });

        test('row 2 (A1=F, A2=*) -> falls through to runtime detection', () => {
            // A1=F: __AJISAI_TARGET__ undefined. A2 is short-circuited and
            // not evaluated. With no window either, detection returns 'web'.
            // Pair (row 1, row 2) flips A1 with A2 held T -> outcome flips.
            expect(detectRuntimeKind()).toBe('web');
        });

        test('row 3 (A1=T, A2=F) -> falls through to runtime detection', () => {
            // A1=T but A2=F: defined but not 'tauri'. Pair (row 1, row 3)
            // flips A2 with A1 held T -> outcome flips.
            setGlobal('__AJISAI_TARGET__', 'web');
            expect(detectRuntimeKind()).toBe('web');
        });
    });

    describe('runtime DOM-detection fallback (B1 && B2)', () => {
        test('row 4 (B1=T, B2=T) -> tauri via window.__TAURI_INTERNALS__', () => {
            // No build-time target; window present and has __TAURI_INTERNALS__.
            setGlobal('window', { __TAURI_INTERNALS__: {} });
            expect(detectRuntimeKind()).toBe('tauri');
        });

        test('row 5 (B1=F, B2=*) -> web (no window)', () => {
            // B1=F: typeof window === 'undefined'. B2 is short-circuited.
            // Pair (row 4, row 5) flips B1 with B2 held T -> outcome flips.
            expect(detectRuntimeKind()).toBe('web');
        });

        test('row 6 (B1=T, B2=F) -> web (window present but no internals)', () => {
            // B1=T but B2=F. Pair (row 4, row 6) flips B2 with B1 held T.
            setGlobal('window', {});
            expect(detectRuntimeKind()).toBe('web');
        });
    });

    test('build-time injection takes precedence over runtime detection', () => {
        // Cross-decision invariant: if decision 1 returns 'tauri', decision 2
        // is unreachable. Set both signals: decision 1 should win.
        setGlobal('__AJISAI_TARGET__', 'tauri');
        setGlobal('window', {}); // would otherwise produce 'web' via decision 2
        expect(detectRuntimeKind()).toBe('tauri');
    });
});
