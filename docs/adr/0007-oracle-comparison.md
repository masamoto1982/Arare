# ADR-0007: Comparing Arare with the Ajisai oracle

Status: Accepted (Phase 2b, 2026-10-28). Decides WORK_ORDER.md Q-03.
Verification: AR-VER-209 (`scripts/check-oracle.mjs`, `tests/oracle/corpus.json`).

## Context

Ajisai is the oracle Arare is checked against (WORK_ORDER.md §1.3): on the same
source, Arare's result must lie within its error budget of Ajisai's exact one.
Three things stand in the way of comparing naively: Ajisai has no QUANTIZE,
RESCALE, NARROW, NEG or SUM; Arare departs from Ajisai on purpose in two places
(a branch on UNKNOWN takes ELSE, D-07; aggregates skip NIL, D-08); and Arare
overflows to 1/0 where Ajisai's exact value simply grows (D-09). Error budgets
are inferred only in Phase 3.

## Decision

- **The comparator side absorbs the differences**, not the Ajisai side: the
  harness runs Ajisai with a prelude that defines Arare's extra Words as their
  exact counterparts (QUANTIZE, RESCALE and NARROW keep the exact value; NEG is
  `-1 MUL`; SUM is a FOLD of ADD) and changes nothing else.
- **A deliberate divergence is a recorded case, not an exclusion.** A corpus
  case marked `divergence` names its kind (`sqlBranch`, `sqlAggregate`,
  `overflow`), the decision behind it, and Arare's expected answer; the harness
  checks that Arare gives exactly that answer and that Ajisai still answers
  something else, so a divergence can neither drift nor disappear unnoticed.
  The laws behind the two SQL divergences are proved against the Lean model
  (AR-VER-106), not against Ajisai.
- **Tolerances are written by hand until Phase 3.** Each case states the
  largest absolute error allowed, worked out from the rounding points it passes
  through; Phase 3 replaces them with the inferred budget, and the corpus then
  checks the budget checker's soundness empirically. (Done in Phase 3,
  ADR-0008: the comparison uses the inferred budget, and the hand-written
  figure, renamed `budget`, is the largest the checker may infer.)
- **The oracle is the committed Node build** in `tools/mcp-server/wasm/
  generated/`, from the commit `UPSTREAM.md` records, so the comparison needs no
  network and no Ajisai build.

## Alternatives

- **An Ajisai-side prelude that makes Ajisai behave like Arare** (SELECT on NIL
  → ELSE, SUM skipping NIL). Rejected: the oracle would then be checking itself
  against Arare's own decisions.
- **A Rust dev-dependency on `ajisai-core` pinned by commit.** Rejected for
  now: it compiles Ajisai's whole engine into every test build, for the same
  answers the committed build gives.
- **Excluding divergent programs from the corpus.** Rejected: an excluded
  behaviour is an unchecked one.
