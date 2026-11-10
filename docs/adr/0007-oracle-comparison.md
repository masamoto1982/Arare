# ADR-0007: Comparing Arare with the Ajisai oracle

Status: Accepted (Phase 2b, 2026-10-28). Decides WORK_ORDER.md Q-03.
Amended 2026-11-10: the oracle moved to Ajisai 1.0.0-beta.4, which takes
ELSE on UNKNOWN and has a SUM that skips NIL; see "Amendment" below.
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

## Amendment (2026-11-10): the SQL divergences are gone

Ajisai adopted both SQL rules in 1.0.0-beta.4 (masamoto1982/Ajisai#1920,
merged as `319fe84`): SELECT takes its second candidate when the condition is
UNKNOWN, as `CASE WHEN` takes ELSE, and a new Standard Word SUM adds the
present elements of a vector, skipping NIL, and is NIL when none is present.
The owner's direction is that the two languages differ only in rounding.

- The oracle is pinned to `319fe84` (`docs/quality/CONFIGURATION_INDEX.md`):
  the Playground bundle, the Node build, the MCP server and the benchmarks'
  `ajisai-core`.
- The prelude no longer defines SUM; Ajisai's own SUM is used. QUANTIZE,
  RESCALE, NARROW and NEG stay as before.
- The `sqlBranch` and `sqlAggregate` cases are now ordinary cases, compared
  within their budget, with more cases on UNKNOWN lanes, an UNKNOWN from an
  order, and NIL skipped after a rounding. The generated programs
  (AR-VER-217) now write UNKNOWN conditions and NIL lanes in summed vectors.
- `overflow` (D-09) is the one divergence left, and it is the rounding
  difference itself: Arare's widest width is finite, Ajisai's numbers are not.
- D-07 and D-08 are unchanged as Arare's decisions; what changed is that
  Ajisai now agrees with them. The Lean proofs (AR-VER-106) remain their
  primary verification, and the oracle is now a second, independent check.
