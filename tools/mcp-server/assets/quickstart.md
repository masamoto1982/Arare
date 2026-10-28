<!-- MCP-facing preface to the generated writing protocol below.
     Hand-written source; `sync-assets.js` composes it with SKILL.md into
     assets/quickstart.md, and `selftest.js` runs every ```ajisai block here
     against the live backend, so no example can drift from what the server
     actually answers. -->

# Ajisai over MCP — read this first

You are connected to Ajisai: a small, bounded, deterministic engine whose
numbers are **exact rationals closed under square root** — no floats anywhere in
its supported domain. Use it when the answer has to be right and the failure
has to be explainable. It is not a general-purpose language runtime.

This preface is the MCP entry point. Everything after it is the generated
protocol for *writing* Ajisai, and is the reference to consult once you know
which call to make.

## 0. What it does, in one table

Ajisai is more than arithmetic, and a caller who assumes otherwise stops
reaching for it exactly where it would have helped. The 77 Words are:

| you need | Words |
|---|---|
| arithmetic | `ADD` `SUB` `MUL` `DIV` `FLOOR` `ROUND` `MIN` `MAX` `SQRT` `POW` `GCD` `RATIO` (negate with `-1 MUL`) |
| comparison and logic | `EQ` `LT` `GT` (not-equal is `EQ NOT`, at-most is `GT NOT`) · `AND` `NOT` (or is `a NOT b NOT AND NOT`) `SELECT` `TRUE` `FALSE` — SQL's three-valued logic, a NIL read as UNKNOWN · `RECONCILE` (two sources joined: agreement stays, NIL yields, any mismatch, TRUE with FALSE included, is a `conflict` NIL) |
| vectors | arithmetic broadcasts element-wise; no separate vector Words |
| collections | `SORT` `ORDER` `UNIQUE` `ZIP` `RANGE` `FILL` `TAKE` `DROP` `CONCAT` `REVERSE` `LENGTH` `GET` `PUT` `BSEARCH` `COLLECT` · `SHAPE` `RESHAPE` `FLATTEN` `DEPTH` |
| records (keyed data) | `RECORD` `KEYS` `VALUES` `WITHOUT` `HAS?` `MERGE` · read and written by key with `GET` `PUT` · `TALLY` `GROUP` answer Records |
| blocks over a collection | `MAP` `FILTER` `FOLD` `SCAN` |
| text | `CHARS` `JOIN` `TOKENIZE` `TRIM` `UPPER` `LOWER` `SEARCH` `REPLACE` `NUM` `STR` · `FORMAT` (decimal text at a stated precision, the one place rounding happens) |
| JSON in and out | `JSON-DECODE` (object → Record, array → Vector, numbers exact) `JSON-ENCODE` (no rounding: `1/3` travels as `"1/3"`, and so do `1/0` and `0/0`; decoding gives back the String, so apply `NUM`, or write `2 FORMAT NUM` first when the receiver needs a JSON number) |
| absence | `NIL` `NIL?` `NIL-REASON` `ABSENT` (declare a reasoned NIL from your own text) |
| naming, control, output | `DEF` `BIND` `DEL` · `EXEC` `FAIL` (raise a declared ERROR) · `PRINT` |
| reflection | `DIGEST` (content identity of a Word, denotation digest of a value) `CONTRACT` (a Word's or a block's contract as a Record keyed `inputs` `outputs` `field`, inferred without running it) |

**This is the whole list.** Word names are case-insensitive — `add` runs as
`ADD`, because every name is canonicalized to upper case — but otherwise exact.
Do not invent one: `vec-add`, `group-by` and `nil-or` are not Ajisai, and a
name that is not here does not exist under another spelling. When unsure, call
`word_contract` — it answers a near miss with `suggestions` — or read
`ajisai://contracts` for every contract at once.

A few phrases programs write again and again — a remainder, a mean, a
disjunction, text joined with a separator — are User Words in
`ajisai://phrasebook`. Name them in the `words` argument (`words: ["MEAN"]`)
instead of writing their definitions into `source`: the result's `prelude`
says which definitions ran.

Out of domain, and not worth a call: transcendental functions, floating point,
I/O, and anything that is really a program rather than a calculation.

Ajisai also carries no external or real-world reference data — no exchange
rates, no calendars, no reading speeds, no other language's syntax semantics.
Do not invent a plausible-looking number for one of those and run it through
`compute` to dress a guess up as an exact answer; if the question needs a
real-world fact rather than a value already given or derivable from first
principles inside this domain, answer directly without a call, or say you
don't know.

## 1. Choose a tool

| you want | call | pass |
|---|---|---|
| a number, a vector, an exact root, a `PRINT` line | `compute` | `source` |
| to know whether source parses, resolves and keeps its `#:contract` declarations, without running it | `check` | `source` |
| the inferred contract of Words *you* defined | `infer_contracts` | `source` |
| a built-in Word's contract, or "did I spell it right?" | `word_contract` | `word` |

All four take text, never a file path. To run a file, read it yourself and pass
its contents as `source`.

## 2. Read a result in this order

