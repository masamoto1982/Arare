# Traceability Matrix

Requirement → implementation → verification evidence, as
`QUALITY_POLICY.md` §2 requires, `VERIFICATION_PLAN.md` asks QL-A and QL-B
changes to update, and `RELEASE_VERIFICATION_CHECKLIST.md` asks to be free of
unresolved high-criticality gaps.

Seven source files cite this document by path as their trace target. It did not
exist until this commit, so each of those citations was a dangling reference:
the matrix the quality process is built around had never been written down.

## On the requirement text below

Only **AQ-REQ-007** was ever written out, inline in
`rust/src/coreword_registry.rs`. The text for AQ-REQ-001 through
AQ-REQ-004 appears nowhere in the repository, so the statements below are
**reconstructed from what each verification suite says it covers** — they
record what is actually verified today, not an original requirement anyone
authored. Treat them as descriptive until an owner replaces them; a
reconstruction that disagrees with intent should be corrected here rather than
worked around in the suites.

## Matrix

| Requirement | Statement | Implementation | Verification | QL |
|---|---|---|---|---|
| **AQ-REQ-001** | Exact rational arithmetic and comparison are correct at every boolean decision in the numeric core, including the big-integer and sign-normalization paths. | `rust/src/types/fraction.rs`, `rust/src/types/fraction_arithmetic.rs`, `rust/src/types/fraction_order.rs`; the integer kernels its reduction calls, `rust/src/types/small_divisor.rs` (remainder and gcd by a one-word divisor) and `rust/src/types/lehmer_gcd.rs` (gcd of wide pairs) | `rust/src/types/fraction_mcdc_tests.rs` — **AQ-VER-001**, MC/DC tables `AQ-VER-001-A` `AQ-VER-001-B` `AQ-VER-001-C` `AQ-VER-001-D` `AQ-VER-001-E` `AQ-VER-001-G` `AQ-VER-001-H` `AQ-VER-001-I`; `rust/src/types/fraction_mcdc_tests/repr.rs` — `AQ-VER-001-J` (128-bit gcd dispatch) `AQ-VER-001-K` (machine-word normalizer) `AQ-VER-001-L` (integer reads) `AQ-VER-001-M` (exponent guard) `AQ-VER-001-N` (mixed-representation order and equality, `rust/src/types/fraction_order.rs`); `rust/src/types/fraction_mcdc_tests/arithmetic.rs` — `AQ-VER-001-O` (machine-integer fast path, `sub`) `AQ-VER-001-P` (`BigInt` sums and Henrici's reduction) `AQ-VER-001-Q` (`mul`/`div` `BigInt` arms and `div`'s guards) `AQ-VER-001-R` (`floor`'s remainder row, `round`, `neg`/`abs`) `AQ-VER-001-S` (`balanced_bigint_gcd` dispatch); the two kernels held equal to `num-bigint`'s operators by property tests, `rust/src/types/small_divisor_tests.rs` and `rust/src/types/lehmer_gcd_tests.rs`; `rust/src/types/small_rational_proofs.rs` — `AQ-VER-001-T`, Kani proofs of the machine-word rational kernels (`rust/src/types/small_rational.rs`) over every `i64` where they have no loop and over every small pair where they do (`docs/quality/FORMAL_VERIFICATION.md`); `rust/src/types/small_divisor_proofs.rs` — `AQ-VER-001-U`, a Kani proof over every normalized machine word that the reciprocal `small_divisor` divides by is Möller and Granlund's (the division step itself stays with the property tests) | QL-A |
| **AQ-REQ-002** | Tokenization produces the correct token stream: no token-count drift, no wrong token kind, no mis-classified whitespace or comment. | `rust/src/tokenizer.rs` | `rust/src/tokenizer_mcdc_tests.rs` — **AQ-VER-002**, MC/DC tables `AQ-VER-002-A` `AQ-VER-002-D` `AQ-VER-002-E` `AQ-VER-002-F`; `rust/src/tokenizer_number_mcdc_tests.rs` — `AQ-VER-002-G` (numeric grammar past the sign) `AQ-VER-002-H` (structural gate and string boundaries) `AQ-VER-002-I` (the digit count the numeric-literal ceiling reads); regression suites `rust/src/tokenizer_regression_tests.rs`, `rust/src/tokenizer_regression_tests_2.rs` (literals over zero are the nested module in `rust/src/tokenizer_mcdc_tests.rs`); `rust/src/tokenizer_proofs.rs` — `AQ-VER-002-J`, Kani proofs that the numeric grammar is `spec/grammar.json`'s on every byte string up to six bytes, and that the string literal's close rule and the cuts `value_as_code` writes literals at are the grammar's on every text up to four characters (`docs/quality/FORMAL_VERIFICATION.md`) | QL-B |
| **AQ-REQ-003** | The `(Value, hint)` → wire-format decision that crosses the host boundary is correct for every value domain, and the same decision survives the real `wasm-bindgen` glue. | `rust/src/types/value_protocol.rs`; `rust/src/wasm_interpreter_bindings/mod.rs` (mechanical `JsValue` shim over it) | `rust/src/types/value_protocol_tests.rs` — **AQ-VER-003-C**, native MC/DC and property coverage of the mapping; `rust/wasm-tests/tests/boundary.rs` — end-to-end on `wasm32` via `wasm-pack test --node` | QL-A |
| **AQ-REQ-004** | The host runtime kind (web vs Tauri) is classified correctly from the build-time injection and the runtime DOM fallback. | `src/platform/index.ts` (`detectRuntimeKind`) | `src/platform/index.test.ts` — **AQ-VER-004-A**; configuration in `vitest.config.ts`, which cites the suite as **AQ-VER-004** | QL-A |
| **AQ-REQ-007** | Built-in word purity classification is self-consistent with the effects and determinism each Word declares. (It also covered a derived `safe_preview` flag; no feature read the flag and no specification declared it, so it was removed.) | `rust/src/coreword_registry.rs`, projected from `spec/words.json` via `rust/src/kernel/generated/word_registry.rs` | `rust/src/coreword_registry.rs` (its `tests` module) — **AQ-VER-007**, cases **A**, **B**, **B2**, **C**, **D** (run the subset with `cargo test aq_ver_007`) | QL-B |

## Verification without a requirement ID

`rust/src/interpreter/nil_conformance_tests.rs` cites this document for "NIL
projection conformance" without naming a requirement. It drives the
interpreter and asserts the runtime honors each Word's declared `nil_policy`
and the NIL Projection Rule (`LANG.FAILURE.PROJECT`,
`LANG.FAILURE.PASSTHROUGH`), with registry-driven completeness so a newly
declared passthrough or projecting Word without a behavioral probe fails the
suite.

It is listed here rather than given an ID because assigning one is an owner's
call: the conformance corpus in `tests/conformance/`, not a requirement row, is
what `PORTABILITY.md` makes the definition of Ajisai's behavior.

Three gates added on 2026-10-15 verify specification-level promises and are
listed here for the same reason:

| Promise | Verification |
|---|---|
| A breaking change raises the specification version (`LANG.AUTHORITY.SOURCES`, `spec/README.md`) | `scripts/check-spec-version.mjs` against the pull request's base, with the rule's own tests in `scripts/check-spec-version.test.mjs` |
| A cost axis says when its charge grows with an algebraic value (`LANG.MACHINE.LIMITS`) | `scripts/check-value-dependent-cost.mjs`, which measures every Word that can take a number |
| The corpus reaches every outcome a program can reach (`LANG.CONFORMANCE.CORPUS`) | `scripts/check-conformance-coverage.mjs`, which runs every case and exempts two error categories with their reasons |

The strong Kleene lane tables behind `AND` and `NOT` (`LANG.VALUES.TRUTH`)
are proved over all three lane codes by the Kani harnesses in
`rust/src/interpreter/truth_ops_proofs.rs` (`docs/quality/FORMAL_VERIFICATION.md`):
`AND` is the meet of the truth order, `NOT` the involution reversing it.

The placement rule `SORT` and `ORDER` answer (`LANG.VALUES.ORDER`,
`nulls_last_order` in `rust/src/interpreter/value_order.rs`) is proved by the
Kani harness in `rust/src/interpreter/value_order_proofs.rs`: on every Vector
of up to four elements and every pattern of NILs, the present elements come
first, ascending and stable, and every NIL after them in its own order.

The broadcast every element-wise binary Word performs over two Tensors
(`LANG.COLLECTIONS.LIFT`, `rust/src/interpreter/tensor_ops.rs`) is proved by
the Kani harnesses in `rust/src/interpreter/tensor_ops_proofs.rs` on every rank
up to 2 and every length up to 3: the shapes meet by NumPy's rule, a refusal
names the first aligned axis that does not meet, each operand's lane is found
in bounds, and the flat and the multi-dimensional index are inverse. The work
meter (`LANG.MACHINE.LIMITS`, `rust/src/interpreter/runtime_limits.rs`) is
proved never to undercharge by those in
`rust/src/interpreter/runtime_limits_proofs.rs`, which also prove that every
ceiling allows its limit and refuses one past it. The alignment the lift and
the truth-column connectives share (`common_extent` and `vector_extent` in
`rust/src/interpreter/lane_lift.rs`) is proved by
`rust/src/interpreter/lane_lift_proofs.rs`, and the truth column's packing rule
(`LANG.VALUES.TRUTH`, `rust/src/types/truth_column.rs`) by
`rust/src/types/truth_column_proofs.rs`.

Three rules the interpreter's positional and search Words read through are
proved by Kani harnesses as well: the index rule (`normalize_index`,
`rust/src/interpreter/value_extraction_helpers_proofs.rs`), the split `TAKE`
and `DROP` share (`rust/src/interpreter/vector_ops/split_proofs.rs`), and
`BSEARCH`'s halving (`partition_point`,
`rust/src/interpreter/search_ops_proofs.rs`).

