# Ajisai host resource profiles

Status date: 2026-08-13. Updated 2026-08-14: the playground / native-default
profile's ceilings are now derived, not chosen. The hand-off memo that set the
work up and the dated record of carrying it out were retired from `docs/dev/`
on 2026-09-30 (the version history holds both); what this page still relies
on from them is folded in below — the reasoning behind each number, the
rejected alternatives, and the correction the record's §10 made to the first
derivation: it had calibrated on native release and applied the result to the
WASM playground, which is not the same engine and — for one meter — not even
a good proxy for it. The numbers below are the corrected, WASM-calibrated
ones, and the doc comments in `rust/src/interpreter/host_profile_defaults.rs`
carry each constant's own derivation.

Ajisai's resource ceilings are **host safety controls, not language semantics**
(`SPECIFICATION.html` §2.5). A conforming host chooses its own, and all
conformance must pass under the documented defaults. Two official hosts
therefore disagree about them on purpose.

That is only safe when each host says what it applies. This page is the
comparison; each host also publishes its own profile at runtime, so nobody has
to trust a document that could drift:

| host | how to read the applied profile |
|---|---|
| MCP stdio server | the `ajisai://limits` resource; every tool result names the profile in `mcp.limitProfile`, and a ceiling that fires is named with its value in `diagnosis.resourceLimit` |
| browser playground | the `profile:` badge in the header (hover for the full table); `AjisaiInterpreter.host_profile()` |
| native CLI (`ajisai run`) | interpreter defaults, `--step-limit` overrides the step budget |
| native CLI (`ajisai agent …`) | the same profile the MCP server applies |

## The profiles

| ceiling | MCP / `ajisai agent` | playground and `ajisai run` | what it bounds | derived? |
|---|---:|---:|---|---|
| `sourceBytes` | 65,536 | 16,777,216 | byte length of one program | judgment call (§ below) |
| `executionSteps` | 100,000 | 12,180,000 | Words executed | **yes**, from the host time budget |
| `materializedElements` | 100,000 | 1,000,000 | elements one generative Word may build | no — size, not time (§ below) |
| `numericLiteralDigits` | 4,096 | 4,096 | digits in one numeric literal | no |
| `numericWork` | 10,000,000 | 311,190,000 | accumulated arithmetic work, in limb-multiply units, in every operand shape | **yes**, from the host time budget |
| `collectionWork` | 20,000,000 | 1,414,860,000 | accumulated element operations inside collection Words — copies, order comparisons, equality probes | **yes**, from the host time budget |
| `bigintBits` | 262,144 | 1,000,000 | coefficient width of one exact arithmetic result | no — size, not time (§ below) |
| `algebraicTerms` | 512 | 10,000 | term count of one exact algebraic value | no — size, not time (§ below) |
| `nestingDepth` | 256 | 256 | containers one value nests, however it was built | no — the native stack a value walk descends, not time |
| `callDepth` | 256 | 256 | User Word calls and block evaluations active at once | no — the native stack a call descends, fixed by the engine for every host |

Both profiles' triple of derived ceilings bound the same amount of *time*, not
the same number of units — the paths do not count the same thing, so setting
them equal would have made one of them mean something different from the
other. Each host has its own single time budget, and each derived ceiling is
that budget times the meter's own measured floor rate (its slowest unbounded
path, in units/ms or steps/ms, **on the engine that host actually runs**):

| host | time budget | `numericWork` floor rate | `collectionWork` floor rate | `executionSteps` floor rate |
|---|---:|---:|---:|---:|
| MCP / `ajisai agent` | 5,000 ms (`wallTimeMs`, adapter-enforced) | 14,465 units/ms | 30,800 units/ms | (not derived; see below) |
| playground / `ajisai run` | 30,000 ms (`DEFAULT_HOST_TIME_BUDGET_MS`) | 10,373 units/ms | 47,162 units/ms | 406 steps/ms |

**The playground row is measured on WASM** (`scripts/wasm-profile-calibration.mjs`,
against the same compiled module the browser ships — Node and Chrome are both
V8), not native release. That correction matters: measured back to back on
one container, native ran `numericWork`'s floor path 2.1x *faster* than WASM,
`collectionWork`'s 4.0x faster, and `executionSteps`'s dispatch loop 0.66x —
*slower* — than WASM. A ratio that swings from "twice as fast" to "a third
slower" depending on the meter means a single native calibration cannot make
three ceilings bound equal time on the engine they actually run on, which is
the entire point of deriving them from a shared time budget. The first
attempt at this derivation (see the git history of this document) calibrated
on native and got playground numbers 2-4x smaller than the ones above for
`numericWork`/`collectionWork` and roughly half for `executionSteps`; the
same review also found that `Fraction::hash` ran a gcd that was superlinear
in element width, which had made the wrong collection path the floor. Both
were corrected before the numbers above were taken.
`rust/examples/work_meter_calibration.rs` and
`rust/examples/collection_word_calibration.rs` remain the right tools for
native's own floor — `ajisai run` and native `ajisai agent` really do run on
that engine — just not for this table's playground column.

