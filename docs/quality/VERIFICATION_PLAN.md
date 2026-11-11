# Verification Plan

The evidence each change must carry, by quality level, and the gates that
enforce it. Every gate listed as blocking fails the `quality-gate`,
`formal-verification-kani` or `formal-verification-lean` job in
`.github/workflows/test.yml`; nothing here is advisory.

## Baseline checks (every pull request, blocking)

- `npm run check:rust`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
  and `cargo test --all-targets` over the Rust workspace.
- `npm run check:formal`: `lake build` in `formal/`, which checks every proof,
  then `scripts/check-formal-axioms.mjs`, which fails when any theorem rests
  on more than Lean's three standard axioms (`sorry` among them: `lake build`
  only warns on it).
- `npm run check:requirements`: the registry `spec/requirements.json` is well
  formed, reaches `formal/` in both directions, and its generated surfaces
  (`spec/REQUIREMENTS.md`, the matrix block) are current.
- `cargo kani` in `rust/`: every bounded-model-checking harness
  (`FORMAL_VERIFICATION.md`).
- `npm run check`, `npm run lint`, `npm run test:coverage`: the TypeScript
  shell's type check, lint and tests.
- `npm run check:gates`: the requirements registry, traceability, version
  sync, the Rust file-size budget and the Playground's WCAG contrast.
- `npm run check:oracle` and `npm run check:oracle-random`: Arare against the
  pinned Ajisai build within the inferred error budget (AR-VER-209,
  AR-VER-217).
- `npm run check:coverage-ratchet`: no QL-A or QL-B file loses branch or line
  coverage against `docs/quality/coverage-baseline.json`.

## Evidence by level

| Level | Requirements-based tests | Structural coverage | Formal | Review |
|---|---|---|---|---|
| **QL-A** | Normal and robustness cases for every requirement; exhaustive where the input space allows (every pair of int8 operands, 65,536 cases) | MC/DC, written as tables in the tests (the form of Ajisai's `fraction_mcdc_tests.rs`), plus the coverage ratchet | A Kani harness for every kernel; a Lean theorem for every semantic law; differential tests between the two | Independent (`PLAN_OVERVIEW.md`) |
| **QL-B** | Normal and robustness cases | Decision coverage, plus the coverage ratchet | Where a kernel is small enough to decide | Independent |
| **QL-C** | Normal cases | Statement coverage reported | — | Any reviewer |
| **QL-D** | Normal cases | Reported | — | Any reviewer |

MC/DC is shown by the tables, not by an instrumentation tool: no tool in the
pinned toolchain reports it, and the coverage ratchet only proves no ground is
lost.

## Verification methods specific to Arare

These start in the phase named and are blocking from then on.

| Method | What it shows | From |
|---|---|---|
| Lean model vs Rust executable specification | `arare-spec` computes what the Lean definitions compute, on every int8 operand pair and the rest of the grid of `formal/Table.lean` (AR-VER-201, in the Lean job) | Phase 2a — active |
| Kani over the kernels | Each kernel meets its law over its whole range, or a stated scope, and reaches no overflow, out-of-bounds index or panic (AR-VER-207; the job fails when fewer harnesses are verified than declared) | Phase 2a — active |
| Oracle comparison | `|Arare − Ajisai| ≤` the inferred error budget over `tests/oracle/corpus.json`, each case's budget no looser than its hand-derived figure, the deliberate divergences recorded as cases (AR-VER-209, ADR-0007, `npm run check:oracle`); Ajisai pinned to the commit `UPSTREAM.md` names | Phase 2b — active; inferred budgets from Phase 3 |
| Error budget against the model | The Rust checker infers what the Lean checker does over the grid of `formal/BudgetTable.lean` (AR-VER-215, in the Lean job) | Phase 3 — active |
| Error budget against Ajisai | `|Arare − Ajisai| ≤` the inferred budget on at least 10,000 seeded, generated programs, with no violation and at least half the values bounded (AR-VER-217, `npm run check:oracle-random`; `--emit` writes the programs, which Ajisai keeps in its own CI as the shared core) | Phase 3 — active |
| Translation validation | Compiled code agrees with the reference interpreter: every int8 pair for the lane-wise Words, seeded boundary and random inputs at every width for every compiled program, wheel points, NIL and mismatched inputs included, and the fast path must be the one that answers on finite inputs (AR-VER-220); natively, and built for `wasm32-wasip1` with SIMD128 under Node's WASI in the performance job | Phase 4a — active for the native route; Phase 4b — active for WASM SIMD128 |
| Performance | Every `AR-PERF-*` requirement, measured against a baseline in the same run — a hand-written loop or Ajisai's native build pinned to `UPSTREAM.md`'s commit — in interleaved rounds, best round kept, built for x86-64-v3 (AR-VER-221, `rust/bench`, the performance job); AR-PERF-003 also within the inferred budget of Ajisai's exact result; AR-PERF-007 per kernel against the native build of the same kernels, both built from `rust/crates/arare-timing` (AR-VER-226, `scripts/check-wasm-speed.mjs`) | Phase 4a — active for AR-PERF-001, 002, 004, 005, 006; Phase 4b — active for 003 and 007 |
| Arrow input and output | What Arare writes reads back unchanged for every width and exponent from −31 to 31, wheel points and NIL lanes, in both IPC formats (AR-VER-233); every prefix and 40,000 seeded byte flips of valid input read to an answer or an error, never a panic (AR-VER-234); the `apache-arrow` package reads what Arare writes and writes what Arare reads (AR-VER-235, `npm run check:arrow`); `arare run`'s Arrow flags (AR-VER-236) | Phase 6a — active |
| The Playground against the engine | The Playground's Arare runs the real engine command under the browser's WASI host in the TypeScript suites: the protocol, sessions and snapshots (AR-VER-231, AR-VER-301, AR-VER-302); each Reference example's result and budgets are the engine's (AR-VER-305); the Playground's comparison is the oracle gate's own code, run end to end on Ajisai (AR-VER-304) | Phase 5 — active |

## Coverage measurement

- Rust: `cargo +<nightly> llvm-cov --branch --workspace`, the nightly pinned
  in `docs/quality/coverage-baseline.json` (`toolchain`), with proptest's seed
  fixed by `PROPTEST_RNG_SEED`. The branch flag needs a nightly rustc; the
  rest of the build stays on the pinned stable.
- TypeScript: `npm run test:coverage` (Vitest with istanbul).
- A change that moves a gated figure re-records the baseline in the same diff
  (`npm run check:coverage-ratchet -- --update` after both coverage runs).

## Review exit criteria

A pull request is ready to merge only when:

1. Every baseline check passes in CI.
2. The matrix rows for every requirement it touches are updated.
3. The evidence its level requires is present.
4. Every open problem report it affects has a disposition.
