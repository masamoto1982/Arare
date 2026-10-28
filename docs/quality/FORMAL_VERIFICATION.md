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

**Phase 0: nothing about Arare's semantics is proved yet.**

- `formal/Arare/Basic.lean` holds one placeholder theorem,
  `Arare.toolchain_live`, so the Lean CI job has something to check. It proves
  nothing about Arare.
- The Rust workspace has no Kani harness. `cargo kani` exits successfully when
  it finds none, so the Kani CI job is green without proving anything; the
  first harnesses arrive with the kernels in Phase 2
  (`PROBLEM_REPORTS.md`, PR-0003).

The theorems Phase 1 and Phase 3 owe are listed in WORK_ORDER.md §5.4:
the wheel axioms over the value codes, the strong Kleene truth lattice,
agreement with ℤ inside `field=closed` code, the half-ulp bound on RESCALE,
and the soundness of error-budget inference.

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
