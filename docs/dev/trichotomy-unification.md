# Trichotomy unification: what landed and what did not

Status: **non-canonical, policy record** (`[方針記録]`). Canonical source
remains `SPECIFICATION.html` / `spec/`; this document records a design
decision and its reasoning, not language semantics.

## The claim

The runtime trichotomy (`LANG.FAILURE.TRICHOTOMY`: a value, a reasoned
absence, or an error) and the static trichotomy `check` reports
(`LANG.CONTRACT.CHECK`: verified, cannot verify, violated) are the same
three-way classification applied at two different times — evaluation time and
check time. This is not a resemblance chosen for a tidy write-up. Three
independent pieces of structural evidence say so:

1. **Gap identifiers propagate through dependency inference exactly the way a
   NIL reason propagates downstream** (`LANG.FAILURE.PASSTHROUGH`). Phase 3's
   `AccumulatedContract::widen_with` merges a dependency's `gaps` into the
   caller's own set for the same reason `ADD1`'s NIL operand flows through
   unchanged: incompleteness (or absence) that originates in one place is
   still true of everything built on top of it. Nobody modeled the gap
   mechanism on NIL passthrough to make a point — it fell out of how
   "cannot verify" has to compose across a call graph, independently, and
   only turned out to have the same shape.
2. **The "value" case already exists as a separate tool.** `ajisai contract`
   (`agent::contract_report`) returns the *inferred* contract — precisely
   what "verified" means, since a declaration verifies exactly when it
   matches what inference derives. The trichotomy was, in effect, already
   split across two commands (`contract` for the value case, `check`
   for cannot-verify/violated) before this phase; stating the
   correspondence in one place is describing an existing fact, not
   introducing a new one.
3. **A gap identifier is the same kind of object as a NIL reason.** Both are
   stable ids for "why a well-formed partial operation produced nothing" —
   `domainMiss` names why `SQRT` had no answer for the radicand it was
   given; `gap.recursiveDependency` names why inference could not decide
   a word's contract. Human-readable text may be reworded around either
   without changing what a caller can rely on.

## What (a) is: output vocabulary and structure

`contractDecls.outcome` (file-level) and `contractDecls.declarations[]`
(per-declaration) state the correspondence directly, in the runtime's own
`value` / `nil` / `error` vocabulary, with `nil` carrying a `reason` (the gap
id) and `error` carrying a `category` (the literal `"contractViolation"`).
`findings` and `violated` are untouched — `outcome` and `declarations` are an
additive projection of the identical check result, not a replacement. The
per-declaration fold to the file-level `outcome` is derived from
`LANG.FAILURE`, not chosen: `error` propagates and halts, so it dominates;
`nil` flows downstream only once nothing halted, so it dominates over
`value`; no declarations (or none outstanding) is `value`. See
`docs/dev/agent-cli-output-contract.md` for the full shape and
`rust/src/agent/contract_gap.rs` (`CheckOutcome`, `fold_outcomes`) and
`rust/src/agent/contract_decl.rs` for the implementation.

The canonical spec gained one paragraph, in `LANG.CONTRACT.CHECK`
(`spec/language-semantics.md`), stating the same correspondence and — this
is the part that matters most — stating plainly that it does **not** make the
check evaluate the program: the correspondence classifies *outcomes*, not
*mechanisms*. Division by zero, a failed parse, and an out-of-range index
already share one outcome (NIL) while sharing no mechanism whatsoever; an
inference that could not decide joins that list on exactly the same terms.
`check` still runs no word body.

## What (b) is, and why it is not done

The stronger version of this idea is for `check` to return an
actual Ajisai `Value` — folding gap identifiers into the NIL reason registry
so a caller could read one back through `NIL-REASON`, the way any other
absence reason is read. That is **not** implemented here, and the reason is
technical, not a deferral of judgment:

The payoff of (b) is that a *checker result* becomes something an *Ajisai
program* can inspect and act on. That payoff exists only when the checker
itself is reachable from inside the language — i.e., when contract checking
is done by Ajisai code, not by this Rust implementation calling into it from
outside. Self-hosting is not on this roadmap. Until it is, (b) does not make
a checker result programmable; it only changes which JSON shape a value-typed
node happens to have, which (a) already provides in full (a `"nil"` outcome
already carries `"reason": "gap.xxx"` — a text-only or a schema-typed reader
gets everything (b) would additionally expose, just not as something an
Ajisai `VENT`/`NIL-REASON` call could touch).

(b) also has a real cost (a)'s change does not: it widens the NIL reason
vocabulary — currently a single, runtime-only registry — across two planes,
check time and run time, defined in two different places for two different
reasons. `spec/words.json`'s reason vocabulary would need to grow to
accommodate a family of ids that never occur at run time, and every consumer
of that vocabulary (the Reference, the reason-listing tooling, any host that
enumerates reasons) would need to know some of its entries are check-time
only. Paying that now, for a benefit that only exists once self-hosting
exists, is exactly the debt this repository's own discipline
(Phase 4 of `docs/dev/spec-impl-alignment-methodology.md`: an unimplemented
spec is noise, not a placeholder worth keeping) argues against.

**Revisit condition:** when any part of the checker is itself written in
Ajisai — the point at which self-hosting stops being hypothetical — (b) is
worth reconsidering on its merits at that time, against whatever the
self-hosting design looks like by then.

## An honest count

This is **not** a change from ten concepts to nine
(spec/language-semantics.md's own ten-concept list). Concept #2 (three
outcomes: value, reasoned absence, error) and concept #8 (a pre-execution
check of user declarations) both remain, unmerged, exactly as listed. What
changed is that concept #8 no longer carries an *implicit second definition*
of concept #2's trichotomy — its three results are now stated as the same
trichotomy read at a different time, rather than as three results that
happen, coincidentally, to number three. That is a reduction in *how many
places the trichotomy is defined*, not in how many concepts the language has.
"Fewer concepts" is not treated as a self-evident good here, and is not what
happened: reducing a duplicated *definition* is the improvement being
claimed, and that claim is deliberately no larger than that.
