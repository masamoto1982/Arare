# Traceability Matrix

Requirement → implementation → verification evidence, as
`PLAN_OVERVIEW.md` requires and `npm run check:traceability`
(`scripts/check-traceability-matrix.mjs`) checks in both directions.

The ID classes are `STANDARDS.md`'s. Arare writes this matrix before the code
it traces: a requirement row comes first, and code citing an ID the matrix
lacks fails the check.

## Matrix

Phase 0 has no Arare requirement yet; the high-level requirements arrive in
Phase 1. The only live row is inherited.

| Requirement | Statement | Implementation | Verification | QL |
|---|---|---|---|---|
| **AR-INH-001** | *Inherited from Ajisai (AQ-REQ-004).* The host runtime kind (web vs Tauri) is classified correctly from the build-time injection and the runtime DOM fallback. | `src/platform/index.ts` (`detectRuntimeKind`) | `src/platform/index.test.ts` — **AR-VER-001-A**; configuration in `vitest.config.ts`, which cites the suite as **AR-VER-001** | QL-D |

### Inherited rows

An `AR-INH` row covers Playground code Arare took over from Ajisai with its
tests but without an Arare requirement (`DEVELOPMENT_PLAN.md`). It keeps the
inherited statement word for word and names the Ajisai ID it came from. Phase 5
replaces each with a high-level requirement for the Playground shell, and the
`AR-INH` ID then moves to the retired section.

The rest of the inherited Playground code (`src/`) is not traced by any row;
that gap is PR-0001 in `PROBLEM_REPORTS.md`.

## Unassigned requirement IDs

None.

## Retired verification IDs

None yet. An ID that goes with its subject is listed here, with the subject and
when it went, so that finding it in history is not confusing; the check keeps a
retired ID out of the source.
