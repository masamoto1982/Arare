# Kernel and executable specification

The design behind AR-LLR-001 to AR-LLR-009: how `rust/crates/arare-spec` and
`rust/crates/arare-kernel` carry the value model of `spec/requirements.json`
(AR-HLR-001 to AR-HLR-019) and how each is held to the layer above it.

## Two crates, two jobs

| Crate | Level | Job | Held to |
|---|---|---|---|
| `arare-spec` | QL-A | The Lean model written again in Rust, definition for definition, on `i128` | The Lean model, by a line-by-line diff of a 71,864-line table both print (`formal/Table.lean`, `rust/crates/arare-spec/src/table.rs`), compared in CI (AR-VER-201) |
| `arare-kernel` | QL-A | The implementation the interpreter and, later, the compiler use: widths, codes, scales, truth, columns; `no_std` | `arare-spec`, by exhaustive and enumerated tests (AR-VER-202 to AR-VER-205), MC/DC tables (AR-VER-206) and Kani (AR-VER-207) |

The kernel is written a second way wherever the law allows, so that agreement
with the specification is evidence rather than a copy: DIV rounds on the
truncating quotient where the specification uses the Euclidean one, RESCALE
rounds by shifting where the specification divides, and AND and NOT work on
the truth order's ranks where the specification uses tables.

## Numbers: widths, codes, scales

A `Num` is a value (`Fin(code)`, `Inf`, `Bot`), a width and a scale exponent;
its value is code × 2^e (AR-HLR-001).

- **Codes** (AR-LLR-003): width N has half-range M = 2^(N−1); `0/0` is −M,
  `1/0` is M−1, the finite codes are [−(M−2), M−2], and −(M−1) is not a code.
  M is computed in `i128` because 2^63 does not fit `i64`.
- **Scales** (AR-LLR-004): exponents stay within ±31. Two operands are
  aligned to the smaller exponent by a left shift of at most 62 bits, so an
  aligned int64 code stays below 2^125 and every sum, product and comparison
  is exact in `i128`. An operation whose result exponent would leave ±31 is an
  error in the program (`ScaleOverflow`), not a value. The limit is a derived
  requirement: the high-level requirements do not bound the scale.
- **Widths of results** (AR-LLR-002): the result of ADD or SUB takes the
  narrowest width whose finite range holds the sum of the aligned operand
  ranges; MUL, the product of the ranges; DIV, the dividend's range scaled by
  the divisor's negative exponent. This is the static range inference of
  AR-HLR-003 taken from widths and scales alone; Phase 3 narrows it with
  value ranges. When no width holds the range, the result is int64 and a
  value outside it is `1/0` (AR-HLR-004).
- **DIV** (AR-LLR-005): the quotient keeps the dividend's scale:
  code = round(a·2^(−e_b) / b), half to even. A program wanting more fraction
  bits quantizes the dividend first.
- **RESCALE and NARROW** (AR-LLR-006): RESCALE raises the exponent and rounds
  by shifting, keeping the width; NARROW fits into a narrower width.
- **EQ and LT** (AR-LLR-007): finite numbers compare by value after exact
  alignment; EQ is decided everywhere, LT is UNKNOWN at a point.

## Truth and columns

- **Scalar truth** (AR-LLR-008): AND is the minimum and NOT the reversal of
  the ranks FALSE 0, UNKNOWN 1, TRUE 2; NIL in truth position is UNKNOWN.
- **Columns** (AR-LLR-009): a `TruthColumn` keeps Arrow's two bitmaps,
  validity and values, lane i at bit i % 8 of byte i / 8, least significant
  bit first, padded with zero bits to a multiple of 64 bytes; no value bit is
  set in a NIL lane. AND and NOT work 64 lanes at a time. The words are
  8-byte aligned `u64`s, Arrow's minimum; the recommended 64-byte alignment
  is a Phase 4 concern for the compiled route. Numeric columns, and NIL in
  them, come with the interpreter's vectors in Phase 2b.