The termination argument (`LANG.DICTIONARY.ACYCLIC`, `LANG.SOURCE.CODE`,
`spec/termination.json`) is verified by `rust/tests/termination_laws.rs`:
every witness runs as the argument says, and a property over random programs
built from block-copying pieces finds no run a ceiling ends (with the
`nestedDataBlock` check taken out, it finds the self-applying block at once).
`rust/src/runtime_limits_tests.rs` pins that such a block is refused before
the `callDepth` ceiling, with nothing left counted.

## Unassigned requirement IDs

Requirement IDs are not dense. `AQ-REQ-005` and `AQ-REQ-006` are unused: no
evidence survives of what they were meant to be, so they are left unassigned
rather than backfilled with something invented. Nothing in the source may claim
them until someone decides what they are.

## Retired verification IDs

These were live and are not, so that finding them in history is not confusing.
Each went when the code it was the sole verification of went — a verification
ID outlives its subject only as a dangling claim.

| ID | Subject | Retired |
|---|---|---|
| `AQ-VER-003-A` | `resolve_effective_hint` (arena → JS hint resolution) | with the arena value representation |
| `AQ-VER-003-B` | `build_bracket_structure_from_shape` (`#[cfg(test)]` bracket rendering) | with the arena value representation |
| `AQ-VER-007-E` | `is_safe_preview_word` decision truth table | with that function, whose only caller was a WASM export nothing called |

`AQ-VER-003-C` keeps its letter rather than being renumbered: the letters are
identifiers, not an ordering, and renaming a live ID to close a gap would break
every citation of it for no gain.

## Keeping this true

`scripts/check-traceability-matrix.mjs` (`npm run check:traceability`, run in
CI) enforces both directions this document can drift in:

- every `AQ-VER-*` / `AQ-REQ-*` ID in the source has a row here, so adding a
  suite without recording it fails;
- every live ID and every file path this document names exists in the tree, so
  a row cannot outlive its subject — and an ID listed above as retired or
  unassigned must stay absent from the source.

The gate exists because that drift had already happened before it was written:
the three retired IDs above went with their subjects in the two commits before
this one, and nothing noticed. A name-reachability check cannot tell whether a
row *describes* its suite correctly, only whether both ends still exist; the
statements themselves are a reviewer's job.
