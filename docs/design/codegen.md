# The compiled route

The design behind AR-LLR-023 to AR-LLR-027 and AR-LLR-029: how `rust/crates/arare-codegen`
compiles a program for a signature into Rust that runs a fast path and falls
back to the reference interpreter (AR-HLR-030), how columns carry their wheel
points and NIL lanes (AR-HLR-031), and how the result is checked and timed.
The decision and its alternatives are ADR-0009.

## Signatures and inputs

A compiled program is a function of its inputs, pushed bottom first before
the program runs, as `arare_interp::run_on` pushes them. A signature names
each input's shape, width and scale: `v8:0` is a vector of int8 at exponent 0,
`s32:-30` a scalar of int32 at exponent −30. At run time an input is an `Arg`:
a scalar `Num`, or a `Column` — codes of one width and scale and, when some
lane is NIL, an Arrow validity bitmap. `Column::new` checks every code once
and records whether the column holds a wheel point or a NIL lane.

## Lowering (AR-LLR-024)

`lower` runs the program once on symbols — a scalar node, a vector whose lanes
are a node, a quote — the way the interpreter runs it, and each Word adds a
node:

| Node | Computes, on codes |
|---|---|
| `Add`, `Sub` | `(a << da) ± (b << db)`, aligned to the smaller exponent |
| `Mul` | `a · b` |
| `Div` | `round((a << p) / (b << q))`, half to even, at the dividend's scale |
| `Neg`, `Rescale`, `Narrow` | as the kernel's |
| `Up`, `Down` | QUANTIZE up (RESCALE, then a fit) or down (a shift, then a fit) |
| `Lt`, `Eq`, `And`, `Not`, `Select` | lane by lane; EQ on scalars only |
| `Sum` | a reduction over the loop |
| `Acc`, `Fold` | FOLD's accumulator, carried from lane to lane, and its value after the loop |

A node's width and scale are what the kernel's own operation gives
placeholder numbers of its operands' widths and scales. Its bound is the
largest code magnitude on finite inputs; a node whose bound exceeds its width,
and every DIV, is checked. MAP runs its block on the lane node, so the block's
nodes are lane nodes. SUM's width is the one the interpreter's running sum
settles at (int64), and the widths before it are recorded, so the fast path
can require enough lanes for the settled width to apply.

Names and BIND (AR-LLR-029) are lowered as the interpreter runs them
(AR-LLR-028): BIND binds a name to a symbol in the innermost scope, a name
pushes the symbol it is bound to, and every block lowered — MAP's, FOLD's —
opens a scope of its own. What the interpreter refuses (an unbound name, BIND
of a name, a block that reads its own name) is refused here too, so the fast
path never answers where the interpreter stops.

FOLD over a vector from a scalar becomes an accumulator node, `Acc`, whose
width and scale are the initial value's and whose bound is its width's; the
block is lowered once, on the accumulator and the lane, and must leave one
scalar of the same width and scale — then every iteration is the same code,
and a step that leaves the width is checked like any other node. A block that
widens the accumulator (`0 [ ADD ] FOLD` over int8 lanes) is refused: written
with a NARROW, as the logistic map below is, it compiles.

Refused, with the reason: a literal wheel point or NIL, a quote used as data,
FILTER, a FOLD whose block changes the accumulator's width or scale, BIND of
several names, a lane value computed from a reduction (a second pass), EQ of
vectors, and an operand that must be a literal integer and is not
(PR-0011).

## Emission (AR-LLR-025, AR-LLR-026)

`emit` writes two functions per program: `NAME_fast`, the fast path, which
answers `None` when it does not apply, and `NAME`, which calls it and
otherwise `rt::fallback`, the interpreter on the same inputs. The fast path:

1. takes every input's plain codes — `None` unless the width and scale are the
   signature's and the column has no point and no NIL — and requires equal
   lengths;
2. for each SUM, requires enough lanes for the settled width and lanes × the
   lane bound within int64, so no prefix of the sum can overflow;
3. computes the uniform nodes once, then the lane nodes in one loop over the
   lanes in chunks of 4,096, each in the narrowest of `i16`, `i32`, `i64` and
   `i128` its bound needs (an int8 ADD runs in `i16`, as many SIMD lanes as a
   hand-written loop), a SUM in a per-chunk partial of a type that holds the chunk, a
   vector written straight into the output, in one pass over the lanes rather
   than chunk by chunk, when it is the only result;
