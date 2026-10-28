# ADR-0003: The wheel on fixed point

Status: Accepted (Phase 1, 2026-10-28). Decides WORK_ORDER.md §3.4.
Requirements: AR-HLR-005, AR-HLR-006, AR-HLR-007, AR-HLR-010, AR-HLR-011.

## Context

Ajisai computes in Carlström's wheel of fractions over ℤ. Arare's numbers are
integer codes, so the values it can hold are the part of that wheel of the
form n/1, 1/0 and 0/0.

## Decision

- Arare's arithmetic on those values is exactly Ajisai's wheel arithmetic
  restricted to them: the rules of AR-HLR-005, AR-HLR-006 and AR-HLR-007.
- That part of the wheel is closed under addition, multiplication and
  negation but **not under the reciprocal** (1/3 is not an integer). Arare
  therefore claims, and proves, only the wheel axioms that do not mention the
  reciprocal: the two commutative monoids, `(x+y)z + 0z = xz + yz`,
  `0·0 = 0`, `(x + 0y)z = xz + 0y`, and `0/0 + x = 0/0`.
- Equality is decided by denotation everywhere; order is decided only over
  finite numbers, and asking the order of a point is UNKNOWN, as in Ajisai.

## Alternatives

- **Claim the full wheel.** Rejected: false for fixed point, and CLAUDE.md
  rule 8 forbids claiming what is not proved.
- **Transreal arithmetic (signed infinities).** Rejected for the reason Ajisai
  gives: it gives up the involutive reciprocal and does not match the oracle.

## Consequences

That these operations agree with Ajisai's wheel on the shared values is
argued here, not proved: the Lean model does not contain Ajisai's wheel of
fractions. The oracle comparison of Phase 2 checks it empirically.
