# Upstream

Arare starts from the content of Ajisai, not its history (WORK_ORDER.md D-17).

| Field | Value |
|---|---|
| Upstream repository | https://github.com/masamoto1982/Ajisai |
| Upstream commit | `4d5f693def073b303d7ede3a98b3e9fa15060601` (merge of masamoto1982/Ajisai#1917, 2026-10-23) |
| Upstream version | implementation 1.0.0-beta.3, specification 1.0.0-beta.3 |
| Imported | 2026-10-28, as the single commit tagged `arare-baseline-0` |
| License | MIT, carried over unchanged (`LICENSE`) |

The baseline commit holds the upstream tree byte for byte, plus three files that
are Arare's own: `WORK_ORDER.md`, `CLAUDE.md` and this file. Every later change
is Arare's and is traced in `docs/quality/` from Phase 0 on.

Ajisai stays the oracle Arare is checked against (WORK_ORDER.md §1.3). A later
pin of the oracle — a Git dependency on `ajisai-core` or the WebAssembly bundle
under `src/wasm/` — names its own upstream commit where it is declared.
