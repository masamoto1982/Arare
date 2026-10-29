# Front end and reference interpreter

The design behind AR-LLR-010 to AR-LLR-017: how source text becomes a
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
Word, which evaluates it on a stack of its own into the vector of what it
leaves; for a quote of literals that is the vector Ajisai writes the same way.

## Lifting and NIL (AR-LLR-014)

Lane-wise Words pair equal-length vectors, reuse a one-element vector or a
scalar, and refuse any other pairing. A NIL operand of an arithmetic or order
Word is the result, unread — the left one when both are NIL, as in Ajisai. AND
and NOT instead read NIL as UNKNOWN, so `FALSE NIL AND` is FALSE and
`TRUE NIL AND` is that NIL, its reason intact.

## The Words (AR-LLR-015)

The twenty Words of WORK_ORDER.md §7 plus the literals TRUE, FALSE and NIL.
Their operand order follows Ajisai's (`vector block MAP`, `vector init block
FOLD`, `then else condition SELECT`), and their arithmetic is the kernel's. The
two SQL departures from Ajisai are SELECT on UNKNOWN taking ELSE and SUM
skipping NIL (ADR-0007). EQ compares whole values, as Ajisai's EQ does; LT is
lane-wise.

## The CLI (AR-LLR-017)

`arare run --json` writes Ajisai's value nodes — exact numerator and
denominator as strings — plus Arare's width and scale. That shared shape is
what lets the oracle harness (ADR-0007) read both languages with one reader.
