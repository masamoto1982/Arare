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
| Error budget, empirically | The Ajisai oracle | `|Arare − Ajisai| ≤` the inferred budget on generated programs |

## Status

**Phase 1: the value model is proved; no implementation exists yet to hold
to it.** 80 theorems in `formal/Arare/`, each resting only on `propext`,
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

### What is not proved

Read these as open, not as proved by implication:

- **Scales and width selection.** The model computes on codes at one common
  scale. Aligning scales before ADD and SUB, adding exponents in MUL, choosing
  DIV's result scale and choosing the narrowest width are tested from Phase 2,
  not proved (AR-HLR-001, 003, 007).
- **Equality across widths and scales**, and MIN, MAX and SORT on points, are
  not modelled (AR-HLR-010, 011).
- **Agreement with Ajisai.** The model does not contain Ajisai's wheel of
  fractions, so that Arare's rules are Ajisai's restricted to fixed point is
  argued in ADR-0003, not proved; the oracle comparison of Phase 2 checks it
  empirically.
- **The reciprocal wheel axioms** do not hold for fixed point and are not
  claimed (ADR-0003).
- **NARROW inside expressions.** `Closed.lean`'s language has no NARROW; its
  `fit` is the widest-width limit. NARROW on its own is AR-VER-101.
- **The static range proof** that makes code `field=closed` (AR-HLR-017) and
  the error budget are Phase 3.
- **Aggregates other than SUM** and the Arrow layout (AR-HLR-019, exempt from
  proof by its nature) are Phase 2 tests.
- **The implementation.** No Rust code exists yet; there is no Kani harness
  and `cargo kani` passes when it finds none (PR-0003).

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
