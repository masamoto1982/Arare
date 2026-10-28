# Problem Reports

Known gaps that Phase 0 opens on purpose, each with the phase that closes it.
A report found later is a GitHub issue from the quality template; this file
holds only the gaps the initialization itself leaves, so they are on record
from the first commit that has them.

| ID | Gap | Why it is open | Closes in |
|---|---|---|---|
| PR-0001 | The inherited Playground (`src/`) has no Arare requirement; only `detectRuntimeKind` is traced (AR-INH-001) | The shell is taken over as-is so it keeps running on the Ajisai oracle | Phase 5 |
| PR-0002 | The Playground and its Reference (`public/docs/`) describe Ajisai, not Arare, and run the Ajisai engine | Arare has no engine until Phase 2 and no Playground adapter until Phase 5 | Phase 5 |
| PR-0003 | No Kani harness exists, and `cargo kani` passes when it finds none | The kernels are Phase 2 work; the Lean half of this report closed in Phase 1 (see Closed) | Phase 2 |
| PR-0006 | The Playground keeps Ajisai's internal identifiers (`__AJISAI_TARGET__`, `AJISAI_BUILD_TARGET`, storage keys such as `ajisai-state.json`) | Renaming them is part of making the shell engine-neutral, not a rename for its own sake | Phase 5 |
| PR-0007 | `tools/mcp-server/` is Ajisai's server, kept only as the oracle for agent sessions, and is outside CI | Arare's own server is Phase 6 work | Phase 6 |
| PR-0008 | Ajisai's Reference generator (`scripts/generate-word-reference.mjs`) was removed with Ajisai's spec | The Reference comes with the Playground; the specification half of this report closed in Phase 1 (see Closed) | Phase 5 |
| PR-0009 | Inherited comments in `src/`, `index.html`, `tests/formatter-corpus.json` and `spec/host-protocol.schema.json` cite Ajisai paths (`rust/src/...`, `spec/...`, `docs/dev/...`) that are not in this repository; they name files at the commit `UPSTREAM.md` records | They describe the Ajisai engine the Playground still drives; rewriting them before the shell is engine-neutral would only move the problem | Phase 5 |

## Closed

| ID | Gap | Disposition |
|---|---|---|
| PR-0005 | The tag `arare-baseline-0` existed only in the session that created the baseline; the remote refused the tag push | Closed 2026-10-28: the owner chose to proceed without the tag. The baseline is identified by its commit, `481dc6cac9e70df776b4cf0dec3b224bb1b47629` (`CONFIGURATION_INDEX.md`) |
| PR-0003 (Lean) | The Lean job checked only a placeholder theorem | Closed 2026-10-28 (Phase 1): `formal/Arare/` holds the value model, 80 theorems, each audited by `scripts/check-formal-axioms.mjs` |
| PR-0004 | `cargo-llvm-cov` was installed at its latest version in CI | Closed 2026-10-28 (Phase 1): pinned to 0.9.1 in `.github/workflows/test.yml` and `CONFIGURATION_INDEX.md` |
| PR-0008 (spec) | Ajisai's specification generator was removed with Ajisai's spec | Closed 2026-10-28 (Phase 1): replaced, not ported — `spec/requirements.json` and `scripts/generate-requirements.mjs` (ADR-0006) |