The two hosts' floor rates were measured on different containers, different
engines, and (for MCP) before the scan family was de-quadraticized and the
`Fraction::hash` gcd fix landed — they are not comparable to each other; only
within a host, between its own rows, does a ratio mean anything. The
playground's own `numericWork`/`collectionWork` floor rates are *not* a clean
multiple of one another (47,162 / 10,373 ≈ 4.5) the way the MCP profile's used
to look (30,800 / 14,465 ≈ 2.1, once hard-coded as
`collectionWork = 2 × numericWork`). That 2× relationship was itself a
derived-but-coincidental ratio of two measured rates, not a rule, and moved
twice on 2026-08-14 — once when the scan family was de-quadraticized (the
de-quadraticization record had predicted that the witness programs and
boundary sources built to reach `collectionWork` cheaply through a large
distinct count would stop reaching it, and they did: every one was replaced
by a repeated cheap small-integer scan), and again when the `Fraction::hash`
fix changed which collection path was the floor. Each derived ceiling now comes independently
from its own host's time budget and its own measured floor rate; the ceilings
only need to agree on the *time* they bound, and they do that by construction.

`DEFAULT_HOST_TIME_BUDGET_MS` (30 s) is a **judgment call, not a
measurement** — nobody has usage data on how long an Ajisai learner tolerates
a running computation before assuming it is stuck. It is several multiples of
the classic ~10 s "the UI looks frozen" threshold (Nielsen/Miller), extended
because the playground shows a running state and an abort control rather than
giving no feedback at all — see the constant's doc comment in
`rust/src/interpreter/host_profile_defaults.rs` (`DEFAULT_HOST_TIME_BUDGET_MS`) for
the full reasoning, and treat it as replaceable the moment real usage data
exists.

**`executionSteps` is now derived too**, closing the one ceiling that used to
be identical across hosts for no stated reason (a shared Rust constant nobody
had threaded a playground-specific value through, not a decision that the two
hosts should agree). It is derived the same way as the two work ceilings — the
host time budget times a measured floor rate — except the unit being priced is
raw word-dispatch count rather than operand size, and the floor path is
therefore a long loop of the *cheapest* word rather than the widest operand
(measured at 406 steps/ms **on WASM** for a trampolined user-word call, the
dispatch-bound analogue of `numericWork`'s "dense tensor lanes"; see
`scripts/wasm-profile-calibration.mjs`). The MCP profile's own
`executionSteps` is untouched: it is the agent profile's own constant
(`agent::api::LOCAL_AGENT_EXECUTION_STEPS`, 100,000), which `ajisai agent
compute` applies unless `--step-limit` says otherwise and which every MCP call
site threads explicitly as well (`tools/mcp-server/index.js`
`LIMITS.executionSteps`), independent of whatever the interpreter's own
default is, so raising the playground/native default did not move it.

The 120x gap between the two step budgets is the one a program notices first
when it moves from the playground to an agent, because a block iteration costs
one step per element *for every Word the block runs* (literals in the block
cost nothing): `0 99999 RANGE 0 [ ADD ] FOLD` walks 100,000 elements through a
one-Word block and is refused by `executionSteps` under the MCP profile, and
`[ 3 MUL 1 ADD ] MAP` over 50,000 elements spends the same 100,000 steps
through its two Words. The same sum as a vector operation (`V V ADD`, or
`RANGE` and `FOLD` over a vector a tenth the size) runs in a handful of steps.
The materialization ceiling admits a vector that the step ceiling cannot walk
element by element: under this profile, block iteration is for a few tens of
thousands of elements divided by the Words in the block, and vector arithmetic
for the rest. The quickstart says so where an agent reads it, and the
playground's Playground badge tooltip shows the agent profile's ceilings
beside its own (`AjisaiInterpreter.agent_host_profile()`); the splash names
both profiles only.

**Three ceilings are deliberately *not* derived from the time budget, and stay
at their prior values (mostly) or a differently-reasoned one:**

- `materializedElements` and `bigintBits` bound the size of *one value*, not
  accumulated time, so a time budget has nothing to say about them. The right
  basis would be something like "elements a browser can hold and a human can
  still make sense of on screen" — nobody has measured that, so both stay at
  their prior values, re-checked only for the weaker property of staying
  reachable inside the new `numericWork`/`collectionWork` budgets (they do:
  see the doc comments on `DEFAULT_MAX_MATERIALIZED_ELEMENTS` and
  `DEFAULT_MAX_BIGINT_BITS` in `host_profile_defaults.rs`).
