# Problem Reports

Known gaps that Phase 0 opens on purpose, each with the phase that closes it.
A report found later is a GitHub issue from the quality template; this file
holds only the gaps the initialization itself leaves, so they are on record
from the first commit that has them.

| ID | Gap | Why it is open | Closes in |
|---|---|---|---|
| PR-0001 | The inherited Playground (`src/`) has no Arare requirement; only `detectRuntimeKind` is traced (AR-INH-001) | The shell is taken over as-is so it keeps running on the Ajisai oracle | Phase 5 |
| PR-0002 | The Playground and its Reference (`public/docs/`) describe Ajisai, not Arare, and run the Ajisai engine | Arare has no engine until Phase 2 and no Playground adapter until Phase 5 | Phase 5 |
| PR-0003 | No Kani harness exists, and `cargo kani` passes when it finds none; the Lean job checks only a placeholder theorem | The kernels and the semantic model are Phase 1 and Phase 2 work | Phase 2 (Kani), Phase 1 (Lean) |
| PR-0004 | `cargo-llvm-cov` is installed at its latest version in CI rather than pinned | Inherited from Ajisai's workflow | Phase 1 |
| PR-0005 | The tag `arare-baseline-0` exists only in the session that created the baseline; the remote refused the tag push | The push route available to that session accepts branches but not tags | When the owner pushes the tag, or Phase 0 review |
| PR-0006 | The Playground keeps Ajisai's internal identifiers (`__AJISAI_TARGET__`, `AJISAI_BUILD_TARGET`, storage keys such as `ajisai-state.json`) | Renaming them is part of making the shell engine-neutral, not a rename for its own sake | Phase 5 |
| PR-0007 | `tools/mcp-server/` is Ajisai's server, kept only as the oracle for agent sessions, and is outside CI | Arare's own server is Phase 6 work | Phase 6 |
| PR-0008 | The specification generator and Reference generator Ajisai had (`scripts/generate-specification.mjs`, `scripts/generate-word-reference.mjs`) were removed with Ajisai's spec | They read Ajisai's spec, which is gone; they are ported from the commit `UPSTREAM.md` names when Arare's spec exists | Phase 1 (spec), Phase 5 (Reference) |
| PR-0009 | Inherited comments in `src/`, `index.html`, `tests/formatter-corpus.json` and `spec/host-protocol.schema.json` cite Ajisai paths (`rust/src/...`, `spec/...`, `docs/dev/...`) that are not in this repository; they name files at the commit `UPSTREAM.md` records | They describe the Ajisai engine the Playground still drives; rewriting them before the shell is engine-neutral would only move the problem | Phase 5 |
