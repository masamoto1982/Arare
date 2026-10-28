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