1. **`status`** decides everything else. `ok` — a value. `error` — an *Ajisai*
   error, still an ordinary successful call carrying `message` and `error`.
   `hostError` (with `isError` set) — this server failed, and your program may
   be fine. A `compute` result also names its **`outcome`**: `value`,
   `nil:<reason>` (a reasoned absence, under `status: ok`) or
   `error:<category>`.
2. On `ok`: `stackDisplay` is the final stack bottom→top, `output` holds `PRINT`
   lines, and `stack` is the machine-readable form of the same values. That is
   the general rule and it has exactly one exception: for an irrational square
   root `stackDisplay` is an exact but display-only rendering (`sqrt(2)`), and
   the value to compute with lives in `semantics.exactTerms` — see §4, which you
   must read before computing with any `SQRT` result. A `stackElided` field
   means a value was too large to send: its slot keeps its `type` and gains an
   `elided` record (`elements`, `approxBytes`) in place of the value, the
   values beside it are whole, and the fix is to leave less on the stack.
3. On `error`: `message` says what went wrong, and `error` carries it as
   data — `category` (the stable id to branch on; never match on `message`),
   the `word` it is attributed to, its `line` and `column` in the source, and
   for a crossed ceiling the `resource` it crossed. A `#:contract` declaration
   the body contradicts is an error like any other (`error.category:
   contractViolation`), with every finding listed in `contractDecls.findings`;
   see §8.
4. On `hostError`: branch on `error.code`, and retry only if `error.retryable`
   is true. `mcp.limitProfile` names the profile that applied; its ceilings
   are the `ajisai://limits` resource, and a ceiling that fired names itself
   in `error.resource` and its value in `message`.

A field carrying no value is **absent**, not `null`: a successful result simply
has no `error`. Test for presence.

Do not collapse these into "it worked / it broke". Retrying a division by zero
and rewriting a program that merely timed out are both wasted turns.

## 3. Absence is a value, not an exception

A partial operation that has no answer produces `NIL` carrying a reason, and the
call still succeeds:

```ajisai tool=compute status=ok stack="NIL"
-1 SQRT
```

Division is not such an operation. Every number is a pair over a non-negative
denominator and division is total: a zero divisor answers a point over zero —
`1/0` for any nonzero dividend, whatever its sign, and `0/0` for zero — and the
call carries no absence:

```ajisai tool=compute status=ok stack="1/0 1/0 0/0"
100 0 DIV -5 0 DIV 0 0 DIV
```

The reason is on the value (`semantics.absence.reason`, here `domainMiss`)
and in the result's `outcome` (`nil:domainMiss`). Supply a fallback with
`BIND`, `NIL?` and `SELECT`. `NIL?` consumes its subject, like every Word,
and answers whether it was absent, which is exactly where `SELECT` reads its
truth operand, so name the subject once and read it twice:

```ajisai tool=compute status=ok stack="[ 99/1 ]"
-1 SQRT 'S' BIND [ 99 ] S S NIL? SELECT
```

`NIL?` asks about the whole value, and a vector holding an absent lane is not
itself absent. Lifted over a vector the same root projects lane by lane
(`LANG.COLLECTIONS.LIFT`) — the negative radicand empties its own lane and
leaves the others — so the top is still a vector and the fallback is not
chosen. Recover such a result per lane (`MAP`), not around it:

```ajisai tool=compute status=ok stack="[ 2/1 NIL ]"
[ 4 -1 ] SQRT
```

`0/0` is the one number with no place in the order, so an order asked of it is
where arithmetic and absence meet: `0 0 DIV 1 LT` projects `NIL` with reason
`domainMiss`, which reads as *unknown* in truth position, while `0/0 0/0 EQ`
is `TRUE`.

## 4. Exact arithmetic: what to read, and what not to

Rationals are exact and their display is exact too:

```ajisai tool=compute status=ok stack="1/1"
2 3 DIV 1 3 DIV ADD
```

An irrational square root is where display and value part company. On the
result of

```ajisai tool=compute status=ok
2 SQRT
```

the two fields to read are:

- **`stackDisplay`** — the value written as one token: `"sqrt(2)"`,
  `"1/2*sqrt(2)"`, `"1/1+sqrt(2)"`. It is exact and never truncated. It is a
  display: read it, do not parse it.
- **`semantics.exactTerms`** — the value itself: a list of
  `{ numerator, denominator, radicand }` terms meaning `Σ (n/d)·√radicand`,
  arbitrary-precision integers as strings. Compute with this.

The display renders exactly these terms. One *other* field on that same result
is **not** the value, and reading it as if it were will mislead you:

- `value.numerator / value.denominator` is a rational *approximation*, marked
  `semantics.approximate: true`. It is a convenience, not the number.

`exactTerms` does not appear on a plain rational or a vector of rationals —
there is no radical to write, and `stackDisplay` is already the whole value.

The display writes the canonical normal form, so two values that *are* equal
are written the same way — `8 SQRT` and `2 SQRT 2 SQRT ADD` both give
`2/1*sqrt(2)`. Even so, never compare these strings to decide equality: the
string is display text, not a value. Ask Ajisai, which decides on the exact
value:

