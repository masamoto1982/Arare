# ADR-0009: The compiled route

Status: Accepted (Phase 4a, 2026-11-03). Decides WORK_ORDER.md Q-02.
Requirements: AR-HLR-030, AR-HLR-031, AR-LLR-023 to AR-LLR-027, AR-PERF-001 to AR-PERF-007.
Verification: AR-VER-220 to AR-VER-225.

## Context

Arare exists to be fast where Ajisai is slow (D-04, AR-SYS-014). The
performance requirements bind an ahead-of-time compiled route, native and
WebAssembly SIMD128. That route must still answer exactly what the reference
interpreter answers, wheel points and NIL included (D-09 to D-11), and the
evidence for it is translation validation (`FORMAL_VERIFICATION.md`, "Compiled
code"). Q-02 asked for the backend: `wasm-encoder` for WebAssembly and
Cranelift for native code, or Rust source generation.

## Decision

- **Rust source is the target.** `arare-codegen` lowers a program and a
  signature — the width and scale of each input, scalar or vector — to a
  typed intermediate representation, and writes Rust; `rustc` compiles it, for
  the host or for `wasm32` with SIMD128. One code generator serves both
  targets, LLVM vectorizes the loops, and the workspace keeps no dependency.
  Widths and scales are asked of the kernel's own operations on placeholder
  numbers, so lowering cannot drift from the kernel.
- **A fast path, and the interpreter behind it.** The emitted code computes on
  plain integer codes in one loop over the lanes, checks only what can fail —
  a zero divisor, a code that may leave its width — and on any failure, or any
  input it does not take, hands the whole program to the reference
  interpreter. A program the lowering does not cover (FILTER, FOLD, a literal
  vector, a second pass) is the interpreter outright. So the compiled route
  answers what the interpreter answers by construction where it falls back,
  and translation validation holds the fast path to it everywhere else.
- **Columns know their wheel points and NIL lanes.** A column records, when it
  is made, whether any lane holds `1/0` or `0/0` and whether any lane is NIL,
  as Arrow records a null count (AR-HLR-031). The fast path reads those two
  facts once instead of looking at every lane; that is what keeps the cost of
  carrying points and NIL within AR-PERF-006.
- **Arithmetic that vectorizes.** DIV is the `f64` quotient rounded half to
  even, which is exact for a dividend below 2^52 and a divisor below 2^53
  (AR-LLR-026); RESCALE runs in `i32` where the code fits. Both are tested
  against the kernel's integer definitions.
- **The benchmarks build for x86-64-v3.** The compiled route is built for the
  machine it runs on, as a deployed binary is. CI builds the benchmarks for
  x86-64-v3 (AVX2), which every runner has, so a run is the same code on every
  machine; the figures are ratios to a baseline measured in the same run, and
  Ajisai is built from the commit `UPSTREAM.md` records.
- **Phase 4 in two halves.** 4a is the native route and AR-PERF-001, 002, 004,
  005 and 006. 4b adds BIND and FOLD to the compiled route (the logistic map
  of AR-PERF-003 needs a value twice) and the WebAssembly build (AR-PERF-007).

## Alternatives

- **Cranelift for native code, `wasm-encoder` for WebAssembly.** Two backends
  to write and validate, two new dependency trees, and neither vectorizes on
  its own: the int8 dot product and the harmonic sum would need a vectorizer
  Arare does not have.
- **A JIT.** The requirements are for ahead-of-time code, and a JIT puts the
  compiler in the trusted run-time path.
- **Checking every lane for wheel points.** Measured: 1.8 times slower on the
  int16 dot product, against AR-PERF-006's 1.1.
- **Integer division for DIV.** Measured: the harmonic sum at 650 times
  Ajisai, against AR-PERF-004's 1,000; the `f64` quotient reaches 1,500 to
  1,800.
