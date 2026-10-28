# ADR-0005: Exporting 1/0 and 0/0 to SQL

Status: Accepted (Phase 1, 2026-10-28). Decides WORK_ORDER.md §3.5.
Requirements: none yet; the export is Phase 6 work and its requirement is written then.

## Decision

When a number crosses into SQL or Arrow, 1/0 and 0/0 become SQL NULL, and the
Arrow field's metadata records, per column, which lanes were 1/0 and which were
0/0, so that a round trip through Arrow restores them.

## Alternatives

- **Floating-point Infinity and NaN.** Rejected: Arare's columns are integer
  columns, and changing their type on export would lose the codes.
- **Refuse to export a column holding a point.** Rejected: one overflowed lane
  would block a whole table.
