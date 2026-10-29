# ADR-0008: The error-budget checker

Status: Accepted (Phase 3, 2026-10-29).
Requirements: AR-HLR-027 to AR-HLR-029, AR-LLR-018 to AR-LLR-022.
Verification: AR-VER-107, AR-VER-215 to AR-VER-218.

## Context

Arare rounds where Ajisai does not, and exists to say by how much
(WORK_ORDER.md §1.1): for every number a program computes, a bound on its
distance from the exact value Ajisai computes on the same source, and the
`field=closed` contract when nothing can overflow or divide by zero (D-12).
The bound must be inferred without running the program, because the compiled
route (Phase 4) runs on data the compiler never sees, and it must be sound,
because the checker is a tool that stands in for verification (§5.5): Phase 3
ends only when the soundness theorem is proved and at least 10,000 generated
programs show no violation against Ajisai (§7).

## Decision

- **The domain.** For each number the checker knows nothing, or that both
  meanings are the same wheel point, or a range `[lo, hi]` for the exact value,
  an error `err` bounding Arare's distance from it, and the range of scales it
  may have. Literals start exact (their quantization error is computed from
  the exact rational the parser keeps); ADD, SUB, MUL and NEG carry errors as
  the operations propagate them; quantized literals, DIV, RESCALE and QUANTIZE
  add half a unit of their result's scale; a result that may leave the widest
  width, or the width NARROW or QUANTIZE names, and a DIV whose divisor's range
  widened by its error reaches zero, are known as nothing. Truth values carry
  only whether both meanings agree; vectors of unknown length, after a FILTER
  whose predicate agrees, carry one description of every lane. A program is
  `field=closed` exactly when every numeric result is bounded as a number.
- **Proved once, in Lean, over the numbers.** `formal/Arare/Budget.lean`
  defines the two meanings of an expression language of literals and the
  numeric Words, the checker `analyze`, and proves `budget_sound`: whatever
  `analyze` says is right. `closed_safe` proves what `field=closed` guarantees.
  The proof needs only the rationals and `grind` of Lean's core library, so
  the Lean dependencies stay none.
- **The Rust checker is the Lean checker computed with checked 128-bit
  rationals.** Each numeric function in `rust/crates/arare-budget/src/info.rs`
  is the Lean definition of the same name. Every rational operation is exact
  or answers `None`, and `None` makes the result "nothing known"; the order is
  computed by integer parts and reciprocals of remainders, so it never
  overflows. The Rust checker therefore answers what Lean's `analyze` answers,
  or nothing — never a different bound. The two print the same table over a
  grid and CI diffs them (AR-VER-215).
- **The rest of the language is carried, not proved.** The stack machine,
  vectors, truth values, SELECT, FILTER, MAP, FOLD and SUM follow the
  interpreter's rules on what is known of each value (`check.rs`, `words.rs`),
  using Lean's `join` (proved sound) for lanes and loop iterations and a cross
  bound for a SELECT whose condition may differ between the meanings. That
  layer is tested and compared with Ajisai; `docs/quality/FORMAL_VERIFICATION.md`
  says so.
- **The oracle checks the inferred budget.** The comparison of ADR-0007 now
  holds Arare to the inferred budget instead of hand-written tolerances; a
  corpus case's hand-derived figure stays as the largest budget the checker may
  infer, so the checker cannot silently loosen. Generated programs (at least
  10,000, seeded) test soundness where the corpus does not reach.

## Alternatives

- **Big rationals or Mathlib in Lean.** Mathlib's `linarith` and `positivity`
  would shorten the proofs, at the cost of a large pinned dependency and a much
  slower Lean job; core Lean's `Rat` and `grind` proved every lemma.
- **Unbounded rationals in Rust** (a big-integer crate, or one written for
  Arare). Rejected for now: the workspace has no dependencies, a QL-A big-integer
  implementation would need its own verification, and checked 128-bit
  arithmetic loses only precision (nothing known), never soundness. Programs
  whose bounds outgrow 128 bits get no budget; the generated programs show how
  rarely that happens.
- **Outward-rounded floating or dyadic bounds.** Sound if every rounding goes
  the right way, but then the Rust checker would no longer compute the Lean
  checker's values, and agreement could not be checked by a diff.
- **Interval arithmetic without an error term** (one interval for Arare's
  value). Rejected: the budget is the distance between two meanings, which an
  interval of one of them does not bound.
- **Counting the budget by running the program on exact rationals.** That is
  Ajisai, not a static checker; it says nothing about code compiled for data
  not yet seen.
