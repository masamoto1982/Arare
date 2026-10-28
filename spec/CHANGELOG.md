# Ajisai specification changelog

Status: canonical record of how the specification version moved. Each
`## <version>` section names what changed between it and the version below
it. Every entry is one bullet that opens with its kind in bold:

| Kind | Meaning | Version |
| --- | --- | --- |
| `breaking` | Some program's answer, or something a host receives, changes | Takes a new version |
| `compatible` | Something is added; no existing program's answer changes | Takes a new version when the machine-read sources change |
| `editorial` | The text changes and the language does not | No new version |

After the kind come the clause (`LANG.*`), one line of the change, and where
it applies a program with its answer before and after. `npm run
check:spec-version` holds this file, `spec/version-lock.json` and the
specification to each other (scripts/lib/spec-version.mjs states the rule).

## 1.0.0-beta.3

- **editorial** LANG.COLLECTIONS.BUDGET — `CHARS` and `TOKENIZE` are named among the Words that honor the materialization ceiling. Their parts were already a generated collection, which LANG.MACHINE.LIMITS bounds; the implementation now refuses them by that ceiling before cutting, where it used to build every part. (2026-10-22)
- **breaking** LANG.SOURCE.CODE — A block run as code is written (a Vector literal of the code running it, a Vector inside one, or a block naming no Word) or built from data, and a block built from data is refused as `error:nestedDataBlock` while another runs. `[ 1 COLLECT [ 0 0 ] GET EXEC EXEC ] 1 COLLECT [ 0 0 ] GET EXEC EXEC` was `error:resourceLimitExceeded` (`callDepth`) and is `error:nestedDataBlock`. A block that chose its own base case with `SELECT` and printed `3 2 1 0` now prints `3 2` and is refused. Code that runs a block it is given cannot be given a block that does the same: with `[ 'F' BIND 1 F EXEC ] 'APPLY' DEF`, `[ [ 2 ADD ] APPLY ] APPLY` was `3` and is refused; a chain of blocks bound in one frame and run from inside each other is refused at its second link. The error category `nestedDataBlock` is new. (2026-10-15)
- **editorial** LANG.DICTIONARY.ACYCLIC — Termination rests on the dictionary's acyclicity and on LANG.SOURCE.CODE's rule together; the claim that Ajisai is total, false in 1.0.0-beta.2 because a block could run a copy of itself, holds again. `spec/termination.json` states the measure. (2026-10-15)
- **editorial** LANG.MACHINE.LIMITS — The call-depth ceiling no longer names self-application among what reaches it. (2026-10-15)

## 1.0.0-beta.2

Recorded after the fact for the changes made between 2026-10-08 and
2026-10-15. Through that week the specification version stayed
1.0.0-beta.1 while the language changed under it; the entries below state
the net change from 1.0.0-beta.1 as labelled on 2026-10-08. Two designs
were adopted and withdrawn inside the week without reaching a release —
Belnap four-valued truth with a `BOTH` Word (2026-10-10 to 2026-10-12) and
a signed point `-1/0` (2026-10-09 to 2026-10-12) — and are not entries.

