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
| Lean dependencies | none | `formal/lake-manifest.json` |
| Node.js | 20 | `.github/workflows/test.yml`, `.github/workflows/build.yml` |
| npm dependencies | as locked | `package-lock.json` |
| Tauri crates | as locked | `src-tauri/Cargo.lock` |

## The Ajisai oracle

| Item | Pinned to |
|---|---|
| Playground WebAssembly bundle | Ajisai `4d5f693def073b303d7ede3a98b3e9fa15060601`, in `src/wasm/ajisai-oracle/` (`src/wasm/ajisai-oracle/ORACLE.md`) |
| MCP server for agent sessions | Ajisai MCP server 0.9.0 with engine 1.0.0-beta.3, in `tools/mcp-server/`, from the same commit |
| Node build for the oracle comparison | Ajisai `4d5f693`, in `tools/mcp-server/wasm/generated/`, read by `scripts/check-oracle.mjs` (ADR-0007) |

## Phase record

| Phase | State | Baseline | Notes |
|---|---|---|---|
| Baseline | Done | `481dc6cac9e70df776b4cf0dec3b224bb1b47629` | Ajisai content without history (`UPSTREAM.md`). Identified by its commit; the owner chose not to tag it |
| 0. Initialization | Merged | `02a8f32ebd5c6b28c8c748b10b4d0b8af754ea9b` (masamoto1982/Arare#1) | WORK_ORDER.md §10. Open gaps: `docs/quality/PROBLEM_REPORTS.md` |
| 1. Requirements and formal model | Merged | `4c12a6e` (masamoto1982/Arare#3) | Requirements registry, ADR-0001 to ADR-0006, Lean model (AR-VER-101 to 106) |
| 2a. Executable specification and kernel | Merged | `1b0afab` (masamoto1982/Arare#4) | `arare-spec`, `arare-kernel`, AR-LLR-001 to 009, AR-VER-201 to 207 |
| 2b. Front end and interpreter | In review | — | Literals and QUANTIZE, the Words, NIL reasons, the CLI, the Ajisai oracle; AR-HLR-021 to 026, AR-LLR-010 to 017, AR-VER-208 to 214 |