4. sets `trap` on a zero divisor or a code that leaves its width, and answers
   `None` if anything trapped. The loop does not stop at a trap, so a value
   after one can lie outside its bound; ADD, SUB, MUL, NEG and the partial
   sums wrap, so that garbage, which is discarded, cannot panic a debug
   build. No wrapped value is ever answered: a trapped run is the
   interpreter's (AR-HLR-004);
5. starts each FOLD's accumulator at its initial value before the loop and
   replaces it with the block's result after each lane;
6. computes what depends on the reductions after the loop.

A loop with a SUM and no FOLD sums, under WebAssembly, each lane of a group of
sixteen into a partial of its own: with one partial, LLVM vectorizes the loop
for WebAssembly four lanes at a time, a vector of four `i32`s; with sixteen it
loads sixteen lanes at a time. Natively one partial vectorizes better, so the
emitted text holds both loops behind `cfg!(target_arch = "wasm32")`, a
constant the compiler folds.

RESCALE's rounding is branch-free (`rt::rescale_i32`): the half-to-even tie is
two compares and an add, which vectorizes under WebAssembly as well as
natively. DIV uses `rt::div_f64`, the `f64` quotient rounded half to even, when the
dividend is below 2^52 and the divisor below 2^53: the correctly rounded
quotient cannot cross a half-integer the exact one does not (AR-LLR-026), and
it vectorizes. RESCALE uses `rt::rescale_i32` where the code fits `i32`.

The text depends on nothing but the program and the signature. Under
WebAssembly it is the same text, compiled by `rustc` for `wasm32-wasip1` with
SIMD128.

## The logistic map

AR-PERF-003's program is twenty steps of x' = (13/4)·x·(1 − x) from x = 1/2,
one step per lane of a vector of twenty, written in y = x − 1/2:

```text
0 16 -15 QUANTIZE [ 'I' BIND 'Y' BIND
  5/16 16 -15 QUANTIZE 13 Y MUL Y MUL 2 RESCALE 1/4 8 -2 QUANTIZE MUL SUB
  15 RESCALE 16 NARROW ] FOLD 1/2 16 -15 QUANTIZE ADD
```

y' = 5/16 − (13/4)·y² is the same map, and Ajisai's exact x₂₀ is the same
rational, but the checker bounds it: it carries an error through `y·y` as
`2|y|·e`, and |y| stays near 0.31 and 0 on the map's two-cycle, so the error
it infers settles (about 4.9 × 10⁻⁵) where the x form's would grow by 13/4 a
step. r = 13/4 is the period-two regime, where the map contracts; in a chaotic
one (r = 3.7) any fixed-point run of twenty steps loses all its digits, and
the checker rightly bounds nothing. y stays at int16 and 2⁻¹⁵: MUL adds
exponents, and two operands finer than 2⁻¹⁵ would leave ±31.
`arare-compiled` compiles the programs listed in its `programs.txt` this way
in its build script; `arare compile` (AR-LLR-027) prints the same text.

## How it is held

| Layer | Evidence |
|---|---|
| Lowering and emission | AR-VER-223: every Word's widths and scales, bounds, checks and refusals; deterministic text |
| The run-time support | AR-VER-224: column checks and flags, values, SUM widths, the fallback |
| The fast-path arithmetic | AR-VER-222: `div_f64` and `rescale_i32` against the kernel on every int8 pair, ties and near-ties to the limits, and seeded random operands |
| The compiled programs | AR-VER-220, translation validation: every int8 pair for the lane-wise Words, seeded random and boundary inputs at every width for every program, wheel points, NIL and mismatched inputs included; the fast path must answer on finite inputs |
| The compiled programs under WebAssembly | AR-VER-220 again: the same tests, built for `wasm32-wasip1` with SIMD128 and run under Node's WASI (`scripts/lib/run-wasi.mjs`), where reductions take the grouped loop |
| Speed | AR-VER-221: `rust/bench` against hand-written loops and Ajisai's native build (AR-PERF-001 to 006; for AR-PERF-003, also within the inferred budget of Ajisai's exact x₂₀) |
| WebAssembly against native | AR-VER-226: `rust/crates/arare-timing`, built both ways, compared by `scripts/check-wasm-speed.mjs` (AR-PERF-007) |