```ajisai tool=compute status=ok stack="TRUE"
8 SQRT 2 SQRT 2 SQRT ADD EQ
```

## 5. When a name is wrong, the answer says so

```ajisai tool=compute status=error
[ 1 2 3 ] LENGHT
```

That returns `status: "error"` with `error.category: unknownWord` and
`error.word: LENGHT`, at line 1, column 11. Look the name up rather than
guessing: `word_contract` answers a misspelling with `suggestions` (`LENGTH`
here), and for a Word it knows gives its arity, purity and NIL policy.

## 6. Bounds

Every result names the profile it ran under in `mcp.limitProfile`, alongside
`mcp.serverVersion`, `mcp.engineVersion` and `mcp.backend.kind`; the ceilings
themselves are read once, at `ajisai://limits`, rather than repeated on every
result. Exceeding a ceiling is an error, never a hang: `error.resource` names
the ceiling, and `message` its value and what crossed it. The
result contract is at `ajisai://schema/result`, every Word's full contract at
`ajisai://contracts`, and the inventory — every Word's name and family — at
`ajisai://vocabulary`.

Two of the ceilings decide how a program should be shaped, and they are 10x
to 120x tighter than the browser playground's, so a program tried there does
not carry over unchanged:

- **`executionSteps` is 100,000, and a block iteration is one step per
  element.** `MAP` / `FILTER` / `FOLD` / `SCAN` walk tens of thousands of
  elements, not more: `0 99999 RANGE 0 [ ADD ] FOLD` is refused. Write the
  same operation on whole vectors instead — `V V ADD` over 100,000 lanes is
  a few steps, and so is `0 99999 RANGE` itself (the materialization ceiling,
  `materializedElements`, is also 100,000).
- **A result is sent in full up to 440 KiB of stack.** Past that, the slot
  that does not fit is elided (§2) and the rest arrives whole. Leave the
  answer on the stack, not the intermediates it was built from.

## 7. Budget before you run

Every Word publishes what it charges, so you can bound a program without
executing it. `word_contract` (and `ajisai://contracts`, for all of them at
once) carries a `cost` object with one entry per metered resource:

```json
"cost": {
  "steps":      { "class": "const",  "exact": true },
  "numeric":    { "class": "linear", "exact": true },
  "collection": { "class": "const",  "exact": true }
}
```

`class` is a sound upper bound on how the charge grows with the Word's input —
`const` < `linear` < `superlinear` < `unbounded`. The classes **join
pointwise**: a phrase's bound on each axis is the widest class any of its Words
declares on that axis, so you can compute a phrase's bound by reading its Words
rather than by running it. `exact: true` means some contribution provably
attains the class; `exact: false` marks a sound over-approximation the real run
may beat.

The three axes are the same counters a result reports back in
`resourceUsage` (`executionSteps`, `numericWork`, `collectionWork`), so a bound
and a measurement are answers about one quantity.

**A class is how the charge grows, not how large it is.** Two programs of the
same class can differ by orders of magnitude, because the class says nothing
about what one operation costs — and an operation on an algebraic value
(anything `SQRT` produced) rebuilds a multiquadratic normal form each time.
Measured through `resourceUsage.numericWork`, all three of these are `const`:

| program | `numericWork` |
| --- | --- |
| `[ 1 2 3 4 5 ] [ 0 ] [ ADD ] FOLD` | 5 |
| `2 SQRT 3 SQRT ADD` | 2048 |
| `2 SQRT 3 SQRT ADD 'S' BIND S S MUL` | 6144 |

The `numericWork` ceiling is 10,000,000, so an algebraic chain meets it after a
few thousand additions while a rational one of the same class runs
indefinitely. Budget an algebraic value at 10^2–10^3 times a rational one, and
when the size matters, measure it: run the small case and read
`resourceUsage`, rather than inferring a size from a class that does not carry
one.

Two further rules:

- An `unbounded` axis means the charge is not a function of input *size* —
  `MAP`/`FILTER`/`FOLD` run a block you supply, and `RANGE`/`FILL` are sized by
  an operand's *value*. Pin those with literal operands, or expect to meet a
  ceiling.
- A Word you define yourself is bounded by the Words its body calls: read
  their costs and join them.

## 8. Declare a contract, and have it checked before anything runs

A Word you define can state its own contract on a `#:contract` comment line,
in the keys `CONTRACT` answers in: `inputs=N` and `outputs=N` (must match the
body) and `field=closed|leaving`. A key left out is not checked; any other
term makes the directive malformed, which is a `contractViolation` error.
`field=closed` promises the body never answers a point over zero (`1/0` or
`0/0`) from operands holding none: only `DIV`, `POW`, `NUM` and a
literal over zero leave the field, and a `DIV` by a non-zero literal (`2 DIV`)
or a `POW` to a non-negative literal (`2 POW`) does not. Inside `closed` code
the field laws — distributivity, `x x SUB` is `0` — hold over finite operands,
and `infer_contracts` names a `leaving` body's exits under `fieldExits`
(LANG.CONTRACT.FIELD). `check` verifies the line against the body without running anything, and
`compute` runs the same check first:

