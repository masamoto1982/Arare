# Configuration Index

The configuration of record: what Arare is built and verified with, and the
state each phase reached. Changing anything in the first table is a reviewed
change that updates this file in the same diff
(`CONFIGURATION_MANAGEMENT_PLAN.md`).

## Environment (SECI)

| Item | Pinned version | Where the pin lives |
|---|---|---|
| Rust (stable) | 1.97.0 | `rust/rust-toolchain.toml` |
| Rust (nightly, branch coverage only) | nightly-2026-10-09 | `docs/quality/coverage-baseline.json` (`toolchain`), `.github/workflows/test.yml` |
| cargo-llvm-cov | 0.9.1 | `.github/workflows/test.yml` |
| Kani | 0.68.0 | `.github/workflows/test.yml` |
| Lean 4 | v4.34.0 | `formal/lean-toolchain` |
| Lean dependencies | none (the budget proofs use core Lean's `Rat` and `grind`) | `formal/lake-manifest.json` |
| Node.js | 20 | `.github/workflows/test.yml`, `.github/workflows/build.yml` |
| npm dependencies | as locked | `package-lock.json` |
| Tauri crates | as locked | `src-tauri/Cargo.lock` |
| Benchmark build target | `-C target-cpu=x86-64-v3` | `.github/workflows/test.yml` (performance job), ADR-0009 |
| Benchmark crates | as locked | `rust/bench/Cargo.lock` |
| WebAssembly target | `wasm32-wasip1` with `-C target-feature=+simd128`, for the stable toolchain above | `.github/workflows/test.yml` (performance job), ADR-0012 |
| WebAssembly runtime | Node.js's `node:wasi` (preview 1), the Node.js above | `scripts/lib/run-wasi.mjs` |
| Playground engine | `rust/crates/arare-wasm` for `wasm32-wasip1` (no SIMD flag), stable toolchain above, built by `scripts/build-engine.mjs` into `src/wasm/arare/` (not committed) | `package.json` (`build:engine`), `.github/workflows/test.yml`, `.github/workflows/build.yml`, ADR-0013 |
| Independent Arrow implementation (verification only) | `apache-arrow` 21.2.0, an exact npm dev dependency read by `scripts/check-arrow-interop.mjs` (AR-VER-235); no Rust crate depends on third-party code | `package.json`, `package-lock.json`, ADR-0014 |
| Playground WebAssembly host | `src/engine/wasi.ts` (WASI preview 1, seven imports), in the browser's own WebAssembly | ADR-0013 |

## The Ajisai oracle

| Item | Pinned to |
|---|---|
| Playground WebAssembly bundle | Ajisai `dd5d1b222f11b72e8aa1664ad68b890dcafc06c4` (head of masamoto1982/Ajisai#1922, engine and specification 1.0.0-beta.4; moved to its merge commit when it merges), in `src/wasm/ajisai-oracle/` (`src/wasm/ajisai-oracle/ORACLE.md`) |
| MCP server for agent sessions | Ajisai MCP server 0.9.0 with engine 1.0.0-beta.4, in `tools/mcp-server/`, from the same commit |
| Node build for the oracle comparison | Ajisai `dd5d1b2`, in `tools/mcp-server/wasm/generated/`, read by `scripts/check-oracle.mjs` (ADR-0007) |
| Native build for the benchmarks | `ajisai-core` at `dd5d1b222f11b72e8aa1664ad68b890dcafc06c4`, a Git dependency of `rust/bench` (AR-VER-221) |

## Phase record

| Phase | State | Baseline | Notes |
|---|---|---|---|
| Baseline | Done | `481dc6cac9e70df776b4cf0dec3b224bb1b47629` | Ajisai content without history (`UPSTREAM.md`). Identified by its commit; the owner chose not to tag it |
| 0. Initialization | Merged | `02a8f32ebd5c6b28c8c748b10b4d0b8af754ea9b` (masamoto1982/Arare#1) | WORK_ORDER.md §10. Open gaps: `docs/quality/PROBLEM_REPORTS.md` |
| 1. Requirements and formal model | Merged | `4c12a6e` (masamoto1982/Arare#3) | Requirements registry, ADR-0001 to ADR-0006, Lean model (AR-VER-101 to 106) |
| 2a. Executable specification and kernel | Merged | `1b0afab` (masamoto1982/Arare#4) | `arare-spec`, `arare-kernel`, AR-LLR-001 to 009, AR-VER-201 to 207 |
| 2b. Front end and interpreter | Merged | `f3e3ea3` (masamoto1982/Arare#5) | Literals and QUANTIZE, the Words, NIL reasons, the CLI, the Ajisai oracle; AR-HLR-021 to 026, AR-LLR-010 to 017, AR-VER-208 to 214 |
| 3. Error-budget checker | Merged | `c4e2ab2` (masamoto1982/Arare#6) | `arare-budget`, `arare check`, ADR-0008; the soundness theorem (AR-VER-107); AR-HLR-027 to 029, AR-LLR-018 to 022, AR-VER-215 to 218; the oracle against inferred budgets and 10,000 generated programs |
| 4a. Compiled route, native | Merged | `7ec8d2a` (masamoto1982/Arare#7) | `arare-codegen`, `arare-compiled`, `arare compile`, `rust/bench`, ADR-0009; AR-HLR-030 and 031, AR-LLR-023 to 027, AR-VER-220 to 225; AR-PERF-001, 002, 004, 005 and 006 measured |
| 4b. Names, FOLD compiled, WebAssembly | Merged | `f40e63a` (masamoto1982/Arare#8) | BIND and names (ADR-0010), FOLD and BIND on the compiled route, the checker's coarsening (ADR-0011, `coarsen_sound`), the WebAssembly SIMD128 build (ADR-0012, `arare-timing`); AR-HLR-032, AR-LLR-028 to 030, AR-VER-226 to 229; AR-PERF-003 and 007 measured |
| 5. Playground | Merged | `89d8d89` (masamoto1982/Arare#9) | Arare's engine in the Playground (ADR-0013, `arare-wasm`), the engine interface and selector, Run and Step, the budget panel and the comparison with Ajisai, the generated Reference; AR-HLR-033 to 036, AR-LLR-031 to 038, AR-VER-230 to 232 and 301 to 306; PR-0001, 0002, 0006, 0008, 0009 closed |
| 6a. Arrow input and output | Merged | `2dc700f` (masamoto1982/Arare#11) | `arare-arrow` (ADR-0014): the IPC stream and file formats written and read by hand, numbers as exact decimals, `arare run --arrow-in/--arrow-out`; AR-HLR-037 and 038, AR-LLR-039 to 043, AR-VER-233 to 236; the Phase 6 exit condition (values and NIL survive an Arrow round trip) met for the IPC route |
