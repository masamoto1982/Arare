// Coverage for cross-origin-isolation capability detection (implicit-
// parallelism roadmap Phase 5).
//
// DUT: src/platform/cross-origin-isolation.ts — detectParallelCapability /
// describeParallelCapability.
//
// `threadsAvailable` is the decision the worker pool and the future
// wasm-bindgen-rayon initializer branch on:
//   threadsAvailable = crossOriginIsolated && (SharedArrayBuffer defined)
// Both conditions are exercised independently (MC/DC), plus the
// hardwareConcurrency clamping and the recommendedThreads / maxThreads policy.

import { describe, expect, it } from 'vitest';
import {
    detectParallelCapability,
    describeParallelCapability,
    type IsolationScope,
} from './cross-origin-isolation';

// A stand-in for `SharedArrayBuffer` so `typeof scope.SharedArrayBuffer` is
// 'function' without depending on the host actually exposing it.
const SAB = function SharedArrayBufferStub() {} as unknown;

function scope(
    isolated: boolean,
    sab: boolean,
    cores: number | undefined = 8,
): IsolationScope {
    return {
        crossOriginIsolated: isolated,
        ...(sab ? { SharedArrayBuffer: SAB } : {}),
        navigator: cores === undefined ? {} : { hardwareConcurrency: cores },
    };
}

describe('detectParallelCapability', () => {
    it('reports threading available only when isolated AND SharedArrayBuffer present', () => {
        const cap = detectParallelCapability(scope(true, true, 8));
        expect(cap.threadsAvailable).toBe(true);
        expect(cap.recommendedThreads).toBe(8);
    });

    it('isolated but no SharedArrayBuffer → no threading (single condition flips)', () => {
        const cap = detectParallelCapability(scope(true, false, 8));
        expect(cap.crossOriginIsolated).toBe(true);
        expect(cap.sharedArrayBuffer).toBe(false);
        expect(cap.threadsAvailable).toBe(false);
        expect(cap.recommendedThreads).toBe(1);
    });

    it('SharedArrayBuffer present but not isolated → no threading (other condition flips)', () => {
        const cap = detectParallelCapability(scope(false, true, 8));
        expect(cap.crossOriginIsolated).toBe(false);
        expect(cap.sharedArrayBuffer).toBe(true);
        expect(cap.threadsAvailable).toBe(false);
        expect(cap.recommendedThreads).toBe(1);
    });

    it('neither → no threading', () => {
        const cap = detectParallelCapability(scope(false, false, 8));
        expect(cap.threadsAvailable).toBe(false);
        expect(cap.recommendedThreads).toBe(1);
    });

    it('clamps a missing/invalid hardwareConcurrency to 1', () => {
        // Absent hardwareConcurrency (navigator present but no field).
        expect(
            detectParallelCapability({ crossOriginIsolated: true, SharedArrayBuffer: SAB, navigator: {} }).hardwareConcurrency,
        ).toBe(1);
        // Zero / non-positive report clamps up to 1.
        expect(
            detectParallelCapability({ crossOriginIsolated: true, SharedArrayBuffer: SAB, navigator: { hardwareConcurrency: 0 } }).hardwareConcurrency,
        ).toBe(1);
    });

    // hardwareConcurrency = typeof reported === 'number' && Number.isFinite(reported)
    //   ? Math.floor(reported) : 1, then clamped to >= 1.
    // Pairs against (T, T) 6 -> 6: (F, *) '6' -> 1 shows the type check;
    // (T, F) Infinity / NaN -> 1 shows the finiteness check.
    it('reads hardwareConcurrency only when it is a finite number', () => {
        const cores = (hardwareConcurrency: unknown) =>
            detectParallelCapability({
                crossOriginIsolated: true,
                SharedArrayBuffer: SAB,
                navigator: { hardwareConcurrency } as { hardwareConcurrency?: number },
            }).hardwareConcurrency;
        expect(cores(6)).toBe(6);
        expect(cores('6')).toBe(1);
        expect(cores(Infinity)).toBe(1);
        expect(cores(NaN)).toBe(1);
        expect(cores(6.9)).toBe(6);
        expect(cores(-4)).toBe(1);
    });

    it('reads no navigator as one core', () => {
        expect(detectParallelCapability({ crossOriginIsolated: true, SharedArrayBuffer: SAB }).hardwareConcurrency).toBe(1);
    });

    // typeof maxThreads === 'number' && maxThreads >= 1 caps the count.
    // Pairs against (T, T) 4 -> 4: (F, *) absent -> 16 shows the type check;
    // (T, F) 0 -> 16 shows the lower bound.
    it('applies maxThreads only when it is a number of at least 1', () => {
        const threads = (maxThreads?: number) =>
            detectParallelCapability(scope(true, true, 16), maxThreads === undefined ? {} : { maxThreads })
                .recommendedThreads;
        expect(threads(4)).toBe(4);
        expect(threads()).toBe(16);
        expect(threads(0)).toBe(16);
        expect(threads(-2)).toBe(16);
        expect(threads(1)).toBe(1);
        expect(threads(2.7)).toBe(2);
        expect(threads(64)).toBe(16);
    });

    it('caps recommendedThreads at maxThreads when threading is available', () => {
        const cap = detectParallelCapability(scope(true, true, 16), { maxThreads: 4 });
        expect(cap.hardwareConcurrency).toBe(16);
        expect(cap.recommendedThreads).toBe(4);
    });

    it('maxThreads does not raise the single-thread fallback', () => {
        const cap = detectParallelCapability(scope(false, true, 16), { maxThreads: 4 });
        expect(cap.recommendedThreads).toBe(1);
    });
});

describe('describeParallelCapability', () => {
    it('names the available case with the thread count', () => {
        const text = describeParallelCapability(detectParallelCapability(scope(true, true, 4)));
        expect(text).toContain('threading available');
        expect(text).toContain('4');
    });

    it('attributes the missing-isolation case to COOP/COEP', () => {
        const text = describeParallelCapability(detectParallelCapability(scope(false, true, 4)));
        expect(text).toContain('COOP/COEP');
    });

    it('attributes the missing-SharedArrayBuffer case', () => {
        const text = describeParallelCapability(detectParallelCapability(scope(true, false, 4)));
        expect(text).toContain('SharedArrayBuffer');
    });

    it('names missing isolation first when both are missing', () => {
        const text = describeParallelCapability(detectParallelCapability(scope(false, false, 4)));
        expect(text).toContain('COOP/COEP');
        expect(text).not.toContain('SharedArrayBuffer unavailable');
    });

    it('falls back to a generic line for a capability whose fields disagree', () => {
        // Unreachable from detectParallelCapability, which derives
        // threadsAvailable from the other two; a hand-built record reaches it.
        const text = describeParallelCapability({
            crossOriginIsolated: true,
            sharedArrayBuffer: true,
            hardwareConcurrency: 4,
            threadsAvailable: false,
            recommendedThreads: 1,
        });
        expect(text).toBe('single-threaded fallback');
    });
});