- `algebraicTerms` moved (100,000 → 10,000) for the same reason it moved on
  the MCP side once before: the old value stopped being *live* once
  `numericWork` shrank below what it needs — the doubling that first exceeds
  100,000 terms costs 520,099,280 units (a fixed cost from the pricing
  formula, independent of which host measured the floor rate), which is over
  even the current, WASM-calibrated 311,190,000 budget, so `numericWork`
  would always answer first and the term ceiling would never fire
  (`profile_liveness_tests` exists to catch exactly this). 10,000 is chosen
  the same way the MCP profile's 512 was: live with comfortable margin (the
  crossing cascade costs ~16% of the current work budget), not derived from a
  stated size-legibility criterion either, and left at 10,000 rather than
  raised back toward 100,000 now that the budget has grown — the value was
  never meant to track the budget's size, only to stay live under it.
- `sourceBytes` (playground: 64 MiB → 16 MiB) is a **judgment call with a
  stated anchor**, replacing a round number that had no stated reason at all:
  16 MiB is about 9× the largest known legitimate machine-generated Ajisai
  program (the perf-benchmark's ~1.77 MB chain), generous in the same
  direction the old 64 MiB was, but for a reason someone can point to instead
  of "large enough". Not a measurement of what a human actually pastes into a
  textarea — nobody has done that study either.

None of the above changes anything about *why* the two profiles are allowed to
differ — `SPECIFICATION.html` §2.5 still says a conforming host chooses its
own ceilings, and the rejected alternatives are unchanged. They were settled
in the 2026-08 hand-off and are recorded here so they are not re-argued:

- **Align upward (the playground's values for MCP too).** A hundredfold
  `numericWork` puts the 5-second wall clock back in front, and the adapter
  can no longer say *what* was expensive by ceiling name — the 2026-08-13
  work is lost as it stands.
- **Align downward (MCP's values for the playground too).** The playground
  protects only "me, on my own CPU"; there is no gain from tightening it, and
  `[ 0 100001 ] RANGE` refused in one's own tab makes the teaching surface
  narrower than the language.
- **One profile.** `SPECIFICATION.html` §2.5 lets each host choose. The two
  hosts have different threat models (someone else's machine, concurrency,
  a caller waiting on a wall clock — versus one's own tab, single, closable at
  any time).

What was decided instead is to unify the *derivation*, not the values: the
two profiles keep different numbers, and what is fixed, written down and
checked is which function of what each difference is. What changed is that
"the playground is generous because 67 seconds felt right" is no longer the
explanation for any number in this table.

The MCP server declares four further ceilings that exist only at the adapter,
because they bound the *call* rather than the computation: `wallTimeMs`
(5,000), `responseBytes` (1,048,576), `concurrentExecutions` (4), and the
adapter-side `sourceBytes` check that rejects an oversized program before a
backend is entered. The playground has no equivalent: it runs in the user's
own tab, at their own pace, for their own eyes.

## Intended divergences

These are decisions, not defects. Each is recorded where it can be seen:

- **`0 100001 RANGE`** succeeds in the playground, materializing 100,002
  elements, and is refused under the MCP profile as `resourceLimitExceeded`
  naming `materializedElements`. The divergence is a failure on one host and
  an answer on the other, never two answers: a crossed ceiling is an ERROR,
  not a value, precisely so that two conforming hosts cannot answer one
  program differently. Pinned as a golden case with an explicit
  `hostDivergence` block (`tools/mcp-server/golden/cases.json`).
- **A 1 MiB program** runs in the playground and is refused by the MCP server
  as `sourceTooLarge`.
- **A computation past MCP's 5-second-equivalent work budget** completes in
  the playground and is refused by the MCP server. Since the work meters
  cover every expensive path it is refused *by name* — `numericWork` or
  `collectionWork` — rather than by the clock; the playground's budgets are
  several times to over an order of magnitude larger (§ above; the exact
  ratio moves with re-measurement, unlike the old "100x" which was itself
  just the pre-derivation numbers' coincidence), so the same program runs
  there.

The practical consequence is worth stating plainly: **a program prototyped in
the playground can behave differently through MCP.** It will never compute a
*different value* — that is what the shared value protocol and the backend
parity tests guarantee — but it can succeed in one host and answer a
reason-carrying `NIL`, or fail as a host error, in the other. Prototype
against the tighter profile if the program is destined for MCP.

## What each MCP-declared limit is actually pinned by

`tools/mcp-server/golden/limits.json` holds one entry per declared limit, and
`selftest.js` fails if that set and the served `LIMITS` set ever differ — so a
newly declared ceiling cannot be added without saying how it is exercised.
Three kinds of coverage appear there:

- **`boundary`** — a real source just under the declared value that succeeds,
  and one just over it that fails in the stated way. Run against the live
  server on every self-test, and compared across both backends in
  `backend/parity-test.js`. Covers `sourceBytes`, `responseBytes`,
  `executionSteps`, `materializedElements`, `numericLiteralDigits`,
  `numericWork`, `collectionWork`, `bigintBits` and `algebraicTerms`.
- **`hostGate`** — enforced by the adapter's admission path rather than by a
  program, and exercised through that path in `selftest.js`. Covers
  `concurrentExecutions` and `wallTimeMs`.
- **`injectedLimit`** — pinned in Rust with a small injected ceiling, because
  the declared value is not reachable within `wallTimeMs`. Covers nothing at
  present.

Every declared ceiling now has coverage of one of the first two kinds, which
was not true for most of this document's life. Three earlier revisions recorded
the opposite state, and the reasons are worth keeping because each was a
different defect:

- `numericWork` was charged only on the scalar arithmetic path, so any operand
  in vector or tensor shape ran free — a ceiling turned off by a representation
  decision the language says is unobservable. It is now charged at one entry
  for every shape, priced as lanes × limb-multiply units.
- `algebraicTerms` was declared at 4,096, which the work budget could not pay
  to reach: the doubling that first exceeds it costs 16,780,400 units against
  10,000,000, so `numericWork` always answered first. Lowered to 512, and
  `rust/src/agent/profile_liveness_tests.rs` now fails the build if any size
  ceiling is declared past what the work budget can reach.
- `wallTimeMs` really was the ceiling that arrived first for anything
  expensive, because the expensive things were unpriced. The last of them was
  the collection family: `[ 0 99999 ] RANGE UNIQUE` is quadratic and took 48
  seconds as *one* execution step, which is why the clock was the only thing
  that noticed. With `collectionWork` charging it, the same program is refused
  by name in 164 ms, and no source program reaches the clock any more — so
  `wallTimeMs` moved to `hostGate`, exercised against a backend built with a
  1 ms deadline.
- An algebraic sum was priced per term with `ALGEBRAIC_PAIR_UNITS`, the
  constant measured on products. One shape of sum ran *below* the floor: one
  whose every addend splits a composite radical (`√(p₁p₂) + √(p₃p₄) + …` then
  `√p₁`, `√p₃`, …) charged 51,880 units/ms natively, a tenth of the other
  paths, because each addition refined the whole basis and rebased every term.
  Rebuilding the sum (`rust/src/types/exact/algebraic_sum.rs`) to refine and
  rebase only what a merge actually splits took that shape to 3.4M units/ms
  and made every other sum an order of magnitude cheaper still, so the sum got
  its own constant, `ALGEBRAIC_SUM_TERM_UNITS` = 256: the dearest sum measured
  (`work_meter_calibration.rs`, `wasm-profile-calibration.mjs`) charges about
  1.5× the native floor and 3.5× the WASM one. The MCP `numericWork` boundary
  pair moved from 18/19 cascades to 19/20 with it.

What the work meters charge is documented where it is charged: the doc
comments in `rust/src/interpreter/collection_meter.rs` carry the collection
prices, and `rust/examples/work_meter_calibration.rs` /
`rust/examples/collection_word_calibration.rs` hold the measurements
themselves. Both examples are runnable: re-measure rather than trust the
constants when a representation changes. `collectionWork` is a separate
ceiling rather than a surcharge on `numericWork` for three reasons that were
settled when it was introduced (2026-08-13): the units do not convert
honestly (an element operation is 1/4 of a limb-multiply for a copy, 1/2.6
for a scan and 85 for an algebraic comparison — a 300-fold spread, so one
number would stop saying whether it was wide arithmetic or a large sort); the
instruction to an agent runs the other way (`numericWork` says "compute
less", the collection ceiling says "make the set smaller", and a quadratic
`UNIQUE` refused under `numericWork`'s name sends a model off to trim
arithmetic); and `wallTimeMs` already backstops the sum, so each meter's job
is to name what the time went on.

`UNIQUE` / `TALLY` / `GROUP` themselves stopped being a linear scan on
2026-08-14: a `Value: Hash`-backed `HashMap` lookup replaced it, and the
per-element charge was re-derived accordingly (the hash itself turned out to
cost enough to need its own unit, `COLLECTION_HASH_UNITS`). The ceiling values
in the table above did not move; the golden boundary source that reaches
`collectionWork` did. The two dated memos that recorded the billing design
and the de-quadraticization were retired on 2026-09-30; the version history
holds them.