- **breaking** LANG.VALUES.EXACT — Division is total over the wheel of fractions. A quotient by zero is a number, and every pair over zero reduces to `1/0` or `0/0`. `1 0 DIV` was `NIL(divisionByZero)` and is `1/0`; `-5 0 DIV` is `1/0`; `0 0 DIV` is `0/0`. The NIL reason `divisionByZero` no longer exists. (2026-10-09, 2026-10-12)
- **breaking** LANG.VALUES.EXACT — `0/0` absorbs every operation, `POW` in either operand included: `2 0/0 POW` is `0/0`. An exponent of `1/0` projects `domainMiss`. (2026-10-10)
- **breaking** LANG.VALUES.ORDER — Neither point over zero has an order. A comparison, `MIN`, `MAX`, `SORT`, `ORDER` or `BSEARCH` that reads one projects `domainMiss`: `0/0 1 LT NIL-REASON` is `'domainMiss'`. (2026-10-12)
- **breaking** LANG.VALUES.ORDER — Strings are ordered by Unicode scalar value, and `SORT`, `ORDER`, `LT`, `GT`, `MIN`, `MAX` accept them. `'abc' 'abd' LT` was `error:nonNumeric` and is `TRUE`. (2026-10-13)
- **breaking** LANG.VALUES.ORDER — `SORT` and `ORDER` set every NIL aside and place it last. `[ 3 NIL 1 ] SORT` was `error:nonNumeric` and is `[ 1/1 3/1 NIL ]`. (2026-10-13)
- **breaking** LANG.VALUES.ORDER — `UNIQUE`, `TALLY` and `GROUP` gather every NIL under one key whatever its reason, as SQL's DISTINCT and GROUP BY gather NULLs. `NIL -1 SQRT 2 COLLECT UNIQUE LENGTH` was `2` (two reasons, two keys) and is `1`. Record keys are grouping keys by the same rule. (2026-10-13)
- **breaking** LANG.VALUES.TRUTH — `RECONCILE` is a new Core Word: what two sources agree on, and `NIL(conflict)` for two different values. `TRUE FALSE RECONCILE NIL-REASON` is `'conflict'`. The NIL reason `conflict` is new. A program that defined its own `RECONCILE` no longer runs. (2026-10-10, 2026-10-12)
- **breaking** LANG.MACHINE.LIMITS — A request past a host ceiling is an ERROR naming the ceiling, never a value. `0 100001 RANGE` under the MCP profile was `NIL(spaceExhausted)` and is `error:resourceLimitExceeded` with `resource: materializedElements`. The NIL reason `spaceExhausted` no longer exists. (2026-10-10)
- **breaking** LANG.AUTHORITY.IDENTITY — `INDEX-OF` and `MEMBER?` are retired and are unknown Words; `spec/retired-words.json` gives the phrase that replaces each. (2026-10-10)
- **breaking** LANG.CONTRACT.REGISTRY — `CONTRACT` answers a Record keyed `inputs` `outputs` `field` only. `[ DIV ] 0 GET CONTRACT 'partiality' GET` was `'projecting'` and projects `notFound`. A `#:contract` directive takes only `inputs=` `outputs=` `field=`; any other term is `contractViolation`. (2026-10-12)
- **compatible** LANG.CONTRACT.FIELD — Every contract names `field`: `closed`, or `leaving` with the calls that leave. `[ 0 DIV ] CONTRACT 'field' GET` is `'leaving'`. (2026-10-09)
- **breaking** LANG.CONTRACT.REGISTRY — `CONTRACT` of a Core Word answers its registered arity with the fixed half included, so a Word with a fixed operand and a variable one no longer reads as wholly variable. (2026-10-15)
- **breaking** LANG.CONTRACT.CHECK — The pre-execution check refuses a literal block written right before `MAP`, `FILTER`, `FOLD` or `SCAN` that cannot leave exactly one value, as running it would. `[ 1 2 ] [ 1 2 ] MAP` passed `check` and is refused by it. (2026-10-15)
- **breaking** LANG.OBSERVATION.PROTOCOL — The agent envelope is `schemaVersion` 5. An error carries `message` and `error { category, resource, word, line, column }`; the diagnosis fields, the execution receipt and the observation digest are gone. (2026-10-12, 2026-10-13)
- **breaking** LANG.MACHINE.LIMITS — The call-depth ceiling is a resource ceiling like the others. Past it a run fails as `resourceLimitExceeded` with `resource: callDepth`, and every limit profile publishes `callDepth`. `[ 1 COLLECT [ 0 0 ] GET EXEC EXEC ] 1 COLLECT [ 0 0 ] GET EXEC EXEC` was `error:recursionLimitExceeded` and is `error:resourceLimitExceeded` naming `callDepth`. The error category `recursionLimitExceeded` no longer exists. (2026-10-15)
- **breaking** LANG.CONTRACT.REGISTRY — `CONTRACT` names a Word by a String, as `DEF`, `BIND` and `DEL` do, as well as by a Symbol. `'INC' CONTRACT` was `error:notASymbol` and is the Word's contract Record; a String that names no Word projects `notFound`. The projection condition is renamed `nameNamesNoWord`. (2026-10-15)
- **editorial** LANG.DICTIONARY.MUTATION — `DIGEST` of a String is the digest of the text even when the text spells a Word's name, and the specification now says so: `'INC' DIGEST` is the text's digest, and `[ INC ] 0 GET DIGEST` is the Word's identity. No answer changes. (2026-10-15)
- **compatible** LANG.MACHINE.LIMITS — A cost axis may say `valueDependent: "algebraicTerms"`: its charge also grows with the term count of an algebraic operand, which the class, counted in elements, cannot see. Twenty-seven Words carry it, set by measurement. `MUL` is `const` on the numeric axis, and two 16-term operands charge 262,144 numericWork where two rationals charge 1. (2026-10-15)
- **compatible** LANG.OBSERVATION.PROTOCOL — `resourceUsage.algebraicTerms` reports the largest term count any result of the run reached, when one was an irrational: `2 SQRT 3 SQRT ADD` reports 2. (2026-10-15)
- **compatible** LANG.OBSERVATION.PROTOCOL — The agent envelope carries `specificationVersion`, read from `spec/version-lock.json`. (2026-10-15)
- **compatible** LANG.COLLECTIONS.LIFT — Every Word declares the key and leaf domains it reads in `stack.domains`; no answer changes. (2026-10-15)

## 1.0.0-beta.1

The first beta, labelled on 2026-10-08. Changes before it are in git
history.

- **breaking** LANG.AUTHORITY.SOURCES — The specification and the implementation both begin beta at 1.0.0-beta.1.
