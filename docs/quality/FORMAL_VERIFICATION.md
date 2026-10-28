# Formal Verification

## What it adds

Ajisai's tests check behavior at the inputs someone wrote down (unit, MC/DC
and conformance tests) or at inputs a generator draws (the `proptest`
properties). Neither says anything about the inputs it did not reach, and the
numeric core's defects live at exactly the inputs nobody thinks to write:
`i64::MIN`, a sum one past `i64::MAX`, a cross product whose 128-bit width
the fast path forgot.

The harnesses here close that gap for the kernels small enough to decide. They
run under [Kani](https://github.com/model-checking/kani), a bit-precise
bounded model checker for Rust: each `kani::any()` value stands for **every**
value of its type, and the solver either proves the assertions hold for all of
them — and that no arithmetic overflow, out-of-bounds index or panic is
reachable — or returns the counterexample. A passing harness is a proof, not
a sample.

## Running

Kani pins its own nightly toolchain and solver, so it is installed apart from
the project's stable toolchain:

```sh
cargo install --locked kani-verifier --version 0.68.0   # the version CI pins
cargo kani setup
```

Then, in `rust/`:

```sh
cargo kani --jobs                 # every harness, in parallel
cargo kani --harness truth_ops    # the harnesses whose path matches
```

The harnesses compile only under `cfg(kani)`, which `cargo kani` sets: a plain
`cargo build` or `cargo test` never sees them, and `Cargo.toml` declares the
cfg so `clippy -D warnings` accepts it. CI runs every harness in the
`formal-verification` job of `.github/workflows/test.yml`.

## Two kinds of harness

- **Whole-range** harnesses quantify over every value of a machine type. They
  are possible where the code under proof has no loop, so the solver sees all
  of it at once.
- **Small-scope** harnesses take every input below a bound and unroll the
  loops in between to a fixed depth (`#[kani::unwind]`); Kani fails the
  harness if the bound is too small to reach a loop's exit, so the bound
  cannot silently cut a path off. They rest on the small-scope hypothesis: a
  defect in a gcd or a normalization shows itself on small operands too. The
  property tests remain the evidence for large ones.

Each harness states its law a second way — the strong Kleene truth order, or
exact `i128` arithmetic where no product of two words overflows — rather than
restating the kernel, so it can disagree with it.

## The harnesses

| File | Subject | Kind |
|---|---|---|
| `rust/src/interpreter/truth_ops_proofs.rs` | The `AND` table and `NOT` lane rule of LANG.VALUES.TRUTH: `AND` is the meet of F < U < T (so a bounded semilattice), `NOT` the order-reversing involution fixing UNKNOWN, and De Morgan gives the join | Whole-range (the three codes) |
| `rust/src/types/small_rational_proofs.rs` | The machine-word rational kernels (`small_rational`): the wasm32 multiply, rounding, the order, the integer routes of `add` and `mul`, the gcd's power-of-two shortcut | Whole-range (every `i64`) |
| `rust/src/types/small_rational_proofs.rs` | Stein's gcd against Euclid's; `add`, `mul` and `div` answering the exact value in lowest terms over a positive denominator; the order agreeing with equality of normalized pairs | Small-scope (halves under 2⁴, gcd operands under 2⁶, and for the gcd's remainder shortcut a larger operand past 2¹⁶ times the smaller) |
| `rust/src/types/small_divisor_proofs.rs` | The reciprocal every one-word division by `small_divisor` (and `fraction_word_term`'s running sum) starts from: for a normalized divisor it fits a word and is Möller and Granlund's, stated by its bounds | Whole-range (every normalized `u64`) |
| `rust/src/interpreter/value_order_proofs.rs` | The placement rule of LANG.VALUES.ORDER that `SORT` and `ORDER` answer (`nulls_last_order`): a permutation with the present elements ascending and stable, then every NIL in its own order | Small-scope (every Vector of up to four elements, every pattern of NILs, every order with ties on the rest) |
| `rust/src/interpreter/tensor_ops_proofs.rs` | The index algebra of LANG.COLLECTIONS.LIFT's broadcast (`tensor_ops`): `aligned_dim` reads a shape from the right; `broadcast_dims` is NumPy's rule, names the first axis that does not meet and is symmetric; `project_broadcast_index` lands in the operand; `ravel_index` and `unravel_index` are inverse over row-major strides | Small-scope (every rank up to 2, every length up to 3) |
| `rust/src/interpreter/runtime_limits_proofs.rs` | The work meter of LANG.MACHINE.LIMITS (`runtime_limits`) never undercharges, and every ceiling allows its limit and refuses one past it, naming its resource, limit and observation: limbs cover every bit, joining measures is a commutative monoid, a sort is priced at no less than `n⌈log₂n⌉`, the operand order does not move a price, nothing is free, and a wider operand or more lanes is never cheaper | Whole-range for the linear laws; small-scope where two measures multiply (down to 16 lanes and four limbs for the monotone one) |
| `rust/src/interpreter/lane_lift_proofs.rs` | LANG.COLLECTIONS.LIFT's alignment, shared by the lift and the truth-column connectives: one lane sets no extent (`vector_extent`), and the operands' extents meet in the one width they share or a refusal naming the first two that differ (`common_extent`), whatever their order | Whole-range (`vector_extent`); small-scope (every sequence of up to four operands, each a scalar or up to three lanes) |
| `rust/src/types/truth_column_proofs.rs` | The truth column's packing rule (`pack_codes`): a flat Vector packs exactly when every lane is a truth value or one shared NIL, each lane's code is its own, and the NIL kept is the first | Small-scope (every sequence of up to four lanes, NILs of two absences) |
| `rust/src/tokenizer_proofs.rs` | The canonical lexical grammar (`spec/grammar.json`): the numeric grammar (`is_number_bytes`) is a finite automaton written from `numericGrammar`'s EBNF; a string literal closes at its first quote that whitespace or the end follows (`closing_quote`); and the cuts `string_literal_pieces` makes (`literal_cuts`) fall after every such quote and nowhere else | Small-scope (every byte string up to seven bytes, the shortest lexeme with sign, fraction and signed exponent; every text up to four characters over quote, space, line break and other) |
| `rust/src/interpreter/value_extraction_helpers_proofs.rs` | The index rule `GET`, `PUT` and the other positional Words read through (`normalize_index`): a non-negative index counts from the front and a negative one from the back, and the answer is a position inside the Vector exactly when the index names one | Whole-range (every `i64` index, every length a Vector can have, up to `isize::MAX`) |
| `rust/src/interpreter/vector_ops/split_proofs.rs` | The split `TAKE` and `DROP` share (`compute_take_bounds`, `compute_drop_bounds`): past the end exactly when `|count|` exceeds the length, otherwise two in-bounds slices that are each other's complement, priced at the elements they copy | Whole-range (every `i64` count, `i64::MIN` included, every length a Vector can have, up to `isize::MAX`) |
| `rust/src/interpreter/search_ops_proofs.rs` | `BSEARCH`'s halving (`partition_point`): over an ascending sequence it answers how many elements lie below the key, so the element there decides the search; it asks only in bounds and at most ⌊log₂ n⌋ + 1 times; an unplaceable pair or a comparison error stops it | Small-scope (every ascending sequence of up to five elements, duplicates included, every key) |

What the small scope does not reach is said here so it is not mistaken for
proved: `add`'s 128-bit fraction route and `div`'s widened fallback answer
only once a cross product leaves a word, which no operand under 2⁴ does, and
proving them would mean identities between 64-bit products (see below); they
stay with the property tests. Every harness runs on the 64-bit host, so a
narrowing from `i64` to `usize` is proved for a 64-bit `usize`, not wasm32's.

In `TRACEABILITY_MATRIX.md` the rational harnesses are `AQ-VER-001-T` and the
one-word reciprocal harness `AQ-VER-001-U`, both under AQ-REQ-001; the
tokenizer harnesses are `AQ-VER-002-J`, under AQ-REQ-002; the
truth-table, truth-column, order, alignment, broadcast, work-meter, index,
split and search harnesses are listed among the verification without a
requirement ID.

## Adding a harness

- Put it in a `*_proofs.rs` file beside its subject, attached as the
  subject's `#[cfg(kani)] #[path = "..."] mod proofs;`, the way the test
  modules attach, so it can reach private items without widening them.
- Prefer a whole-range harness; when the subject loops, bound the input, set
  `#[kani::unwind]` just high enough, and say in the doc comment what the
  bound covers.
- Keep the subject narrow. A `Value`, or anything that may reach a
  `BigInt` or a formatted error message, gives the solver more paths than it
  finishes: factor the logic worth proving into a function over plain data
  (as `nulls_last_order` is), and prove that.
- Give a harness concrete lengths. A symbolic length hands the solver every
  branch the standard library has for long inputs; run each length, and each
  pattern that decides one, as its own concrete case, and keep the contents
  symbolic.
- Identities between products of two 64-bit words are out of reach: neither
  the SAT solver nor Z3 decided `div_two_by_one`'s within an hour, and even the
  commutativity of a saturating product of two free words ran past ten
  minutes. Prove what such a step relies on instead, and leave the step to the
  property tests; where a law compares two products (a price is monotone),
  bound the factors to a few bits each.
- Text is the costliest input. A `str` built from symbolic bytes hands the
  solver the UTF-8 validator, `format!` its formatting machinery, and
  `char::is_whitespace` the Unicode table; each ran out of memory or time.
  Factor a scan over `(usize, char)` items, as `closing_quote` and
  `literal_cuts` are, feed it characters from the few kinds its rule tells
  apart, and return an iterator rather than a `Vec` the proof must grow.
- A walk over a symbolic graph is out of reach. `find_reference_cycle`, the
  DEF-time check LANG.DICTIONARY.ACYCLIC rests on, was factored over any
  graph and any reached-from map; with a `HashMap` the solver did not leave
  the hash table's probing, and with an array map on three names it ran out
  of memory converting the formula. The check stays with
  `rust/tests/termination_laws.rs` and `dependents_index_tests`.
- A `Vec` comparison is a `memcmp` over its bytes, eight a `usize` lane, so a
  harness that compares two short Vecs needs an unwinding bound of `8 · len + 1`.
- Keep each harness to a few minutes of solver time (the slowest today,
  `add_is_the_normalized_sum`, takes about eight): CI runs them all on every
  pull request. Most of a small-scope harness's formula is its specification;
  compute it in the narrowest type the scope fits, as `small_rational_proofs`
  does in 16 and 32 bits.