```ajisai tool=check status=ok
#:contract DOUBLE inputs=1 outputs=1 field=closed
[ 2 MUL ] 'DOUBLE' DEF 21 DOUBLE
```

A declaration the body contradicts is refused by both, as an ordinary error:
`status: error`, `error.category: contractViolation`, `outcome:
error:contractViolation` (from `compute`), `message` quoting the finding, and
`contractDecls.findings` listing every one. Nothing runs and nothing prints:

```ajisai tool=compute status=error
#:contract DOUBLE inputs=2 outputs=1
[ 2 MUL ] 'DOUBLE' DEF 5 DOUBLE PRINT
```

A body the inference cannot read (a block taken out of data and passed to
`EXEC`) is *cannot verify* — a `note` finding, and that declaration's
`outcome` is `nil` — never a false violation. To write a declaration without guessing, call `infer_contracts`
first and paste the `suggested` line it answers for the Word.

---

The rest of this document is the generated writing protocol: syntax, the full
Word table, and worked examples verified against the real interpreter.

<!-- BEGIN GENERATED SKILL.md -->

<!-- GENERATED FILE — do not edit by hand.
     Regenerate: npm run generate:skill   (verified against the ajisai CLI)
     Source of truth for semantics: SPECIFICATION.html.
     Generator: scripts/generate-skill-md.mjs -->

# Ajisai — Agent Writing Protocol (SKILL.md)

How to *write working Ajisai on the first try*. Every code line below was
executed by the generator against the real interpreter; results shown are
actual outputs. **If a word is not in the §9 index, it does not exist — when
unsure, grep §9 before writing.** Each Word's full summary is in
`skill/words.md` (over MCP: `word_contract` or `ajisai://contracts`).

## 1. Run loop

```sh
ajisai agent compute program.ajisai   # exit 0 = ok, 1 = language error, 2 = usage
ajisai agent check program.ajisai     # resolve names, verify contracts and blocks; no execution
```

Read the JSON in this order (contract: docs/dev/agent-cli-output-contract.md):
1. `status` / exit code. On ok: `stackDisplay` (final stack, bottom→top) and `output` (PRINT lines).
2. On error: `message`, and `error` — `category` (the spec/outcomes.json error category), `resource` for a ceiling, `word` (the Word it is attributed to) and `line`/`column` locate the failure.
3. A NIL on the stack carries its reason in `semantics.absence.reason`.

## 2. Minimal syntax

- Postfix, stack-based. Operands first, word last: `1 2 ADD`.
- Numbers are **exact rationals** (`1/3`, `3.14` → 157/50). No floats. Display shows `3/1` for 3.
- A scalar is written bare: `42`, `1 3 DIV`. Data lives in vectors: `[ 1 2 3 ]`, and vectors nest for ragged and grouped data. `[ 42 ]` is a one-element *vector*, not another way to write 42 — arithmetic broadcasts it like a scalar, but a Word that takes an *element* (`PUT`, `GET`, `RECONCILE`) stores or reads the vector itself, and nothing errors (§7). Write scalars bare and the question never arises.
- Strings: `'single quotes'` (a value domain of its own, not a vector of codepoints). Booleans: `TRUE` / `FALSE`, with a NIL read as UNKNOWN — SQL's three-valued logic. Absence: `NIL`.
- Code blocks are quoted programs passed to MAP / FILTER / FOLD / DEF, written as an ordinary Vector (§6) — there is no separate block bracket. SELECT is not among them: it takes values, not code.
- Named data is a Record, built by `RECORD` from a Vector of keys and a Vector of values: `[ 'x' 'y' ] [ 1 2 ] RECORD`. It is not a Vector and is never code. It displays as `[ 'x' 'y' ] [ 1/1 2/1 ] RECORD` — the keys, the values and the Word that joins them, which is also valid source that rebuilds it.
- Define a user word with a body Vector, then a `'NAME'` string, then `DEF`, then call `NAME`: `[ 1 2 ADD ] 'MY-SUM' DEF MY-SUM` (§6). Words are case-insensitive (canonicalized to upper case).
- Declare a user word's contract on a `#:contract` comment line — `#:contract DOUBLE inputs=1 outputs=1 field=closed` — and `check` verifies it against the body before anything runs; `compute` runs the same check first and refuses a program whose declaration is violated (§2a).
- Comments: `#` to end of line.
- Every Word consumes the operands it reads. To use a value more than once, name it with `BIND` and read the name: `5 'N' BIND N N 1 ADD` leaves `5/1  6/1`.
- One word does one thing to the stack; there are **no** DUP/SWAP-style shufflers (§8).

## 2a. Declaring a contract (`#:contract`)

One comment line per Word, written in the keys and values of a contract Record
(the same vocabulary `CONTRACT` answers in):

`#:contract NAME [inputs=N] [outputs=N] [field=closed|leaving]`

