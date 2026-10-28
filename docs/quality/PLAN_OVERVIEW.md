# Plan Overview

The top-level plan for how Arare is developed and verified: the role a PSAC
plays under DO-178C, scaled to a language project. It names the other plans,
the quality levels, and the life-cycle data each phase leaves behind.

**This is a DO-178C-inspired development quality framework. It is not, and
does not claim to be, a certification basis.** No authority has accepted it
and no airborne use is intended.

## Purpose of the software

Arare is a stack-based, vector-oriented language that quantizes by design and
accounts for the precision it gives up. Its reason to exist is speed: it must
solve the slowness of its sister language Ajisai (WORK_ORDER.md §1.2), so its
performance requirements (`AR-PERF-*`) carry the same weight as its functional
ones. Ajisai remains the oracle Arare is checked against (WORK_ORDER.md §1.3).

## The plans

| Plan | File | DO-178C counterpart |
|---|---|---|
| This overview | `docs/quality/PLAN_OVERVIEW.md` | PSAC |
| Development | `docs/quality/DEVELOPMENT_PLAN.md` | SDP |
| Verification | `docs/quality/VERIFICATION_PLAN.md` | SVP |
| Configuration management | `docs/quality/CONFIGURATION_MANAGEMENT_PLAN.md` | SCMP |
| Quality assurance | `docs/quality/QUALITY_ASSURANCE_PLAN.md` | SQAP |
| Requirements, design and code standards | `docs/quality/STANDARDS.md` | SRS, SDS, SCS |
| Formal methods | `docs/quality/FORMAL_VERIFICATION.md` | DO-333 supplement, as a reading |

## Quality levels

Every component is assigned a level. The level decides the evidence a change
to that component must carry (`VERIFICATION_PLAN.md`). The levels play the role
of DO-178C's software levels; they are named QL-A to QL-D so that nothing reads
as a claimed DAL.

| Level | Components | Why |
|---|---|---|
| **QL-A** | `rust/crates/arare-spec`, `arare-kernel`, `arare-budget`, `arare-codegen` | A defect here gives a program a wrong value, or certifies an error budget that does not hold, without any symptom |
| **QL-B** | `rust/crates/arare-front`, `arare-interp`, `arare-wasm`; `spec/host-protocol.schema.json` | A defect here misreads a program or misreports a value, which the QL-A checks or the oracle would usually expose |
| **QL-C** | `rust/crates/arare-cli`; `tools/mcp-server/` once Arare's own server replaces the oracle's (Phase 6); Arrow and SQL I/O | A defect here affects how results reach a host, not what they are |
| **QL-D** | The Playground (`src/`, `index.html`, `src-tauri/`), documentation | A defect here is visible to the person using it |

The Lean model in `formal/` is verification evidence, not a component, and is
held to the standard of the QL-A requirements it proves.

## Life-cycle data

| Data | Where it lives |
|---|---|
| High-level requirements (`AR-HLR-*`) | `spec/` (from Phase 1) |
| Low-level requirements and design (`AR-LLR-*`) | `docs/design/` (from Phase 1) |
| Decisions | `docs/adr/` |
| Source and executable object code | `rust/`, `src/`, built by CI |
| Verification cases and procedures | the test suites, the Kani harnesses, the Lean proofs, `docs/quality/VERIFICATION_PLAN.md` |
| Verification results | CI artifacts; a summary per release under `docs/quality/results/` |
| Traceability | `docs/quality/TRACEABILITY_MATRIX.md`, checked by `npm run check:traceability` |
| Configuration index and environment | `docs/quality/CONFIGURATION_INDEX.md` |
| Problem reports | GitHub issues from the quality template; the open Phase 0 gaps in `docs/quality/PROBLEM_REPORTS.md` |
| Accomplishment summary | `docs/quality/RELEASE_SUMMARY.md`, written per release |

## Phases and transition criteria

The phases, their content and their exit criteria are WORK_ORDER.md §7. A phase
is entered only when the previous phase's exit criteria hold, and every phase
additionally requires: CI green, the traceability check passing, the coverage
ratchet not lowered, and the phase's outcome recorded in
`CONFIGURATION_INDEX.md`.

## Independence

The session or person who writes a change is not the only one who verifies it.
A QL-A or QL-B change is reviewed, against its requirement and its tests, by a
different session or by the owner. Until a second maintainer exists the
fallback Ajisai used applies: no self-approval inside the pull request that
makes the change, and a 24-hour cooling period before merging a change that
touches a path in `.github/CODEOWNERS`.
