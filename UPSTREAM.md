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

Ajisai stays the cross-check Arare is compared with, as Arare is Ajisai's (WORK_ORDER.md §1.3, ADR-0015); "oracle" in names means that cross-check. A later
pin of the oracle — a Git dependency on `ajisai-core` or the WebAssembly bundle
under `src/wasm/` — names its own upstream commit where it is declared.
Since 2026-11-12 the oracle is pinned to Ajisai `dd5d1b222f11b72e8aa1664ad68b890dcafc06c4`
(1.0.0-beta.4, the head of masamoto1982/Ajisai#1922, which makes every lifted
Word align at the innermost axis; to be moved to its merge commit once it
merges); every place it is pinned is listed
in `docs/quality/CONFIGURATION_INDEX.md` (ADR-0007, Amendment).
