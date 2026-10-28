# ADR-0004: Where widths and scales are written

Status: Accepted (Phase 1, 2026-10-28). Decides WORK_ORDER.md Q-01.
Requirements: AR-HLR-018.

## Context

Ajisai has nowhere to write a type. Arare needs every number's width and scale
known before it runs, so that the compiler can choose kernels and the budget
checker can bound errors.

## Decision

A number's width and scale are stated where the number enters a program — in
the form of its literal, or as the operands of QUANTIZE — and inferred
everywhere else from the rules of ADR-0002. Nothing else in a program states a
width or a scale. The literal forms and QUANTIZE's signature are low-level
requirements of the front end, written in Phase 2.

## Alternatives

- **Annotations on Words or blocks.** Rejected: it is the type system Ajisai
  chose not to have, and inference makes it unnecessary.
- **One global default width.** Rejected: the error budget would have no
  per-value information to work with.

## Consequences

The same source can run in Ajisai only where its literals are also Ajisai
literals; how a quantized literal is spelled so that Ajisai reads it as its
exact value is part of the Phase 2 front-end design.
