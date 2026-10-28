# Standards

The requirements, design and code standards every change follows: the roles
of DO-178C's SRS, SDS and SCS.

## Identifiers

| Class | Form | Lives in | Meaning |
|---|---|---|---|
| System requirement | `AR-SYS-nnn` | `WORK_ORDER.md` §2, then `spec/` | A decision about what Arare is for (D-01..D-17 become these in Phase 1) |
| High-level requirement | `AR-HLR-nnn` | `spec/` | What the language does, observable from a program |
| Low-level requirement | `AR-LLR-nnn` | `docs/design/` | How a component does it, precise enough to code from |
| Performance requirement | `AR-PERF-nnn` | `spec/` (WORK_ORDER.md §6 until then) | A speed the compiled route must reach, relative to a baseline measured in the same run |
| Inherited requirement | `AR-INH-nnn` | `docs/quality/TRACEABILITY_MATRIX.md` | Playground behavior taken over from Ajisai with its tests, pending a Phase 5 requirement |
| Verification case | `AR-VER-nnn`, optionally `AR-VER-nnn-X` | the test, harness or proof | One verification suite, and a case within it |

IDs are never reused and never renumbered to close a gap. An ID that goes is
moved to the matrix's retired section.

## Requirements standard

A requirement:

- states one thing, observable at its level, in the present tense;
- is verifiable: it says what result would show it is not met;
- names the quality level of the component that implements it;
- carries `derived: true` and a reason when it does not follow from a higher
  requirement;
- for QL-A, has a Lean statement in `formal/` that the requirement and the
  theorem both cite.

## Design standard

- A decision with alternatives is an ADR in `docs/adr/`, numbered, with the
  options considered and why one was chosen.
- A low-level requirement cites the high-level requirement it refines.
- The crate boundaries are WORK_ORDER.md §8; a new crate is an ADR.

## Code standard

- Rust: `cargo fmt`; `cargo clippy -D warnings`; `unsafe_code` forbidden
  workspace-wide (a crate that needs it records the exception in an ADR and
  in `.github/CODEOWNERS`).
- **File-size budget**: a hand-written Rust file stays at or under 500 lines,
  enforced by `npm run check:file-size`. A file allowed past it is a reviewed
  entry in `docs/quality/file-size-baseline.json`; the baseline starts empty.
- A function that implements a requirement cites its ID in its doc comment.
- Tests cite the verification ID they belong to, and the matrix names the file.
- TypeScript: `npm run check` and `npm run lint` clean.
- Comments are in English (WORK_ORDER.md D-16).
