# Formal Verification

What is proved about Arare, with which tool, and — just as binding — what is
not. A claim in this file that something is proved must name the theorem or
harness that proves it (CLAUDE.md, rule 8). The arrangement follows the
reading of DO-333 in `PLAN_OVERVIEW.md`: a proof may stand in for testing
exactly where it is stated, and nowhere else.

## The layers

| Layer | Tool (pinned in `CONFIGURATION_INDEX.md`) | Subject |
|---|---|---|
| Semantic model | Lean 4, `formal/` | The value model of WORK_ORDER.md §3 and its laws |
| Model to code | Differential tests | `rust/crates/arare-spec` against the Lean model, on every int8 operand pair |
| Implementation | Kani, `rust/` | Each kernel against `arare-spec`, over its whole range or a stated small scope |
| Compiled code | Translation validation | Native and WASM output against the reference interpreter |
| Error budget | Lean 4, `formal/`; differential tests; the Ajisai oracle | The checker's soundness, proved over the numeric Words; the Rust checker against the Lean one; `|Arare − Ajisai| ≤` the inferred budget on the corpus and on generated programs |

## Status

**Phase 3: the value model is proved in Lean, mirrored in Rust and diffed
against it, and the kernel is proved against laws in Kani. The error-budget
checker is proved sound in Lean over the numeric Words, computed in Rust as
the same functions and diffed against Lean, and held to Ajisai on the corpus
and on at least 10,000 generated programs.** The front end and the
interpreter are QL-B: tested, compared with the Ajisai oracle (AR-VER-209),
not proved.

### Lean

119 theorems in `formal/Arare/`, each resting only on `propext`,
`Classical.choice` and `Quot.sound` (`npm run check:formal` runs
`scripts/check-formal-axioms.mjs` after `lake build`, because `lake build`
alone accepts `sorry`). Every theorem is cited by a requirement in
`spec/requirements.json`, and every QL-A requirement on values cites at least
one (`npm run check:requirements`).

