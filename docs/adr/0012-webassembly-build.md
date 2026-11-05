# ADR-0012: The WebAssembly build

Status: Accepted (Phase 4b, 2026-11-04). Extends ADR-0009, whose decision stands.
Requirements: AR-HLR-030, AR-LLR-025, AR-PERF-007.
Verification: AR-VER-220 (under WebAssembly), AR-VER-226.

## Context

ADR-0009 compiles the emitted Rust for the host and for WebAssembly SIMD128,
and AR-PERF-007 asks the latter to be within 3 times the former. Phase 4a
measured only the native build. WebAssembly needs a target, a runtime to run
it in CI, and a way to time it.

## Decision

- **`wasm32-wasip1`, run by Node's WASI.** The timing program
  (`rust/crates/arare-timing`) and the translation-validation tests are
  ordinary Rust with `main` and `Instant`; built for `wasm32-wasip1` with
  `+simd128`, they run under `node:wasi` (`scripts/lib/run-wasi.mjs`), the
  same V8 a browser runs the Playground in. `wasm32-unknown-unknown` has no
  clock, and exporting functions by hand needs `#[no_mangle]`, which the
  workspace's `forbid(unsafe_code)` refuses.
- **One emitted text, a loop per target.** LLVM vectorizes a reduction with
  one `i32` partial four lanes at a time under WebAssembly, and with sixteen
  partials a whole vector at a time; natively one partial is faster. The
  emitter writes both loops behind `cfg!(target_arch = "wasm32")`.
- **Compared per kernel, in one job.** `scripts/check-wasm-speed.mjs` runs
  both builds twice, alternating, keeps each kernel's best time, and fails
  when any kernel's WebAssembly time exceeds 3 times its native one (x86-64-v3,
  as the other AR-PERF requirements).

## Consequences

AR-PERF-007's figure, set before anything was measured, holds: 1.0 to 2.4
times across the six kernels locally. The grouped loop runs only under
WebAssembly, so the translation tests run there too.
