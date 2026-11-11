# ADR-0015: Arare as Ajisai's counterpart, not its dependent

Status: Accepted (2026-11-12, by the owner). Restates WORK_ORDER.md §1 and the
wording of D-05 and D-13; changes no semantics, no Word and no AR-PERF figure.
Requirements: AR-SYS-001, AR-SYS-005, AR-SYS-013, AR-SYS-014, AR-HLR-027,
AR-HLR-033, AR-HLR-035.

## Context

Arare began from Ajisai's content (D-17), and its work order described it in
Ajisai's terms: its motive was to solve Ajisai's slowness (§1.2), Ajisai was
"the standard of correctness, the oracle" (§1.3), its Words followed Ajisai's
unless Arare had a reason of its own, and its error budget was a distance from
"the exact value Ajisai computes". The owner decided on 2026-11-12 that Arare
is not a language premised on Ajisai but its counterpart: an antithesis of
equal standing.

## Decision

- **Each language holds one thing fixed and counts the other.** Ajisai fixes
  the value — it never rounds — and counts what that costs, in the work meter
  that stops a run at its ceiling. Arare fixes the cost — every value is one
  machine word of a width known before it runs — and counts what that loses,
  in the error budget. Speed is what Arare's thesis yields, not the reason it
  exists; the AR-PERF requirements stay as they are and say how fast.
- **The standard of correctness is the exact meaning, not Ajisai.** The error
  budget bounds the distance of Arare's value from the exact rational the
  program denotes — the exact meaning of `formal/Arare/Budget.lean`, the one
  `budget_sound` is proved against. Ajisai is an independent implementation of
  exact arithmetic, and comparing with it is a cross-check of Arare, as
  running Arare's programs is a cross-check of Ajisai (Ajisai's
  `tests/sister/arare-shared-core.json`). The scripts, directories and panels
  named `oracle` keep their names; "oracle" means the cross-check.
- **Shared Words belong to both languages.** A Word both languages have has
  one name and one meaning, and neither changes it alone: a change to a
  shared Word is made in both, or the Word stops being shared. A Word only one
  language has is that language's own (QUANTIZE, RESCALE, NARROW and NEG are
  Arare's). Where the two theses demand different behaviour, each language
  keeps its own, as a recorded divergence (ADR-0007), and neither is the
  deviation of the other.
- **The Playground holds two engines of equal standing.** Arare's Playground
  runs Arare by default and shows Ajisai beside it as the exact counterpart;
  nothing in the shell makes one engine the other's subordinate.

## Alternatives

- **Keep Ajisai as the oracle.** Rejected by the owner: it makes Arare an
  approximation of Ajisai rather than a language with its own thesis.
- **Drop the comparison with Ajisai.** Rejected: two independent
  implementations checking each other on the programs they share are worth
  more to each than either alone, and the comparison stays the empirical half
  of the budget's soundness.

## Consequences

WORK_ORDER.md §1, D-05 and D-13 and CLAUDE.md are reworded accordingly, with
the owner's approval this record carries. AR-HLR-027 names the exact meaning;
its proof (`budget_sound`) already was against it, so no theorem changes.