`field=closed` promises the Word never answers a point over zero (`1/0`
`0/0`) from operands holding none — only `DIV`, `POW`, `NUM` and a literal over
zero leave the field, and a `DIV` by a non-zero literal (`2 DIV`) or a `POW`
to a non-negative literal (`2 POW`) does not; a divisor the body computes is
not read. Inside `closed` code the field laws (distributivity, `x x SUB` = 0)
hold; a `leaving` report's `fieldExits` names where the body leaves.

`inputs`/`outputs` must equal what the body does; `field=closed` is a bound the
body must not exceed; a key left out is not checked. The check is conservative:
a body the inference cannot read is reported as *cannot verify* (a `note`),
never as a false violation. To write a
declaration without guessing, run `infer_contracts` first and paste its
`suggested` line.

- Declare what your Word does; check verifies it before anything runs
  ```ajisai
  #:contract DOUBLE inputs=1 outputs=1 field=closed
  [ 2 MUL ] 'DOUBLE' DEF 21 DOUBLE
  ```
  `check` → exit 0, `contractDecls: { outcome: "value" }`; `compute` → stack: `42/1`, with the same `contractDecls`.

## 3. Control and iteration

- Branch: the two candidates, then the truth that chooses between them, then `SELECT`: `'non-negative' 'negative' 4 0 LT NOT SELECT PRINT` (§6). Both candidates are values the program already built, so neither is skipped and nothing is evaluated by SELECT itself. The choice is made lane by lane, so a Vector of truths branches a whole Vector at once: `0 [ -3 5 -1 ] [ -3 5 -1 ] 0 LT SELECT`. An absent truth chooses neither and answers that same absence. For the same reason SELECT cannot choose between two whole Vectors: candidates of different lengths are `shapeMismatch`, and a one-element candidate is silently repeated (`[ 1 2 3 ] [ 9 ] FALSE SELECT` is `[ 9 9 9 ]`). To pick one whole value by one truth, collect both and read one: `[ 1 2 3 ] [ 9 ] 2 COLLECT 0 1 FALSE SELECT GET` is `[ 9 ]`.
- Iterate data, not counters: `MAP` / `FILTER` / `FOLD` with block operands (examples in §6). `FOLD` requires an explicit initial value: `[ 1 2 3 ] 0 [ ADD ] FOLD`. The block leaves **exactly one** value — the mapped element, the truth, the next accumulator; leaving none or a surplus is a `blockContractViolation` (§7).
- Budget: a block iteration costs one step per element for every Word the block runs (literals are free), so under the MCP profile's 100,000-step budget a one-Word block (`[ ADD ] FOLD`) walks 100,000 elements, a two-Word block (`[ 3 MUL 1 ADD ] MAP`) 50,000, a three-Word block 33,000 — tens of thousands of elements divided by the Words in the block, not more. Beyond that, write the operation on whole vectors — `V 3 MUL 1 ADD` over 100,000 lanes is two steps — and leave no large intermediate on the stack.
- Irrationals cost more than their class says: an axis marked `valueDependent: "algebraicTerms"` in `word_contract` (MUL, ADD, EQ, SORT and others) also grows with the number of terms in an algebraic value, and a product of distinct square roots doubles its terms with each factor. Run a small case first and read `resourceUsage.algebraicTerms`, the largest term count a result reached.
- No recursion: `DEF` refuses a word whose body names itself, directly or through other user words (a diagnosed error at definition time, not at the call). Repetition is expressed only through MAP / FILTER / FOLD / SCAN over an already-finite vector.
- No block runs itself: a block EXEC / MAP / FILTER / FOLD / SCAN runs is *written* (a `[ ]` literal of the code running it, or one nested inside) or *built from data* (taken apart and rebuilt, or bound in another frame). Only one block built from data may run at a time; a second is `nestedDataBlock`. So code that runs a block it is given cannot be given one that does the same — write such code as Words with `DEF`, which nest freely.
- JSON boundary: `JSON-ENCODE` writes a rational with no finite decimal, and `1/0` and `0/0`, as its lexeme in a string (`"1/3"`), so `JSON-DECODE` gives back a String and `NUM` recovers the number. For a receiver that needs a JSON or SQL number, choose the digits first: `1 3 DIV 2 FORMAT NUM JSON-ENCODE` is `'0.33'`.
- Naming a Word: `DEF`, `BIND`, `DEL` and `CONTRACT` take its name as a String (`'INC' CONTRACT`); `CONTRACT` also takes a Symbol, written `[ INC ] 0 GET`. A bare `INC` *calls* the Word, so `INC CONTRACT` runs INC first. `DIGEST` is the one exception: a String is text there, so `'INC' DIGEST` digests the text and `[ INC ] 0 GET DIGEST` is the Word's content identity.

## 4. NIL — absence is a value, not an exception

Failed partial operations *project to NIL*: `-1 SQRT` succeeds (exit 0) and
pushes `NIL` (reason: `domainMiss`). The NIL value itself carries
`semantics.absence.reason` on the stack.

