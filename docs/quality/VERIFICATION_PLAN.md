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
| Lean model vs Rust executable specification | `arare-spec` computes what the Lean definitions compute, on every int8 operand pair | Phase 2 |
| Kani over int16 and int32 kernels | Each kernel agrees with `arare-spec` over its whole range and reaches no overflow, out-of-bounds index or panic | Phase 2 |
| Oracle comparison | `|Arare − Ajisai| ≤ the inferred budget` on generated programs; Ajisai pinned to the commit `UPSTREAM.md` names | Phase 2 (harness), Phase 3 (budget) |
| Translation validation | Compiled code (native and WASM SIMD128) agrees with the reference interpreter: exhaustively for int8, by boundary and random cases for int16 and int32 | Phase 4 |
| Performance | Every `AR-PERF-*` requirement, measured against a baseline loop in the same run (WORK_ORDER.md §6) | Phase 4 |

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
