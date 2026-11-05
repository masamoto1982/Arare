# ADR-0011: Coarsening the checker's rationals

Status: Accepted (Phase 4b, 2026-11-04). Extends ADR-0008, whose decision stands.
Requirements: AR-HLR-027, AR-LLR-019, AR-LLR-030.
Verification: AR-VER-107 (`coarsen_sound`), AR-VER-215, AR-VER-229.

## Context

The checker computes on exact rationals of 128 bits and answers nothing when
one would not fit (ADR-0008). A loop that multiplies doubles its rationals'
size each step: twenty steps of the logistic map hold rationals of a million
bits, and the checker knew nothing of AR-PERF-003's program after five. A
requirement that names "the inferred budget" needs a budget to exist.

## Decision

After every numeric Word the checker coarsens: where a denominator of the
range or the error exceeds 2^48, the range is rounded outward and the error
up to multiples of 2^-48; a denominator within the grid is kept. The model is
Lean's `coarsen`, with `coarsen_sound` proving a coarsened `Info` right about
whatever the original was right about; the Rust function is held to it by the
table diff, which now prints coarsening at three grids.

## Alternatives

- **Bigger rationals.** Arbitrary precision would carry the million bits, at
  a cost that grows with every step, for precision no fixed-point value has.
- **Intervals in floating point.** Sound only with directed rounding, which
  the Lean model would have to describe; dyadic rationals are exact and the
  model already has them.
- **A coarser grid.** 2^-48 is far below the 2^-31 of the finest scale, so
  coarsening never decides whether a budget exists, and two such rationals
  multiply within 128 bits.

## Consequences

Budgets the checker inferred before are unchanged where their rationals had
small denominators, as every corpus case's had. A long loop's budget is up to
2^-48 looser than the exact analysis, which could not finish.