Division by zero is **not** a projection: every number is a pair over a
non-negative denominator, taken up to a nonzero factor (the wheel of fractions),
and a quotient by zero is `1/0` for any nonzero dividend, whatever its sign,
and `0/0` for zero — `100 0 DIV -5 0 DIV 0 0 DIV` → stack `1/0 1/0 0/0`.
The two points `1/0` and `0/0` are numbers: they add, multiply, divide and
compare by `EQ`. `1/0` is unsigned and `0/0` absorbs everything it meets;
neither has a place in the order, so `LT` / `GT` / `MIN` / `MAX` / `SORT`
project `domainMiss` on them.

- Provide a fallback with `BIND`, `NIL?` and `SELECT`: `-1 SQRT 'S' BIND 99 S S NIL? SELECT` → stack `99/1`. `NIL?` consumes its subject like every Word and answers whether it was absent, which is exactly where `SELECT` wants the truth — so name the subject once and read it twice: the phrase reads "S, or the fallback if S is absent".
- Over a vector the projection is **per lane, not per value**: `[ 4 -1 ] SQRT` → stack `[ 2/1 NIL ]`. The lane that had no root is the only one emptied.
- That makes the top a vector, not a NIL, so `NIL?` — which asks about the whole value — answers FALSE and the fallback is not chosen. Recover a lifted result inside the vector, not around it.
- NIL flows through later operations (NIL projection rule); check for it where it matters instead of letting it propagate to the end.

## 5. Exactness — comparison decides over the algebraic field

Numbers are exact rationals, closed under `SQRT`. Arithmetic never rounds,
coefficients are arbitrary-precision, and **every comparison of two scalars
built from rationals and `SQRT` decides**: there is no budget, no refinement
limit, and no undecided outcome.

```ajisai
8 SQRT 2 SQRT 2 SQRT ADD EQ   # √8 vs √2+√2
```

→ stack `TRUE` (exit 0). Values built through different
histories are the same value when they denote the same real.

That field is the whole numeric domain. `POW` answers inside it — an integer
exponent, or `p/2` over a non-negative rational — and projects NIL for any
other exponent rather than leave it:

```ajisai
8 1/3 POW NIL-REASON
```

→ stack `'domainMiss'` (exit 0). Truth has three values: `TRUE`,
`FALSE`, and the logical UNKNOWN, which is what a NIL operand reads as in a
truth position (§4) — no comparison produces it of its own. An operation that
cannot produce a value produces NIL (§4); a malformed one raises an error.

## 6. Canonical examples (all verified by the generator)

- Push a number: a bare scalar
  `42` → stack: `42/1`
- Exact rational division — no floats, ever
  `1 3 DIV` → stack: `1/3`
- Elementwise vector arithmetic
  `[ 1 2 3 ] [ 4 5 6 ] ADD` → stack: `[ 5/1 7/1 9/1 ]`
- Scalar broadcast over a vector
  `5 [ 1 2 3 ] MUL` → stack: `[ 5/1 10/1 15/1 ]`
- Remainder: name the operands, then a - b * floor(a/b)
  `10 'A' BIND 3 'B' BIND A A B DIV FLOOR B MUL SUB` → stack: `1/1`
- Comparison pushes a boolean
  `1 2 LT` → stack: `TRUE`
- Comparison lifts over vectors element-wise
  `[ 1 2 ] [ 3 1 ] LT` → stack: `[ TRUE FALSE ]`
- Range: start end, both included
  `0 5 RANGE` → stack: `[ 0/1 1/1 2/1 3/1 4/1 5/1 ]`
- A stride is a multiplication of a range
  `0 5 RANGE 2 MUL` → stack: `[ 0/1 2/1 4/1 6/1 8/1 10/1 ]`
- Fill a shape with one number: [ shape ] value
  `[ 2 2 ] 7 FILL` → stack: `[ [ 7/1 7/1 ] [ 7/1 7/1 ] ]`
- MAP with a [ ] code block
  `0 4 RANGE [ 2 MUL ] MAP` → stack: `[ 0/1 2/1 4/1 6/1 8/1 ]`
- FILTER keeps matching elements
  `0 10 RANGE [ 5 GT ] FILTER` → stack: `[ 6/1 7/1 8/1 9/1 10/1 ]`
- FOLD needs an explicit initial value
  `[ 1 2 3 ] 0 [ ADD ] FOLD` → stack: `6/1`
- A Record from a Vector of keys and a Vector of values
  `[ 'x' 'y' ] [ 1 2 ] RECORD` → stack: `[ 'x' 'y' ] [ 1/1 2/1 ] RECORD`
- Define a user word: [ body ] then name, then DEF
  `[ 1 2 ADD ] 'MY-SUM' DEF MY-SUM` → stack: `3/1`
- SELECT: the two candidates, then the truth that chooses between them
  `'non-negative' 'negative' 4 0 LT NOT SELECT PRINT` → prints `non-negative`
- SELECT chooses lane by lane, so a whole vector branches at once
  `0 [ -3 5 -1 ] [ -3 5 -1 ] 0 LT SELECT` → stack: `[ 0/1 5/1 0/1 ]`
- To choose one whole value by one truth, collect both and GET
  `[ 1 2 3 ] [ 9 ] 2 COLLECT 0 1 FALSE SELECT GET` → stack: `[ 9/1 ]`
