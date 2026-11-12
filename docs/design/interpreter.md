# Front end and reference interpreter

The design behind AR-LLR-010 to AR-LLR-017 and AR-LLR-028: how source text becomes a
`Program` (`rust/crates/arare-front`), how the reference interpreter runs it
(`rust/crates/arare-interp`), and how `arare run` shows the result
(`rust/crates/arare-cli`).

## Literals (AR-HLR-018, AR-LLR-011, AR-LLR-012)

An integer literal is exact: scale 0, the narrowest width that holds it. Any
other numeric literal — a decimal such as `0.1`, a fraction such as `1/3`, a
wheel point such as `1/0` — has no fixed-point value of its own, so it must be
followed, where it is written, by a width, an exponent and QUANTIZE:

```text
0.1 16 -12 QUANTIZE      # 0.1 at int16, exponent −12: code round(409.6) = 410
```

The parser folds the four tokens into one number before anything runs
(`Num::from_ratio`, AR-LLR-010), so the program cannot run with an unquantized
value in it, and refuses a non-integer literal anywhere else. The same text is
an Ajisai program too: with the oracle prelude QUANTIZE drops its two operands,
and Ajisai computes on the exact 1/10.

## Values and the machine (AR-LLR-013, AR-LLR-016)

A value is a number (`arare_kernel::Num`), TRUE or FALSE, a NIL with its
reason, a vector, or a quote not yet consumed. UNKNOWN is not a value of its
own: it is a NIL where a truth value is expected (AR-HLR-013). The machine runs
one top-level item per step, which is what the Playground's step-through will
show (Phase 5). A quote is code to MAP, FILTER and FOLD and data to every other
Word, which reads it as the vector of the literals it writes (numbers, NIL,
TRUE, FALSE, names as text, nested quotes), the vector Ajisai writes the same
way. Any other Word or a name in a quote read as data is an error, not run:
Ajisai keeps it a name, so running it would give the bracket a second meaning.

## Lifting and NIL (AR-LLR-014)

Lane-wise Words lift as Ajisai's LANG.COLLECTIONS.LIFT does. A scalar pairs
with every lane of a vector however it nests. Two or more vectors must each
have a shape and align from the innermost axis, as NumPy broadcasts: a lane of
length 1 and a vector of lower rank are reused, so `[ [ 1 2 ] [ 3 4 ] ] [ 10 20 ]
ADD` is `[ [ 11 22 ] [ 13 24 ] ]`; any other pairing, and a ragged vector
paired with another vector, is refused. SELECT lifts its condition and both
branches alike, so `[ [ 1 2 ] [ 3 4 ] ] [ 8 9 ] FALSE SELECT` is
`[ [ 8 9 ] [ 8 9 ] ]`. A NIL operand of an arithmetic or order
Word is the result, unread — the left one when both are NIL, as in Ajisai. AND,
NOT and SELECT instead read a NIL as a lane, UNKNOWN to AND and NOT, so `FALSE NIL AND` is FALSE and
`TRUE NIL AND` is that NIL, its reason intact.

## The Words (AR-LLR-015)

The twenty Words of WORK_ORDER.md §7 plus the literals TRUE, FALSE and NIL.
Their operand order follows Ajisai's (`vector block MAP`, `vector init block
FOLD`, `then else condition SELECT`), and their arithmetic is the kernel's.
SELECT on UNKNOWN takes ELSE and SUM skips NIL, as SQL's do and as Ajisai's do
since 1.0.0-beta.4 (ADR-0007, Amendment). EQ compares whole values, as Ajisai's EQ does; LT is
lane-wise.

## Names (AR-HLR-032, AR-LLR-028)

BIND is Ajisai's, so that a value read more than once — the x of the logistic
map, read twice a step — can be named instead of recomputed (ADR-0010):

```text
5 'N' BIND N N ADD                     # 10
[ 1 2 ] [ 'A' 'B' ] BIND B A SUB       # 1
[ 1 2 3 ] 0 [ 'I' BIND 'X' BIND X I ADD ] FOLD   # 6
```

`'X'` is a name as text, a value only BIND reads; a bare `X` pushes the value
bound to it. The front end accepts a bare name only where the source also
writes it as text, so a misspelled Word is still refused before anything runs;
whether the name is bound where it runs is the interpreter's question. The
machine keeps the names in scopes: the program's, and one for every run of a
block, which ends with it — a block reads the names of the code around it,
and its own bindings do not outlive it. Binding a name again replaces it.

Arare has no User Words, so the one scope that is not a block's is the
program's, and a name is looked up when the item that names it runs. A quote
that leaves its block — a block's result that is itself a quote, read as data
after the block — reads the scope where it is read, not the one it was
written in, where Ajisai reads the latter (PR-0013).

## The CLI (AR-LLR-017)

`arare run --json` writes Ajisai's value nodes — exact numerator and
denominator as strings — plus Arare's width and scale; a name as text is
Ajisai's string node. That shared shape is
what lets the oracle harness (ADR-0007) read both languages with one reader.
