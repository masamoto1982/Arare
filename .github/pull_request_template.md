## Summary

<!-- Describe the change and intent. -->

## Quality Classification

- Highest impacted level: <!-- QL-A / QL-B / QL-C / QL-D (docs/quality/PLAN_OVERVIEW.md) -->

## Traceability

- Requirement(s): <!-- e.g., AR-HLR-00X; a change with none is not merged -->
- Verification evidence: <!-- tests, harnesses, proofs, CI jobs -->

## Quality Checklist

- [ ] Every change implements a named requirement, and the matrix rows are updated in this diff.
- [ ] `npm run check:rust`
- [ ] `npm run check:formal`, if `formal/` or a QL-A requirement changed
- [ ] `cargo kani --harness <name>` (in `rust/`), if a proved kernel changed
- [ ] `npm run check`, `npm run lint`, `npm test`, if TypeScript changed
- [ ] `npm run check:gates`
- [ ] MC/DC tables updated for modified boolean logic in QL-A code
- [ ] Performance requirements (`AR-PERF-*`) still met, if the compiled route changed
- [ ] Open problem reports this change affects have a disposition

## Notes

Arare follows a DO-178C-inspired development quality framework and does not claim DO-178C certification.
