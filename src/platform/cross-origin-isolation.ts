// Browser-parallelism capability detection (implicit-parallelism roadmap
// Phase 5). SharedArrayBuffer-backed wasm threading only runs when the page is
// *cross-origin isolated* — which requires the COOP/COEP response headers
// (`Cross-Origin-Opener-Policy: same-origin` +
// `Cross-Origin-Embedder-Policy: require-corp`) and a browser that exposes
// `SharedArrayBuffer`. GitHub Pages cannot set those headers and there is
// currently no service-worker fallback registered, so production always
// degrades silently to single-threaded execution (Never Slower: the
// sequential lane is always correct).
//
// This module is the single source of truth for "can we thread in this
// browser?" It is intentionally a pure function over an injectable scope so it
// is unit-testable without a real `Window`, and so the worker pool, the wasm
// loader, and any future `wasm-bindgen-rayon` thread-pool initializer all read
// the same decision.

/** The subset of the global scope this detection reads. */
export interface IsolationScope {
    crossOriginIsolated?: boolean;
    SharedArrayBuffer?: unknown;
    navigator?: { hardwareConcurrency?: number };
}

export interface ParallelCapability {
    /** `globalThis.crossOriginIsolated === true`. */
    crossOriginIsolated: boolean;
    /** `SharedArrayBuffer` is a defined constructor in this scope. */
    sharedArrayBuffer: boolean;
    /** Reported logical cores, clamped to at least 1. */
    hardwareConcurrency: number;
    /**
     * The only field callers should branch on to decide whether to spin up
     * wasm threads: true exactly when shared-memory threading can actually run
     * (isolated *and* `SharedArrayBuffer` present).
     */
    threadsAvailable: boolean;
    /**
     * Worker/thread count to request: `hardwareConcurrency` when threading is
     * available (optionally capped by `maxThreads`), otherwise 1.
     */
    recommendedThreads: number;
}

interface DetectOptions {
    /** Upper bound on `recommendedThreads`; omit for no cap. */
    maxThreads?: number;
}

/**
 * Detect whether SharedArrayBuffer-backed wasm threading can run in `scope`.
 *
 * Defaults to the real global scope; pass an explicit `scope` in tests to
 * exercise the isolated / non-isolated / no-SAB branches deterministically.
 */
export function detectParallelCapability(
    scope: IsolationScope = globalThis as IsolationScope,
    options: DetectOptions = {},
): ParallelCapability {
    const crossOriginIsolated = scope.crossOriginIsolated === true;
    const sharedArrayBuffer = typeof scope.SharedArrayBuffer !== 'undefined';
    const reported = scope.navigator?.hardwareConcurrency;
    const hardwareConcurrency = Math.max(
        1,
        typeof reported === 'number' && Number.isFinite(reported) ? Math.floor(reported) : 1,
    );
    const threadsAvailable = crossOriginIsolated && sharedArrayBuffer;

    let recommendedThreads = threadsAvailable ? hardwareConcurrency : 1;
    if (typeof options.maxThreads === 'number' && options.maxThreads >= 1) {
        recommendedThreads = Math.min(recommendedThreads, Math.floor(options.maxThreads));
    }

    return {
        crossOriginIsolated,
        sharedArrayBuffer,
        hardwareConcurrency,
        threadsAvailable,
        recommendedThreads,
    };
}

/** Human-readable one-liner for startup logs / diagnostics. */
export function describeParallelCapability(
    capability: ParallelCapability = detectParallelCapability(),
): string {
    if (capability.threadsAvailable) {
        return `cross-origin isolated; SharedArrayBuffer threading available (${capability.recommendedThreads} thread(s))`;
    }
    if (!capability.crossOriginIsolated) {
        return 'not cross-origin isolated (COOP/COEP missing); single-threaded fallback';
    }
    if (!capability.sharedArrayBuffer) {
        return 'SharedArrayBuffer unavailable in this browser; single-threaded fallback';
    }
    return 'single-threaded fallback';
}
