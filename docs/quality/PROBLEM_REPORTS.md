# Problem Reports

Known gaps that Phase 0 opens on purpose, each with the phase that closes it.
A report found later is a GitHub issue from the quality template; this file
holds only the gaps the initialization itself leaves, so they are on record
from the first commit that has them.

| ID | Gap | Why it is open | Closes in |
|---|---|---|---|
| PR-0001 | The inherited Playground (`src/`) has no Arare requirement; only `detectRuntimeKind` is traced (AR-INH-001) | The shell is taken over as-is so it keeps running on the Ajisai oracle | Phase 5 |
| PR-0002 | The Playground and its Reference (`public/docs/`) describe Ajisai, not Arare, and run the Ajisai engine | Arare has no engine until Phase 2 and no Playground adapter until Phase 5 | Phase 5 |
| PR-0006 | The Playground keeps Ajisai's internal identifiers (`__AJISAI_TARGET__`, `AJISAI_BUILD_TARGET`, storage keys such as `ajisai-state.json`) | Renaming them is part of making the shell engine-neutral, not a rename for its own sake | Phase 5 |
| PR-0007 | `tools/mcp-server/` is Ajisai's server, kept as the oracle for agent sessions; the server is outside CI, though its committed Node build of Ajisai is what the oracle comparison runs (ADR-0007) | Arare's own server is Phase 6 work | Phase 6 |
| PR-0008 | Ajisai's Reference generator (`scripts/generate-word-reference.mjs`) was removed with Ajisai's spec | The Reference comes with the Playground; the specification half of this report closed in Phase 1 (see Closed) | Phase 5 |
| PR-0009 | Inherited comments in `src/`, `index.html`, `tests/formatter-corpus.json` and `spec/host-protocol.schema.json` cite Ajisai paths (`rust/src/...`, `spec/...`, `docs/dev/...`) that are not in this repository; they name files at the commit `UPSTREAM.md` records | They describe the Ajisai engine the Playground still drives; rewriting them before the shell is engine-neutral would only move the problem | Phase 5 |
| PR-0011 | The compiled route has no fast path for FILTER, BIND of several names, a FOLD whose block changes the accumulator's width or scale, a literal vector, a lane value computed from a reduction or EQ of vectors: such a program runs in the interpreter, correctly but slowly (AR-LLR-024, AR-LLR-029) | Phase 4a compiled the Words the benchmarks need and Phase 4b FOLD and BIND of one name, for the logistic map; the rest has no performance requirement yet | When a requirement needs it |
| PR-0013 | A name is looked up where the item naming it runs. A quote that leaves the block it was written in — a block's result that is itself a quote, read as data later — reads the names where it is read, where Ajisai reads those of the frame it was written in (ADR-0010) | Without User Words a block runs inside the scope it was written in, except in this case; closures for it would cost every quote | When a program needs it, or with User Words |

## Closed

| ID | Gap | Disposition |
|---|---|---|
| PR-0005 | The tag `arare-baseline-0` existed only in the session that created the baseline; the remote refused the tag push | Closed 2026-10-28: the owner chose to proceed without the tag. The baseline is identified by its commit, `481dc6cac9e70df776b4cf0dec3b224bb1b47629` (`CONFIGURATION_INDEX.md`) |
| PR-0003 (Lean) | The Lean job checked only a placeholder theorem | Closed 2026-10-28 (Phase 1): `formal/Arare/` holds the value model, 80 theorems, each audited by `scripts/check-formal-axioms.mjs` |
| PR-0004 | `cargo-llvm-cov` was installed at its latest version in CI | Closed 2026-10-28 (Phase 1): pinned to 0.9.1 in `.github/workflows/test.yml` and `CONFIGURATION_INDEX.md` |
| PR-0008 (spec) | Ajisai's specification generator was removed with Ajisai's spec | Closed 2026-10-28 (Phase 1): replaced, not ported — `spec/requirements.json` and `scripts/generate-requirements.mjs` (ADR-0006) |
| PR-0003 (Kani) | No Kani harness existed, and `cargo kani` passed when it found none | Closed 2026-10-28 (Phase 2a): nine harnesses in `rust/crates/arare-kernel/src/proofs.rs`, and the Kani job fails when fewer are verified than the source declares |
| PR-0012 | AR-PERF-003 (the logistic map) and AR-PERF-007 (WASM SIMD128) were not measured | Closed 2026-11-04 (Phase 4b): AR-PERF-003 in `rust/bench` (AR-VER-221), within the inferred budget of Ajisai's exact result; AR-PERF-007 per kernel by `scripts/check-wasm-speed.mjs` (AR-VER-226) |
| PR-0010 | The oracle corpus's tolerances were worked out by hand, case by case | Closed 2026-10-29 (Phase 3): the comparison uses the budget `arare-budget` infers (ADR-0008); each case's hand-derived figure stays as the largest budget the checker may infer |
