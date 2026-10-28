# Verification Plan

## Objective
Define minimum verification evidence required for Ajisai changes.

## Required Baseline Checks (All PRs)
- `cargo fmt --check` (run in `rust/`)
- `cargo clippy --all-targets -- -D warnings` (run in `rust/`)
- `cargo test --all-targets --verbose` (run in `rust/`)
- `npm run check` (run at repository root, if JS/TS affected)

> Every baseline check is blocking. The `AJISAI_STRICT_QUALITY` repository
> variable that once staged the rollout has been removed: an advisory-by-default
> gate produces green checkmarks that do not certify anything. See the
> `quality-gate` job in `.github/workflows/test.yml`.

## Enhanced Checks
- `cargo +nightly-2026-10-09 llvm-cov --branch --workspace` when coverage instrumentation is
  available and relevant to changed Rust behavior. The toolchain is not
  incidental: `--branch` expands to `-Z coverage-options=branch`, which only a
  nightly rustc accepts, so the same command on stable fails rather than
  reporting less. Installing it needs
  `rustup toolchain install nightly-2026-10-09 --component llvm-tools-preview` and
  `cargo install cargo-llvm-cov --locked`.
- `npm run test:coverage` for TypeScript: the Vitest suite under istanbul,
  reports in `coverage/ts/`. Istanbul counts from the source, so the figures
  are the same on every Node version.
- QL-A and QL-B files may not lose coverage. The `quality-gate` job runs both
  suites under instrumentation (`Rust branch coverage`, and `TypeScript MC/DC
  tests (Vitest, with coverage)`), then `QL-A / QL-B coverage ratchet`
  (`npm run check:coverage-ratchet`, `scripts/check-coverage-ratchet.mjs`)
  fails the job when any Rust or TypeScript file
  `TRACEABILITY_MATRIX.md` names as a QL-A or QL-B requirement's implementation has a
  lower covered fraction of branches or lines than
  `docs/quality/coverage-baseline.json` records for it. It is a ratchet, not a
  fixed percentage: new code in a gated file must be covered at least as well
  as the file already was. A change that moves a figure — a better-covered
  file, a new QL-A or QL-B row, a deliberate trade — re-records the baseline in the
  same diff (`npm run check:coverage-ratchet -- --update`, after both
  coverage runs), where the reviewer sees it
  move. The baseline also has to match the matrix: a gated file with no entry,
  or an entry for a file that is no longer one, fails.
- The counts are compared across runs, so they are measured reproducibly: CI
  pins the nightly the baseline names (`toolchain`) and fixes proptest's seed
  (`PROPTEST_RNG_SEED`); the TypeScript side needs nothing more than
  `npm run test:coverage`. To reproduce the Rust side locally:
  `rustup toolchain install nightly-2026-10-09 --component llvm-tools-preview`,
  then in `rust/`, `PROPTEST_RNG_SEED=20261010 cargo +nightly-2026-10-09
  llvm-cov --branch --workspace --no-report` and `cargo +nightly-2026-10-09
  llvm-cov report --branch --json --summary-only --output-path
  target/coverage/coverage.json`.
- Every other coverage figure is reported, not gated: the job summary lists
  the Rust and TypeScript totals and every traced file
  (`scripts/coverage-summary.mjs`), and the `rust-coverage` and `ts-coverage`
  artifacts hold the LCOV and HTML reports.

## Formal Verification
- `cargo kani` (run in `rust/`) proves the bounded-model-checking harnesses
  under `cfg(kani)`: every input a harness names, not a sample of them. It is
  blocking, in the `formal-verification` job of `.github/workflows/test.yml`.
  Installation, the two kinds of harness and how to add one are in
  `FORMAL_VERIFICATION.md`.
- A change to a kernel a harness proves (`rust/src/types/small_rational.rs`,
  `rust/src/types/small_divisor.rs`, `rust/src/interpreter/value_order.rs`,
  `rust/src/interpreter/tensor_ops.rs`, `rust/src/interpreter/runtime_limits.rs`,
  `rust/src/interpreter/lane_lift.rs`, `rust/src/types/truth_column.rs`,
  the numeric grammar and string-literal scans of `rust/src/tokenizer.rs`,
  `normalize_index`, the `TAKE`/`DROP` split, `BSEARCH`'s `partition_point`,
  the lane tables of `rust/src/interpreter/truth_ops.rs`) runs its harnesses
  locally before review: `cargo kani --harness <name>`.

## Level-Based Evidence Expectations
- **QL-A**
  - Baseline checks + targeted semantic/regression tests.
  - MC/DC-like checklist reviewed for modified boolean logic paths.
  - Traceability matrix row updates required (`TRACEABILITY_MATRIX.md`).
  - QL-A / QL-B coverage ratchet holds; a moved figure re-records
    `docs/quality/coverage-baseline.json` in the same change.
- **QL-B**
  - Baseline checks + impacted unit/integration tests.
  - Traceability updates for requirement-to-test linkage (`TRACEABILITY_MATRIX.md`).
  - QL-A / QL-B coverage ratchet holds; a moved figure re-records
    `docs/quality/coverage-baseline.json` in the same change.
- **QL-C**
  - Baseline checks; focused verification accepted if unaffected stacks are justified.
- **QL-D**
  - Appropriate subset of checks based on file types changed.

## Review Exit Criteria
A PR is merge-ready only when:
1. Relevant checks pass in CI.
2. PR quality checklist is completed.
3. Any open quality issue has explicit disposition.
