# Ajisai oracle bundle

The WebAssembly build of Ajisai that the Playground runs until Arare's own
engine is connected (WORK_ORDER.md D-13, Phase 5), and that Arare's results are
compared against afterwards.

| Field | Value |
|---|---|
| Source | https://github.com/masamoto1982/Ajisai |
| Commit | `4d5f693def073b303d7ede3a98b3e9fa15060601` |
| Engine | 1.0.0-beta.3 |
| Files | `ajisai_core.js`, `ajisai_core.d.ts`, `ajisai_core_bg.wasm`, `ajisai_core_bg.wasm.d.ts`, byte for byte as committed upstream under `src/wasm/generated/` |

The bundle is never rebuilt here and never edited. Moving the oracle to another
Ajisai commit replaces all four files from that commit's `src/wasm/generated/`
and updates this table and `docs/quality/CONFIGURATION_INDEX.md` in the same
change.
