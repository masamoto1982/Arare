# ADR-0016: Arare marks the comparisons its budget does not decide

Status: Accepted (2026-11-12, by the owner). Requirements: AR-HLR-039,
AR-LLR-044.

## Context

Arare's error budget bounds how far a number may be from the exact value the
program denotes (AR-HLR-027). A comparison turns that distance into a truth
value, and a truth value has no distance: it is either the exact meaning's or
the other one. The checker already knows which comparisons are safe — an LT or
EQ whose operands, widened by their budgets, are apart agrees in both meanings
(AR-HLR-029) — but it only said so through `± ?` on a truth value that reached
the stack. A comparison inside a SELECT's condition or a FILTER's predicate
left no trace, so a caller could not tell a program whose every branch was
exact from one that may have taken the other branch.

Measured on the comparisons of three workloads (the synergy notes,
c1-measurement.md), the budget decides 91.6% of pairwise comparisons and 95.5%
of comparisons with a threshold; no decided answer was wrong, and Arare alone
answered 1.3–1.7% of comparisons wrongly. Those are the ones worth naming.

## Decision

- **Arare marks, and only marks.** The checker records every LT and EQ whose
  answer it cannot show to agree; a program with none is *decided*.
  `arare check` and the engine command report the mark beside the field
  contract.
- **Arare calls no other implementation.** What to do with an undecided
  comparison — compute it again exactly, in Ajisai or anywhere else, widen the
  width, or accept it — is the caller's choice. This keeps Arare a language of
  its own (ADR-0015): it states what it knows of its own result, and nothing
  in it presumes a second engine.
- **The mark is static.** It is the checker's inference over every value the
  program may reach, not a record of one run, so it is sound for every input
  the checker covers and may mark a comparison that happened to come out
  right.

## Alternatives

- **Recompute undecided comparisons in Ajisai, inside Arare.** Rejected by
  the owner: it makes Ajisai part of Arare's semantics.
- **Mark lanes at run time.** A run knows each lane's actual operands and
  would decide 4–8 points more of the comparisons. Left for later: it needs
  the interpreter and the compiled route to carry the budget per lane, which
  costs on the fast path (AR-PERF).

## Consequences

`Analysis` gains `undecided`; `check --json` and the engine's `run` gain
"decided" and "undecided". The generated-program gate (AR-VER-217) reports how
many programs are decided and fails if one marked decided leaves a truth value
without a budget. Nothing is proved in Lean about the mark; it rests on the
checker's agreement rule, which is tested, not proved (AR-HLR-029).