- Strings are bare '...' literals; CHARS/JOIN convert
  `'hello' CHARS REVERSE JOIN` → stack: `'olleh'`
- Cast a string to an exact number
  `'42' NUM` → stack: `42/1`
- PRINT pops and emits to output (not the stack)
  `[ 1 2 3 ] PRINT` → prints `[ 1/1 2/1 3/1 ]`
- Sorting is a plain Core word
  `[ 3 1 2 ] SORT` → stack: `[ 1/1 2/1 3/1 ]`
- Exact square root takes a bare scalar
  `2 SQRT` → stack: `sqrt(2)`
- A value used twice is named with BIND
  `5 'N' BIND N N 1 ADD` → stack: `5/1  6/1`

## 7. Common errors — actual CLI output, and the fix

- **Typo / unknown word** — `[ 1 ] ADDD`
  → exit 1, `message: "Unknown word: ADDD — did you mean ADD or AND?"`, `error: {"category":"unknownWord","column":7,"line":1,"word":"ADDD"}`.
  Fix: The message names the Core Words within a typo of what you wrote (here: `ADD`); otherwise grep §9. Word names are upper-cased automatically.
- **A Word from another language that Ajisai writes as a phrase** — `10 3 MOD`
  → exit 1, `` message: "Unknown word: MOD — written as a phrase, for example `10 'X' BIND 3 'Y' BIND X X Y DIV FLOOR Y MUL SUB`" ``, `error: {"category":"unknownWord","column":6,"line":1,"word":"MOD"}`.
  Fix: The message carries one runnable line that does the same thing with Core Words — MOD, OR, ABS, DUP, SWAP, IF, SUM and the like each have one. Adapt it to your operands; DEF it as a User Word if you need it more than once.
- **Stack underflow: operands must be pushed first** — `ADD`
  → exit 1, `` message: "ADD: stack underflow — for example `1 2 ADD`" ``, `error: {"category":"stackUnderflow","column":1,"line":1,"word":"ADD"}`.
  Fix: Push both operands before the operator: `1 2 ADD`. Ajisai is postfix; there is no infix form.
- **FOLD without an initial value** — `[ 1 2 3 ] [ ADD ] FOLD`
  → exit 1, `` message: "FOLD: stack underflow — for example `[ 1 2 3 ] 0 [ ADD ] FOLD`" ``, `error: {"category":"stackUnderflow","column":19,"line":1,"word":"FOLD"}`.
  Fix: FOLD is `vector init [ op ] FOLD`: `[ 1 2 3 ] 0 [ ADD ] FOLD`.
- **A block that leaves more than its one result** — `[ 1 2 3 ] [ 2 MUL 7 ] MAP`
  → exit 1, `message: "MAP: expected the block to leave one value, and it left 2"`, `error: {"category":"blockContractViolation","column":23,"line":1,"word":"MAP"}`.
  Fix: A MAP / FILTER / FOLD / SCAN block leaves exactly one value — its result. A surplus is not discarded, it is this error. Consume what you computed before the block ends, or name it with `BIND`, which leaves nothing: `[ 2 MUL ]`, or `[ 'X' BIND X X MUL ]`. `check` reports this before anything runs when the block is written right before the Word.
- **A `#:contract` declaration the body contradicts** — `#:contract DOUBLE inputs=2 outputs=1
[ 2 MUL ] 'DOUBLE' DEF 5 DOUBLE`
  → exit 1, `` message: "Contract declaration violated: `#:contract DOUBLE`: declared `inputs=2` but inferred `inputs=1`." ``, `error: {"category":"contractViolation","word":"DOUBLE"}`.
  Fix: The declaration is checked before anything runs, and a violated one stops the run (`contractDecls.findings` lists every finding). Fix the body or the line; `ajisai agent infer-contracts` answers a paste-ready `suggested` line for the Word as written.
- **SELECT takes three operands: both candidates, then the truth** — `[ 'big' ] [ 5 ] [ 3 ] GT SELECT`
  → exit 1, `` message: "SELECT: stack underflow — for example `[ 'yes' ] [ 'no' ] TRUE SELECT`" ``, `error: {"category":"stackUnderflow","column":26,"line":1,"word":"SELECT"}`.
  Fix: SELECT is `whenTrue whenFalse truth SELECT` — push both candidates before the test that chooses between them: `[ 'big' ] [ 'small' ] [ 5 ] [ 3 ] GT SELECT`. It chooses between values, never running either one, so an effect goes after it: `... SELECT PRINT`.
- **SELECT needs a truth value, not a number** — `[ 'y' ] [ 'n' ] 1 SELECT`
  → exit 1, `` message: "SELECT: expected a truth value, got Scalar — for example `[ 'yes' ] [ 'no' ] TRUE SELECT`" ``, `error: {"category":"nonTruthValue","column":19,"line":1,"word":"SELECT"}`.
  Fix: The third operand must be TRUE, FALSE or an absence — a scalar is not a truth value (§4). Write the test: `1 0 EQ NOT`.
