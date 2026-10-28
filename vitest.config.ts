import { defineConfig } from 'vitest/config';

// AQ-REQ-004 / AQ-VER-004 — Vitest configuration for TypeScript-side
// MC/DC tests. Kept intentionally minimal to mirror the Rust-side
// `cargo test` ergonomics: no globals, no UI. Coverage is opt-in
// (`npm run test:coverage`), measured with istanbul so the counts the
// coverage ratchet compares do not depend on the Node version.
export default defineConfig({
    // Mirror the production build's injected globals so importing modules that
    // transitively reference them (e.g. the platform adapters pulled in by the
    // persistence layer) does not throw a ReferenceError at module load. This
    // only defines build-time constants; it adds no DOM tooling.
    define: {
        __AJISAI_BUILD_TIMESTAMP__: JSON.stringify('test'),
        __AJISAI_RELEASE_VERSION__: JSON.stringify('test'),
    },
    test: {
        // Co-locate tests with source: src/**/*.test.ts.
        include: ['src/**/*.test.ts'],
        // No DOM helpers required for the current MC/DC suite. The few
        // tests that exercise `window` detection do so via deliberate
        // global stubbing, not via a simulated DOM.
        environment: 'node',
        globals: false,
        // Quality gate: surface unhandled rejections and errors.
        dangerouslyIgnoreUnhandledErrors: false,
        // Fail fast on snapshot drift; we don't use snapshots here, but
        // future contributors should opt in explicitly.
        passWithNoTests: false,
        // Coverage runs only under `npm run test:coverage` (CI's Quality
        // Gate); a plain `npm test` stays uninstrumented. Every source file
        // is measured, tested or not, so an untested module shows as 0%
        // rather than vanishing from the report. The JSON summary feeds the
        // job summary and the QL-A / QL-B coverage ratchet
        // (scripts/check-coverage-ratchet.mjs).
        coverage: {
            provider: 'istanbul',
            include: ['src/**/*.ts'],
            exclude: ['src/**/*.test.ts', 'src/test-support.ts', 'src/**/*.d.ts'],
            reporter: ['text-summary', 'json-summary', 'lcov', 'html'],
            reportsDirectory: 'coverage/ts',
        },
    },
});
