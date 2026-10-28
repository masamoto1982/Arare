# Development Plan

How Arare is built, from requirement to executable. The phase plan itself is
WORK_ORDER.md §7; this file fixes the process every phase follows.

## Requirements first

1. A change starts from a requirement. A high-level requirement (`AR-HLR-nnn`)
   lives in `spec/` and says what the language does; a low-level requirement
   (`AR-LLR-nnn`) lives in `docs/design/` and says how a component does it.
   Their form is `STANDARDS.md`'s.
2. A requirement that does not follow from a higher one — one that exists
   because of a design choice — is a **derived requirement**. It carries
   `derived: true` and the reason, and its review asks whether it can affect
   the values a program computes.
3. Code that implements no requirement is not merged (CLAUDE.md, rule 1). The
   inherited Playground is the one standing exception, recorded as AR-INH rows
   in the matrix and re-based on requirements in Phase 5.

## From requirement to code

| Step | Output | Verified by |
|---|---|---|
| Write or change the requirement | `spec/` or `docs/design/` entry | Review against `STANDARDS.md`; for a QL-A requirement, a Lean statement in `formal/` |
| Record a decision the requirement depends on | `docs/adr/NNNN-title.md` | Review |
| Implement | Rust or TypeScript source citing the requirement ID | `VERIFICATION_PLAN.md` for its level |
| Trace | A row in `docs/quality/TRACEABILITY_MATRIX.md` | `npm run check:traceability` |

## Languages and environments

- **Rust** (`rust/`, the workspace of WORK_ORDER.md §8) for every engine
  component. The toolchain is pinned in `rust/rust-toolchain.toml`.
- **Lean 4** (`formal/`) for the semantic model and its proofs. The toolchain
  is pinned in `formal/lean-toolchain`.
- **TypeScript** (`src/`) for the Playground shell, built with Vite and, for
  the desktop app, Tauri.
- Every pinned version is listed in `CONFIGURATION_INDEX.md`.

## Language of the documents

Specifications (`spec/`), plans (`docs/quality/`), design (`docs/design/`) and
code comments are written in English; development notes (`docs/dev/`) and the
work order are written in Japanese (WORK_ORDER.md D-16).

## Branches and changes

Work happens on a branch and reaches `main` through a pull request. A pull
request names the requirement IDs it implements, updates the matrix in the
same diff, and is merged only with CI green (`QUALITY_ASSURANCE_PLAN.md`).