- **Broadcast shape mismatch** — `[ 1 2 ] [ 1 2 3 ] ADD`
  → exit 1, `message: "ADD: expected shapes that align, got [2] and [3] (axis 0 is 2 and 3, and neither is 1)"`, `error: {"category":"shapeMismatch","column":19,"line":1,"word":"ADD"}`.
  Fix: Elementwise ops need equal or broadcastable shapes (a scalar `5`, or a one-element vector, broadcasts; `[2]` vs `[3]` does not).
- **NUM casts strings, not booleans** — `TRUE NUM`
  → exit 1, `` message: "NUM: expected a String, got Boolean — for example `'42' NUM`" ``, `error: {"category":"nonText","column":6,"line":1,"word":"NUM"}`.
  Fix: NUM accepts strings: `'42' NUM`. There is no boolean→number cast.
- **Old one-vector RANGE form** — `[ 0 5 ] RANGE`
  → exit 1, `` message: "RANGE: stack underflow — for example `0 5 RANGE`" ``, `error: {"category":"stackUnderflow","column":9,"line":1,"word":"RANGE"}`.
  Fix: RANGE takes two bounds: `0 5 RANGE`. A stride is a multiplication: `0 5 RANGE 2 MUL`.

These raise. The next one does not — it succeeds and answers something other
than it looks like it answers, which is the harder kind to notice:

- **A one-element vector where a Word wants an element** — both of these succeed (exit 0):
  `[ 1 2 3 ] [ 1 ] [ 9 ] PUT` → stack `[ [ 1/1 [ 9/1 ] 3/1 ] ]`
  `[ 1 2 3 ] 1 9 PUT` → stack `[ 1/1 9/1 3/1 ]`
  Fix: PUT and GET take an *element*, not a one-element vector holding it: `[ 9 ]` is that vector, so it is stored as one. Write a scalar bare (§2) and this cannot happen; a one-element vector is a vector, everywhere, and no error says so.

## 8. Forbidden patterns (each verified to fail)

- **DUP / SWAP / DROP / OVER / ROT** (`DUP` fails) — Forth-style stack shufflers do not exist. Every Word consumes the operands it reads; name a value with `BIND` to use it more than once.
- **IF / ELSE / THEN / WHILE** (`[ 1 ] IF` fails) — No structured keywords, and no loops. Branch with SELECT over two values; iterate with MAP / FILTER / FOLD / SCAN.
- **A word calling itself** (`[ REC ] 'REC' DEF` fails) — The User dictionary is acyclic: `DEF` refuses a body that names the word being defined, directly or through other user words, so this fails at definition time rather than the call. Repetition is expressed only through MAP / FILTER / FOLD / SCAN over an already-finite vector.
- **Parentheses ( )** (`( 1 2 )` fails) — Reserved; not valid in source. `[ ]` is the sole bracket, for vectors and code alike.
- **Double-quoted strings** (`"hello" PRINT` fails) — Strings use single quotes: 'hello'.
- **// line comments** (`// comment` fails) — Comments start with `#`.

## 9. Word index

Generated from `docs/word-manifest.json` — the complete inventory:
77 canonical Words in one flat Core dictionary, of which
49 form the Semantic Kernel and 28 are Standard Words. Both are
ordinary Core Words called by their plain names; the split is a design
classification, not a namespace. A word absent here does not exist. There is
no module system and nothing to import. For a phrase you would otherwise
write out again and again — a remainder, a disjunction, a sum or mean, text
joined with a separator — `docs/phrasebook/phrasebook.ajisai` holds User Word
definitions whose contracts and examples CI verifies; copy the definitions
you need to the top of your program (over MCP, name them in `words`).

Every Word, by family. `skill/words.md` has each one's full summary — operand
order, NIL and error behaviour, and an example — and §6 and §7 above show the
common ones at work.

| family | Words |
|---|---|
| truthOrder | `TRUE` `FALSE` `AND` `NOT` `SELECT` `EQ` `LT` `GT` `MIN` `MAX` |
| knowledgeOrder | `RECONCILE` `ADD` `SUB` `MUL` `DIV` `FLOOR` `ROUND` `SQRT` `POW` `GCD` `RATIO` |
| collection | `GET` `LENGTH` `TAKE` `DROP` `CONCAT` `REVERSE` `COLLECT` `RANGE` `FILL` `SHAPE` `RESHAPE` `FLATTEN` `DEPTH` `SORT` `ORDER` `UNIQUE` `ZIP` `PUT` `BSEARCH` |
| record | `TALLY` `GROUP` `RECORD` `KEYS` `VALUES` `WITHOUT` `HAS?` `MERGE` |
| higherOrder | `MAP` `FILTER` `FOLD` `SCAN` |
| text | `CHARS` `JOIN` `TRIM` `UPPER` `LOWER` `TOKENIZE` `SEARCH` `REPLACE` `NUM` `STR` `FORMAT` `JSON-DECODE` `JSON-ENCODE` |
| control | `EXEC` `FAIL` |
| dictionary | `CONTRACT` `BIND` `DEF` `DEL` `DIGEST` |
| absence | `NIL` `NIL?` `NIL-REASON` `ABSENT` |
| output | `PRINT` |
