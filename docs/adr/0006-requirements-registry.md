# ADR-0006: The requirements registry

Status: Accepted (Phase 1, 2026-10-28). Addresses PR-0008 for the specification.

## Context

Ajisai's specification is authored HTML generated from semantic sources. Arare
needs its requirements to be checked by machine against the Lean model and the
traceability matrix from the first requirement on.

## Decision

- `spec/requirements.json` is the registry: one record per requirement, with
  its class, level, what it refines, whether it is derived and why, and its
  verification — the Lean theorems that prove it, or why a proof is not the
  evidence, and what is planned.
- `spec/REQUIREMENTS.md` and the requirement rows of
  `docs/quality/TRACEABILITY_MATRIX.md` are generated from it by
  `scripts/generate-requirements.mjs`.
- `scripts/check-requirements.mjs` checks the registry and its link to
  `formal/` in both directions; `scripts/check-formal-axioms.mjs` checks that
  every theorem rests only on Lean's standard axioms.

## Alternatives

- **Port Ajisai's specification generator.** Deferred, not rejected: a
  published specification and Reference come with the Playground in Phase 5,
  and may be generated from this registry then.
- **Requirements in Markdown only.** Rejected: nothing could check them.