| File | Verification ID | What is proved | Requirements |
|---|---|---|---|
| `formal/Arare/Codes.lean` | AR-VER-101 | The code layout for every half-range M ≥ 2: values and codes are in bijection apart from the one unused code, negation stays in range; `fit` (NARROW and the widest-width limit) answers the exact value or 1/0, never a wrapped one | AR-HLR-001, 002, 004, 009 |
| `formal/Arare/Wheel.lean` | AR-VER-102 | The exact domain ℤ ∪ {1/0, 0/0}: the reciprocal-free wheel axioms (both commutative monoids, Carlström's distributive law, `0·0 = 0`, `(x + 0y)z = xz + 0y`, `0/0` absorbing); the ring laws that fail at the points; finite arithmetic is ℤ's; a point never turns finite under `+ − ·` and negation; the division rules and the half-unit bound of DIV | AR-HLR-003, 005, 006, 007 |
| `formal/Arare/Rounding.lean` | AR-VER-103 | Round half to even as an integer quotient: within half a unit, ties to even, exact quotients unchanged; RESCALE within half an ulp, RESCALE by 0 the identity; DIV's bound for either sign of divisor | AR-HLR-007, 008 |
| `formal/Arare/Truth.lean` | AR-VER-104 | Strong Kleene: AND the meet and OR the join of FALSE < UNKNOWN < TRUE, NOT the reversing involution; commutative, associative, idempotent, absorptive, distributive, De Morgan, Kleene's law; excluded middle fails at UNKNOWN | AR-HLR-012 |
| `formal/Arare/Closed.lean` | AR-VER-105 | Over an expression language of literals, points, `+ − ·`, negation and DIV: closed code computes the exact integer; a machine that fits every intermediate into the widest width answers that integer or a point, never another finite number, and the exact integer when nothing overflows; associativity does not survive an intermediate overflow | AR-HLR-003, 004, 017 |
| `formal/Arare/Sql.lean` | AR-VER-106 | NIL in truth position is UNKNOWN; EQ always decided; order of a point UNKNOWN; FILTER keeps exactly the TRUE elements; a branch takes THEN only on TRUE; SUM skips NIL and is NIL when nothing is left | AR-HLR-010, 011, 013, 014, 015, 016 |
| `formal/Arare/Budget.lean` | AR-VER-107 | Over an expression language of quantized literals, the wheel points, ADD, SUB, MUL, NEG, DIV, RESCALE, NARROW and QUANTIZE, with an exact meaning on rationals (Ajisai's) and a machine meaning on codes at static scales (Arare's kernel, fitted into the widest width): the checker `analyze` is sound — every wheel point it names both meanings reach, every range it gives holds the exact value, and every error it gives bounds the machine's distance from it (`budget_sound`); code it bounds everywhere has no wheel point at any node and no DIV by zero in either meaning (`closed_safe`); a join is right about either side (`join_sound_left`, `join_sound_right`) | AR-HLR-027, 028, 029 |

### Model to code (AR-VER-201)

`formal/Table.lean` runs the Lean model over a fixed grid — every pair of int8
values under ADD, SUB, MUL, DIV, EQ and LT, NEG, RESCALE, the two rounding
quotients, `fit`, every code, the truth connectives, SUM, FILTER and SELECT —
and prints 71,864 lines. `rust/crates/arare-spec/src/table.rs` prints the
same grid from the Rust executable specification. The Lean job of CI diffs
the two and fails on any difference. This ties `arare-spec` to the model on
that grid, not beyond it: the theorems cover every input, the diff covers the
grid.

### The checker, model to code (AR-VER-215)

`formal/BudgetTable.lean` runs the Lean checker over a grid — 17 literals
(integers, rationals that round and that do not, one out of its width, the two
wheel points) under seven unary Words, and every pair of those under ADD, SUB,
MUL, DIV and the join — and prints 14,280 lines;
`rust/crates/arare-budget/src/table.rs` prints the same grid from the Rust
checker. The Lean job of CI diffs the two. The Rust checker computes Lean's
functions with checked 128-bit rationals and answers nothing where one would
not fit (ADR-0008), so beyond the grid it answers Lean's result or nothing —
an argument from the code's structure, not a proof. The Rust checker is also
held to the kernel directly: every Word over a grid of literals, two levels
deep on part of it, with the claim `Sound` checked against exact rationals
(AR-VER-216).

### Kani (AR-VER-207)

`rust/crates/arare-kernel/src/proofs.rs`, run by the Kani job, which also
fails when fewer harnesses are verified than the source declares (`cargo kani`
alone passes with none).

| Harness | What it proves | Scope | Time (local) |
|---|---|---|---|
| `and_word_is_lanewise_and` | Word-wide Kleene AND is the scalar AND on every lane, and stays canonical | Every pair of canonical 64-lane words | < 1 s |
| `not_word_is_lanewise_not` | The same for NOT | Every canonical word | < 1 s |
| `fit_is_exact_or_inf` | `fit` answers the integer itself inside the range and 1/0 outside | Every `i128`, every width | < 1 s |
| `codes_round_trip` | Every code decodes to a value that encodes back; finite only in range | Every `i64` code, every width | < 1 s |
| `rescale_is_round_half_even_w32` | The shift-based RESCALE equals round half to even of the Euclidean quotient | Every int32 code, every shift ≤ 31 | 3 s |
| `add_exact_w16_w32` | ADD of int16 is the exact sum at int32; of int32, at int64 | Every pair | 2 s |
| `add_w64_any_scales_no_overflow` | ADD at int64 at any two scales never overflows an intermediate and answers the exact aligned sum or 1/0 | Every pair, every scale pair in ±31 | 7 s |
| `mul_exact_w16` | MUL of int16 is the exact product at int32 | Every pair | 9 s |
| `div_half_unit_w16` | DIV is within half a unit of the exact quotient (the law `Arare.W.div_error` proves for the model), ties to even | Every pair, divisor nonzero, scale 0 | about 9 min |

`add_w64_any_scales_no_overflow` found a defect on its first run: `Num::new`
checked the scale with `scale.abs()`, which overflows for `i32::MIN` and in a
release build wraps to a negative number that passed the check. The check is
now a range check, with a regression test in
`rust/crates/arare-kernel/tests/widths.rs`.

### What is not proved

Read these as open, not as proved by implication:

- **Scales and width selection in the model.** The Lean model computes on
  codes at one common scale. The kernel's alignment, width rule and DIV result
  scale are tested exhaustively over int8 values at exponents {−2, 0, 3}
  (AR-VER-203) and proved free of overflow for ADD at int64 (Kani), but no
  theorem states them (AR-HLR-001, 003, 007).
- **MUL beyond int16, DIV beyond int16 at scale 0, EQ and LT across scales**
  are tested, not proved: identities between products of two wide words are
  out of the solvers' reach.
- **Agreement with Ajisai.** Argued in ADR-0003, not proved. The oracle
  comparison checks it within the inferred budget on a corpus (AR-VER-209) and
  on generated programs (AR-VER-217), which is evidence on those programs, not
  a proof.
- **The reciprocal wheel axioms** do not hold for fixed point and are not
  claimed (ADR-0003).
- **NARROW inside expressions.** `Closed.lean`'s language has no NARROW; its
  `fit` is the widest-width limit. NARROW on its own is AR-VER-101.
- **The checker beyond the numeric Words.** `Budget.lean`'s language has no
  vectors, truth values or stack. That a truth value agrees between the two
  meanings (an order or equality of exact or separated numbers), the bound
  across a SELECT whose condition may differ (`info::cross`), and the run of
  the checker over a program's stack, vectors and loops (`check.rs`,
  `words.rs`) are tested (AR-VER-216-C) and compared with Ajisai on the corpus
  and on at least 10,000 generated programs (AR-VER-209, AR-VER-217), not
  proved (AR-HLR-029). Only `join`, which lanes and loop iterations go
  through, is proved.
