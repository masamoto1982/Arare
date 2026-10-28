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

