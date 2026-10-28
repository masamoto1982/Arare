# ADR-0001: Value representation

Status: Accepted (Phase 1, 2026-10-28). Decides WORK_ORDER.md §3.1, §3.2 and Q-06.
Requirements: AR-HLR-001, AR-HLR-002.

## Context

WORK_ORDER.md D-02 makes int8, int16 and int32 fixed point the default. Two
questions were left open: how the wheel points of D-09 are coded inside a
width (§3.1), and whether a 64-bit width exists (Q-06). Widening (ADR-0002)
needs a ceiling: without one, `int32 × int32` has nowhere exact to go.

## Decision

- A number has a width N ∈ {8, 16, 32, 64} and a scale exponent e; its value
  is code × 2^e. 8, 16 and 32 are storage widths; 64 is the widest width,
  reached only by widening.
- Scales are powers of two. A scale built from other primes (`2ᵃ3ᵇ5ᶜ7ᵈ`)
  stays an extension outside Phase 1.
- With half-range M = 2^(N−1): code −M is 0/0, code M−1 is 1/0, the finite
  numbers are [−(M−2), M−2], and code −(M−1) is unused. The finite range is
  symmetric, so negation never leaves it; 1/0 has no sign and is its own
  negation.

## Alternatives

- **Saturation without codes for the points.** Rejected by D-09: overflow
  would be silent.
- **Points outside the code (a side bitmap).** Rejected: every lane would
  carry a second validity-like bit, doubling the masks the kernels move, and
  the points would no longer be numbers (D-10).
- **No 64-bit width; overflow at int32.** Rejected: the product of two int32
  numbers, the most common accumulation, would overflow by construction.

## Consequences

Each width gives up two of its codes and leaves one unused, about 0.01 bits
for int8. The proofs (`formal/Arare/Codes.lean`) hold for every M ≥ 2, so they
cover all four widths at once.
