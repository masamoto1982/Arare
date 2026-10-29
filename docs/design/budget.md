# The error-budget checker

The design behind AR-LLR-018 to AR-LLR-022: how `rust/crates/arare-budget`
infers, from a program's text, how far each of its numbers may be from the
exact value Ajisai computes (AR-HLR-027), decides the `field` contract
(AR-HLR-028), and carries both through truth values, SELECT and vectors
(AR-HLR-029); how `arare check` shows it (`rust/crates/arare-cli`). The
decision and its alternatives are ADR-0008.

## Two meanings

Every claim the checker makes relates two meanings of the same program:

- **Arare's**: what the reference interpreter computes, on fixed-point codes,
  rounding at quantized literals, DIV, RESCALE and QUANTIZE, and fitting where
  results may overflow.
- **The exact one**: the same program on exact rationals, with QUANTIZE,
  RESCALE and NARROW keeping the exact value. It is what Ajisai computes on
  the same source with the oracle prelude (ADR-0007), except where Arare
  departs from Ajisai on purpose — a SELECT on UNKNOWN and an aggregate over
  NIL follow SQL in both meanings (AR-HLR-015, AR-HLR-016).

The budget of a number is a bound on |Arare − exact|. It is never computed by
running the exact meaning: the checker reads literals and Words only.

## One number (AR-LLR-018, AR-LLR-019)

What the checker knows of a number is Lean's `Info` (`formal/Arare/Budget.lean`):

| Info | Means |
|---|---|
| `Top` | nothing |
| `Inf`, `Bot` | both meanings are `1/0`, or both `0/0` |
| `Num { lo, hi, err, smin, smax }` | both are finite; the exact value is in `[lo, hi]`; Arare's is within `err` of it; its scale is in `[smin, smax]` |

Each numeric Word maps what is known of its operands to what is known of its
result, the Lean function of the same name computed with checked rationals:

| Word | Range | Error added | Known as nothing when |
|---|---|---|---|
| literal `p/q w e` | `[p/q, p/q]` | the exact quantization error | the code does not fit `w` |
| ADD, SUB | sum of the ranges | the operands' errors add | Arare's value may leave int64 at the smallest scale |
| MUL | product of the ranges | `(|x| + e₁)·e₂ + |y|·e₁` | as ADD |
| DIV | the range times `[1/hi, 1/lo]` of the divisor | half a unit of the dividend's scale, plus `e₁/L + |x|·e₂/L²`, `L` the divisor's least magnitude | the divisor's range, widened by its error, reaches zero |
| RESCALE `s` | unchanged | half a unit of the new scale | — |
| NARROW `w` | unchanged | — | Arare's value may leave `w` |
| QUANTIZE `w e` | unchanged | half a unit of `e` when the scale may rise | Arare's value may leave `w` |

A wheel point follows the wheel's rules in both meanings: `0/0` absorbs, `1/0`
plus a number is `1/0`, `1/0` times a number surely nonzero is `1/0`, a
number over `1/0` is `0`, and a number over a divisor surely zero in both
meanings is `1/0` or `0/0`. Where the rule depends on a value the checker does
not know, the result is `Top`.

The rationals (`Q`) are 128-bit, in lowest terms, and every operation answers
the exact result or `None`; `None` turns the result into `Top`. A scale that
would leave ±31 does too — there the kernel refuses the program. So the Rust
checker answers what Lean's `analyze` answers, or nothing.

## A program (AR-LLR-020)

`analyze` runs the program the way the interpreter does (`arare-interp`'s
`machine.rs`, `words.rs`, `lift.rs`), on what is known of each value:

| Abs | Means |
|---|---|
| `Top` | nothing, not even that both meanings are the same kind of value |
| `Nil` | NIL in both |
| `Num { info, nil }` | a number in both, related as `info` says; with `nil`, NIL in both instead may be |
| `Truth { agree }` | a truth value or NIL in each; with `agree`, the same one |
| `Lanes(xs)` | a vector of this length in both, lane by lane |
| `Hull { max, elem }` | a vector of the same length in both, at most `max`, every lane described by `elem` |
| `Quote(items)` | a quote not yet consumed |

- **Numeric Words** lift as the interpreter lifts: NIL is the result, vectors
  pair lane by lane, a one-lane vector or a scalar is reused, and a `Hull`
  pairs with what any lane of the other may be. The operands a RESCALE,
  NARROW or QUANTIZE reads as integers must be exact in both meanings.
- **LT and EQ** agree when both operands are exact (their error is zero) or
  their ranges, widened by their errors, are apart; LT of a wheel point is
  NIL in both. AND, NOT and EQ of agreeing values agree.
- **SELECT** whose condition agrees takes the same branch in both meanings, so
  the result is the `join` of the branches. One whose condition may differ is
  bounded across them: within the hull `[lo, hi]` of the two ranges, Arare's
  value from one branch is at most its error plus `hi − lo` from the exact
  value of the other (`info::cross`).
- **FILTER** whose predicate agrees keeps the same lanes in both meanings, so
  its result is a `Hull` of the input's length; one whose predicate may differ
  is `Top`.
- **MAP** runs its block on each lane, or once on a `Hull`'s lane. **FOLD** and
  **SUM** run lane by lane; over a `Hull` they run once for every length it
  may have and join the results, a FOLD stopping early when an iteration
  changes nothing. SUM skips NIL lanes and may be NIL when nothing is left.

The checker never panics on a program the interpreter would refuse: a Word
whose operands it cannot follow answers `Top`.

## The field contract (AR-LLR-021)

Every literal and every numeric Word's result that is not `Num` is noted, with
why (`wheelPoint`, `mayOverflow`, `divisorMayBeZero`, `unbounded`). The program
is `field=closed` exactly when there is no note. In the Lean model this is
`closed`, and `closed_safe` proves that closed code has no wheel point at any
node — so nothing overflowed — and no DIV with a zero divisor.

## The command (AR-LLR-022)

`arare check` runs the program, then the checker, and prints each value with
`± budget` (`± ?` when there is none) and the contract; `--json` adds
`"budget"` to each value node of `run --json` and `"field"` and `"open"` to the
document; `--lines` checks one program per input line, which is how the oracle
harnesses run thousands of programs in one process (AR-VER-209, AR-VER-217).

## How each layer is held

| Layer | Evidence |
|---|---|
| The numeric checker, in Lean | `budget_sound`, `closed_safe`, `join_sound_left`, `join_sound_right` (AR-VER-107) |
| Rust's numeric functions against Lean's | the same 14,280-line table, diffed in CI (AR-VER-215) |
| Rust's numeric functions against the kernel | every Word over a grid of literals, and two levels deep, held to `Sound` with exact rationals (AR-VER-216) |
| The rationals | grid tests and the order past overflow (AR-VER-216-R); no Kani harness, since symbolic 128-bit division is beyond the solver in CI time |
| The program layer | requirements-based tests (AR-VER-216-C); the oracle corpus and 10,000 generated programs against Ajisai (AR-VER-209, AR-VER-217) |
