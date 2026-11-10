# Upstream

Arare starts from the content of Ajisai, not its history (WORK_ORDER.md D-17).

| Field | Value |
|---|---|
| Upstream repository | https://github.com/masamoto1982/Ajisai |
| Upstream commit | `4d5f693def073b303d7ede3a98b3e9fa15060601` (merge of masamoto1982/Ajisai#1917, 2026-10-23) |
| Upstream version | implementation 1.0.0-beta.3, specification 1.0.0-beta.3 |
| Imported | 2026-10-28, as the single commit `481dc6cac9e70df776b4cf0dec3b224bb1b47629` (untagged) |
| License | MIT, carried over unchanged (`LICENSE`) |

The baseline commit holds the upstream tree byte for byte, plus three files that
are Arare's own: `WORK_ORDER.md`, `CLAUDE.md` and this file. Every later change
is Arare's and is traced in `docs/quality/` from Phase 0 on.

Ajisai stays the oracle Arare is checked against (WORK_ORDER.md §1.3). A later
pin of the oracle — a Git dependency on `ajisai-core` or the WebAssembly bundle
under `src/wasm/` — names its own upstream commit where it is declared.
Since 2026-11-10 the oracle is pinned to Ajisai `319fe84fc1c2e339284156fe310d16a395d664e9`
(1.0.0-beta.4, the merge of masamoto1982/Ajisai#1920); every place it is pinned is listed
in `docs/quality/CONFIGURATION_INDEX.md` (ADR-0007, Amendment).