- **The checked rationals** (`rat.rs`, AR-LLR-018) have no Kani harness:
  symbolic 128-bit division is beyond the solver in the time CI allows (a
  harness for `Q::new` over 8-bit operands took 13 minutes locally, and over
  10 with CaDiCaL). They are tested on a grid and across overflow
  (AR-VER-216-R) and through the soundness tests and the Lean–Rust table.
- **The Rust checker beyond the grid.** That it computes Lean's functions or
  nothing rests on its structure (each function written after the Lean one,
  every rational operation checked) and the 14,280-line diff, not on a proof
  (AR-LLR-019).
- **The exact meaning is Ajisai's only where the oracle shows it.** The
  model's exact meaning is written from ADR-0007's prelude; that Ajisai
  computes it is what the oracle comparison checks, on its programs.
- **Aggregates other than SUM**, numeric columns, and 64-byte buffer alignment
  (AR-HLR-019 is exempt from proof by its nature; its bitmaps are tested and
  proved lane-wise).
- **The front end and the interpreter** (QL-B): tested and compared with the
  oracle, not proved. QUANTIZE's rounding is the proved `roundDiv` and
  `rescale` (AR-HLR-026); its kernel implementation is tested over a grid
  (AR-VER-208), with no Kani harness.
- **Widths in the budget model.** Like `Closed.lean`, `Budget.lean` fits
  arithmetic results into the widest width only; that the kernel's narrower
  result widths never change a value is tested (AR-VER-203, AR-VER-204), not
  proved.

## Running

Lean, in `formal/`. Install elan once; it reads the pinned toolchain from
`lean-toolchain` and installs exactly that version:

```sh
curl -sSfL https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh | sh -s -- -y --default-toolchain none
lake build
```

Where elan cannot reach Lean's release server but can reach GitHub (as in some
sandboxed sessions), download the release archive and link it under the pinned
name:

```sh
curl -sSLO https://github.com/leanprover/lean4/releases/download/v4.34.0/lean-4.34.0-linux.zip
unzip -q lean-4.34.0-linux.zip
elan toolchain link leanprover/lean4:v4.34.0 "$PWD/lean-4.34.0-linux"
```

Kani, in `rust/`:

```sh
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup
cargo kani --jobs                 # every harness
cargo kani --harness <name>       # the harnesses whose path matches
```

Harnesses compile only under `cfg(kani)`, which `cargo kani` sets; the
workspace declares the cfg so `clippy -D warnings` accepts it.

## Writing a Kani harness

Arare inherits Ajisai's experience with Kani; these rules come from it.

- Put a harness in a `*_proofs.rs` file beside its subject, attached as
  `#[cfg(kani)] #[path = "..."] mod proofs;`, so it can reach private items.
- State the law a second way — against `arare-spec`, or in a wider integer
  type — rather than restating the kernel, so the harness can disagree with it.
- Prefer a whole-range harness. Fixed-width kernels without loops are where
  Arare can afford them: every int8 and int16 input, and every int32 input
  where no product of two words is involved.
- When the subject loops, bound the input, set `#[kani::unwind]` just high
  enough, and say in the doc comment what the bound covers.
- Identities between products of two 64-bit words are out of reach of the
  solvers; prove what such a step relies on and leave the step to the
  exhaustive and property tests.
- Text, hash maps and symbolic lengths give the solver more paths than it
  finishes. Factor the logic worth proving into a function over plain data and
  concrete lengths.
- Keep each harness to a few minutes of solver time; CI runs all of them on
  every pull request.

## Writing a Lean proof

- One file per subject under `formal/Arare/`, imported from `formal/Arare.lean`.
- A definition the Rust executable specification mirrors carries the same name
  in both, so the differential tests can be read side by side.
- A theorem that discharges a requirement names its `AR-HLR-*` ID in its doc
  comment, so the traceability check reaches it.
- A dependency (Mathlib, when needed) is pinned by `lake-manifest.json` and
  listed in `CONFIGURATION_INDEX.md`.
