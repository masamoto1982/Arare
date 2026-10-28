# ADR-0002: Widening and the rounding points

Status: Accepted (Phase 1, 2026-10-28). Decides WORK_ORDER.md §3.3, with one change.
Requirements: AR-HLR-003, AR-HLR-004, AR-HLR-007, AR-HLR-008, AR-HLR-009.

## Context

WORK_ORDER.md §3.3 proposed that arithmetic widens until a program says where
to round, with RESCALE and NARROW as the only rounding points. Writing the
requirements showed that proposal cannot hold for division: the quotient of
two fixed-point numbers is in general not on any fixed-point grid, so DIV has
to round as well.

## Decision

- ADD, SUB, MUL and NEG are exact. Their result takes the narrowest width that
  holds its statically inferred range; operands of different scales are
  aligned by an exact shift.
- **RESCALE, NARROW and DIV are the rounding points**, and the only ones.
  RESCALE and DIV round half to even; NARROW does not round but may overflow.
- A result outside the finite range of the widest width, or of NARROW's
  target, is 1/0. Nothing wraps around and nothing saturates.

## Alternatives for DIV

- **Exact rational quotient, rounded at the next RESCALE.** Rejected: it brings
  back the unbounded intermediate values Arare exists to avoid.
- **Integer division (truncation).** Rejected: biased, and a second rounding
  rule beside round half to even.
- **No DIV; multiply by a quantized reciprocal.** Rejected as the only form: it
  rounds twice. It remains an optimisation the compiler may use where it is
  proved equal.

## Consequences

CLAUDE.md's summary of the semantics names DIV among the rounding points.
Associativity of addition holds for exact values but not across an
intermediate overflow (`Expr.machine_add_not_assoc`); the error budget of
Phase 3 has to account for that, not assume it away.
