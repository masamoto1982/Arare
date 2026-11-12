# Ajisai oracle bundle

The WebAssembly build of Ajisai that the Playground ran until Arare's own
engine was connected (WORK_ORDER.md D-13, Phase 5), and that Arare's results
are compared against since.

| Field | Value |
|---|---|
| Source | https://github.com/masamoto1982/Ajisai |
| Commit | `0bb45c6aaf7c9603c1ec5fec37bc8aadc4800bcc` |
| Engine | 1.0.0-beta.4 |
| Files | `ajisai_core.js`, `ajisai_core.d.ts`, `ajisai_core_bg.wasm`, `ajisai_core_bg.wasm.d.ts`, byte for byte as committed upstream under `src/wasm/generated/` |

The bundle is never rebuilt here and never edited. Moving the oracle to another
Ajisai commit replaces all four files from that commit's `src/wasm/generated/`
and updates this table and `docs/quality/CONFIGURATION_INDEX.md` in the same
change.
