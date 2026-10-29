# Requirements

Generated from `spec/requirements.json` by `scripts/generate-requirements.mjs`; do not edit by hand.
The identifier classes and the requirements standard are `docs/quality/STANDARDS.md`.

## System requirements

### AR-SYS-001 — Stack syntax

Arare programs are sequences of Words on a stack, as in Ajisai, so that one source text runs in both languages and the Playground's stack view, step execution, vocabulary panel and Reference apply unchanged.

- Source: WORK_ORDER.md D-01

### AR-SYS-002 — Fixed-point numbers

Numbers are fixed-width fixed-point values; the default storage widths are int8, int16 and int32. Arare does not represent numbers as residues over primes by default.

- Source: WORK_ORDER.md D-02

### AR-SYS-003 — Primes with cheap reduction

Where Arare computes modulo a prime, the prime is one whose reduction is cheap: Mersenne31, Goldilocks or BabyBear. Such primes serve a wide-precision mode, NTT convolution and Halton dither only.

- Source: WORK_ORDER.md D-03

### AR-SYS-004 — Two execution routes

Arare has a reference interpreter, slow and step-through, and an ahead-of-time compiler to native code and WASM SIMD128. Performance requirements bind the compiled route.

- Source: WORK_ORDER.md D-04

### AR-SYS-005 — Strong Kleene truth

Truth is the strong Kleene logic of SQL, as in Ajisai: TRUE, FALSE and UNKNOWN, where UNKNOWN is NIL read in truth position.

- Source: WORK_ORDER.md D-05

### AR-SYS-006 — Arrow layout

Truth values and NIL are laid out in memory as Apache Arrow lays them out, so that data crosses into Arrow-based SQL engines without a copy.

- Source: WORK_ORDER.md D-06

### AR-SYS-007 — SQL branch on UNKNOWN

A branch whose condition is UNKNOWN takes its ELSE side, as SQL's CASE does. This departs from Ajisai's SELECT on purpose.

- Source: WORK_ORDER.md D-07

### AR-SYS-008 — SQL aggregation

An aggregate skips NIL, as SQL's aggregates do; an aggregate with nothing left to aggregate is NIL.

- Source: WORK_ORDER.md D-08

### AR-SYS-009 — Wheel points

Arare keeps the wheel's two points, 0/0 and 1/0. 1/0 marks a value outside the representable range, so overflow is never silent.

- Source: WORK_ORDER.md D-09

### AR-SYS-010 — Numbers are not absence

0/0 and 1/0 are numbers; NIL is absence. Asking the order of a wheel point is UNKNOWN.

- Source: WORK_ORDER.md D-10

### AR-SYS-011 — Where NIL keeps its reason

The reference interpreter keeps the reason a NIL carries; the compiled route keeps the validity bit only.

- Source: WORK_ORDER.md D-11

### AR-SYS-012 — The field contract

A contract says field=closed exactly when it is statically proved that the code neither overflows nor divides by zero.

- Source: WORK_ORDER.md D-12

### AR-SYS-013 — One Playground shell

The Playground is one shell with an engine adapter; Ajisai runs in it as the oracle beside Arare.

- Source: WORK_ORDER.md D-13

### AR-SYS-014 — Faster than Ajisai

Arare exists to be fast where Ajisai is slow; a build that misses a performance requirement (AR-PERF) is not complete.

- Source: WORK_ORDER.md §1.2

## High-level requirements

### AR-HLR-001 — Widths and scales

A number has a width of 8, 16, 32 or 64 bits and a scale exponent e; its value is its code times 2^e. Widths 8, 16 and 32 are storage widths; 64 is the widest width arithmetic widens into and is never a storage default. A width of N bits has half-range M = 2^(N−1).

- Level: QL-A
- Refines: AR-SYS-002
- Derived: The 64-bit width is not in WORK_ORDER.md D-02; it exists so that widening has a fixed ceiling (ADR-0001, Q-06).
- Proved in `formal/Arare/Codes.lean` (AR-VER-101): `encode_range`
- Refined by: AR-LLR-002
- Planned: Phase 2: width and scale bookkeeping in arare-kernel, by requirements-based tests against arare-spec.

### AR-HLR-002 — Value codes

For a width with half-range M, code −M is 0/0, code M−1 is 1/0, the finite numbers are the codes in [−(M−2), M−2], and code −(M−1) is never produced. Every value has exactly one code and every code but −(M−1) is the code of exactly one value. Negation keeps a finite number inside the finite range.

- Level: QL-A
- Refines: AR-SYS-009, AR-SYS-010
- Proved in `formal/Arare/Codes.lean` (AR-VER-101): `decode_encode`, `encode_range`, `encode_decode`, `neg_valid`
- Refined by: AR-LLR-001, AR-LLR-003

### AR-HLR-003 — Exact integer arithmetic

ADD, SUB, MUL and NEG of finite numbers compute the exact result. Before ADD and SUB the operand with the larger scale exponent is shifted to the smaller one, which is exact; MUL adds the exponents. The result takes the narrowest of the four widths whose finite range holds the result's statically inferred range. A wheel point that enters ADD, SUB, MUL or NEG never comes out as a finite number.

- Level: QL-A
- Refines: AR-SYS-002, AR-SYS-009
- Proved in `formal/Arare/Wheel.lean` (AR-VER-102): `W.fin_add`, `W.fin_sub`, `W.fin_mul`, `W.fin_neg`, `W.add_point_left`, `W.add_point_right`, `W.mul_point_left`, `W.mul_point_right`, `W.neg_point`
- Proved in `formal/Arare/Closed.lean` (AR-VER-105): `Expr.closed_evalW`
- Refined by: AR-LLR-001, AR-LLR-002, AR-LLR-004, AR-LLR-015
- Planned: Phase 2: scale alignment and width selection, by tests against arare-spec; the static range inference behind width selection is Phase 3.

### AR-HLR-004 — Overflow is 1/0

An arithmetic result outside the finite range of the widest width is 1/0. No arithmetic result wraps around: whatever a machine answers is either the exact value or a wheel point. Associativity of addition holds for the exact values but not across an intermediate overflow.

- Level: QL-A
- Refines: AR-SYS-009
- Proved in `formal/Arare/Codes.lean` (AR-VER-101): `fit_valid`, `fit_exact_or_inf`
- Proved in `formal/Arare/Closed.lean` (AR-VER-105): `Expr.closed_evalM_of_fits`, `Expr.closed_evalM_sound`, `Expr.machine_add_not_assoc`
- Refined by: AR-LLR-001, AR-LLR-003, AR-LLR-004

### AR-HLR-005 — The wheel points under addition

0/0 plus anything is 0/0; 1/0 plus a finite number is 1/0; 1/0 plus 1/0 is 0/0; 1/0 is its own negation. Addition is commutative and associative with identity 0 over the exact values, wheel points included. x − x is 0 for every finite x but not for 1/0, and an implementation does not restore it by special case.

- Level: QL-A
- Refines: AR-SYS-009
- Proved in `formal/Arare/Wheel.lean` (AR-VER-102): `W.add_comm`, `W.add_assoc`, `W.add_zero`, `W.bot_add`, `W.neg_neg`, `W.inf_sub_inf`, `W.not_sub_self`
- Refined by: AR-LLR-001, AR-LLR-004

### AR-HLR-006 — The wheel points under multiplication

0/0 times anything is 0/0; 1/0 times a nonzero finite number or 1/0 is 1/0; 1/0 times 0 is 0/0. Multiplication is commutative and associative with identity 1, and the exact values satisfy the wheel axioms that do not mention the reciprocal: (x+y)z + 0z = xz + yz, 0·0 = 0, (x + 0y)z = xz + 0y. 0·x is 0 for every finite x but not for 1/0.

- Level: QL-A
- Refines: AR-SYS-009
- Proved in `formal/Arare/Wheel.lean` (AR-VER-102): `W.mul_comm`, `W.mul_assoc`, `W.mul_one`, `W.wheel_distrib`, `W.zero_mul_zero`, `W.wheel_zero_term`, `W.bot_mul`, `W.zero_mul_inf`, `W.not_mul_zero`
- Refined by: AR-LLR-001, AR-LLR-004

### AR-HLR-007 — Division

DIV of a nonzero finite number by 0 is 1/0 and of 0 by 0 is 0/0; a finite number over 1/0 is 0; 1/0 over 1/0 is 0/0; 1/0 over anything else is 1/0; 0/0 absorbs. DIV of two finite numbers, the divisor nonzero, is the quotient at the result's scale rounded half to even, within half an ulp of the exact quotient. DIV is therefore a rounding point.

- Level: QL-A
- Refines: AR-SYS-009
- Derived: WORK_ORDER.md §3.3 named RESCALE and NARROW as the only rounding points; a fixed-point quotient is in general not on the grid, so DIV must round too (ADR-0002).
- Proved in `formal/Arare/Wheel.lean` (AR-VER-102): `W.div_by_zero`, `W.zero_div_zero`, `W.fin_div_inf`, `W.inf_div_inf`, `W.div_error`
- Proved in `formal/Arare/Rounding.lean` (AR-VER-103): `roundDiv_error`
- Refined by: AR-LLR-001, AR-LLR-005, AR-LLR-015
- Planned: Phase 2: how the result scale is chosen is fixed with the surface syntax (AR-HLR-018) and tested then.

### AR-HLR-008 — RESCALE

RESCALE by s ≥ 0 bits raises a number's scale exponent by s and rounds its code half to even: the error is at most half an ulp of the new scale, a tie goes to the even code, a code already on the new grid is unchanged, and s = 0 changes nothing. The wheel points pass through unchanged.

- Level: QL-A
- Refines: AR-SYS-002
- Proved in `formal/Arare/Rounding.lean` (AR-VER-103): `rescale_error`, `rescale_zero`, `roundDivPos_error`, `roundDivPos_tie_even`, `roundDivPos_exact`, `two_pow_pos_int`
- Refined by: AR-LLR-001, AR-LLR-006, AR-LLR-015

### AR-HLR-009 — NARROW

NARROW to a width keeps a finite number inside that width's finite range and turns any other finite number into 1/0; the wheel points pass through unchanged. NARROW twice is NARROW once.

- Level: QL-A
- Refines: AR-SYS-009
- Proved in `formal/Arare/Codes.lean` (AR-VER-101): `fit_of_valid`, `fit_valid`, `fit_exact_or_inf`, `fit_idem`
- Refined by: AR-LLR-001, AR-LLR-003, AR-LLR-006, AR-LLR-015

### AR-HLR-010 — Equality

EQ compares what numbers denote: two finite numbers are equal when their values are equal, whatever their widths and scales; 1/0 equals 1/0; 0/0 equals 0/0; a wheel point never equals a finite number. EQ always answers TRUE or FALSE.

- Level: QL-A
- Refines: AR-SYS-010
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `eqT_bot`, `eqT_inf`, `eqT_decided`
- Refined by: AR-LLR-001, AR-LLR-007, AR-LLR-015
- Planned: Phase 2: equality across widths and scales, by tests against arare-spec.

### AR-HLR-011 — Order

LT and GT order finite numbers by value and always answer TRUE or FALSE for them; when either operand is a wheel point they answer UNKNOWN. MIN, MAX and SORT, asked to order a wheel point, answer NIL.

- Level: QL-A
- Refines: AR-SYS-009, AR-SYS-010
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `ltT_point_left`, `ltT_point_right`, `ltT_fin_decided`
- Refined by: AR-LLR-001, AR-LLR-007, AR-LLR-015
- Planned: Phase 2: MIN, MAX and SORT on wheel points, by tests.

### AR-HLR-012 — Strong Kleene connectives

AND is the meet and OR the join of the truth order FALSE < UNKNOWN < TRUE, and NOT reverses it. AND and OR are commutative, associative and idempotent, absorb and distribute over each other, and satisfy De Morgan's laws and Kleene's law a ∧ ¬a ≤ b ∨ ¬b; excluded middle fails exactly at UNKNOWN.

- Level: QL-A
- Refines: AR-SYS-005
- Proved in `formal/Arare/Truth.lean` (AR-VER-104): `Truth.and_rank`, `Truth.or_rank`, `Truth.not_rank`, `Truth.and_comm`, `Truth.and_assoc`, `Truth.and_idem`, `Truth.and_true`, `Truth.and_false`, `Truth.or_comm`, `Truth.or_assoc`, `Truth.not_not`, `Truth.de_morgan_and`, `Truth.absorb`, `Truth.distrib`, `Truth.kleene`, `Truth.excluded_middle_fails`
- Refined by: AR-LLR-001, AR-LLR-008, AR-LLR-009, AR-LLR-014

### AR-HLR-013 — UNKNOWN is NIL in truth position

A NIL read where a truth value is expected is UNKNOWN, and it is still that NIL: it keeps its reason where reasons are kept (AR-HLR-020).

- Level: QL-A
- Refines: AR-SYS-005, AR-SYS-010
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `truthOf_nil`
- Refined by: AR-LLR-001, AR-LLR-008, AR-LLR-014, AR-LLR-016

### AR-HLR-014 — FILTER

FILTER keeps exactly the elements its predicate calls TRUE; an element whose predicate is FALSE, UNKNOWN or NIL is dropped, as SQL's WHERE drops a row.

- Level: QL-A
- Refines: AR-SYS-005
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `mem_filterT`
- Refined by: AR-LLR-001, AR-LLR-015

### AR-HLR-015 — Branching on UNKNOWN

A branch takes its THEN side only when its condition is TRUE; on FALSE, UNKNOWN or NIL it takes its ELSE side.

- Level: QL-A
- Refines: AR-SYS-007
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `selectT_true`, `selectT_false`, `selectT_unknown`, `selectT_nil`
- Refined by: AR-LLR-001, AR-LLR-015

### AR-HLR-016 — Aggregates skip NIL

SUM, and every aggregate, skips the NIL elements of its input; an aggregate whose input holds no element that is not NIL is NIL.

- Level: QL-A
- Refines: AR-SYS-008
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `sumT_all_nil`, `sumT_skip_nil`, `sumT_two`
- Refined by: AR-LLR-001, AR-LLR-015
- Planned: Phase 2: the other aggregates, by tests.

### AR-HLR-017 — What field=closed guarantees

Code that calls neither DIV nor NARROW and writes no wheel point computes the exact integer its arithmetic denotes; run by a machine it answers that integer or a wheel point, never another finite number. When it is moreover proved that every intermediate result fits the widest width, its contract is field=closed and the machine answers the exact integer.

- Level: QL-A
- Refines: AR-SYS-012, AR-SYS-009
- Proved in `formal/Arare/Closed.lean` (AR-VER-105): `Expr.closed_evalW`, `Expr.closed_evalM_of_fits`, `Expr.closed_evalM_sound`
- Planned: Phase 3: soundness of the static range inference that establishes the fit, and the error budget beyond it.

### AR-HLR-018 — Where widths and scales are written

A number's width and scale are fixed where it enters a program — by the form of its literal or by QUANTIZE — and are inferred everywhere else. No other place in a program states a width or a scale.

- Level: QL-B
- Refines: AR-SYS-001, AR-SYS-002
- Derived: Decides Q-01 (ADR-0004): Ajisai has nowhere to write a type, and Arare keeps that, except where a value is created.
- Refined by: AR-LLR-010, AR-LLR-011, AR-LLR-012
- Planned: Verified through AR-LLR-011 and AR-LLR-012 (AR-VER-210).

### AR-HLR-019 — Arrow layout of truth and NIL

A vector whose lanes may be NIL carries a validity bitmap in which a cleared bit marks a NIL lane; a vector of truth values carries, besides it, a value bitmap. UNKNOWN is a lane whose validity bit is cleared. Bit order, alignment and padding are Apache Arrow's.

- Level: QL-A
- Refines: AR-SYS-006
- Refined by: AR-LLR-009
- Not proved formally: A memory layout is not a law about values; it is verified by tests against the Arrow specification, not by a Lean theorem.
- Planned: Phase 2: kernel tests of the bitmaps; Phase 6: a round trip through the Arrow C Data Interface.

### AR-HLR-020 — Reasons for NIL

The reference interpreter keeps, with each NIL, the reason it arose; the compiled route keeps only the validity bit. A reason never changes a value or a truth value.

- Level: QL-B
- Refines: AR-SYS-011
- Refined by: AR-LLR-016
- Planned: Verified through AR-LLR-016 (AR-VER-211); Phase 4: the compiled route drops reasons and nothing else, by translation validation.

### AR-HLR-021 — Program structure

A program is a sequence of integer literals, quantized literals, Words and bracketed quotes; `#` begins a comment that runs to the end of the line. A quote is code to a Word that runs a block (MAP, FILTER, FOLD) and data to every other Word, which evaluates it, on a stack of its own, into the vector of what it leaves.

- Level: QL-B
- Refines: AR-SYS-001
- Refined by: AR-LLR-011, AR-LLR-012, AR-LLR-013, AR-LLR-017
- Planned: Verified through AR-LLR-011, AR-LLR-012 and AR-LLR-013.

### AR-HLR-022 — Lane-wise lifting

ADD, SUB, MUL, DIV, NEG, LT, AND, NOT, RESCALE, NARROW, QUANTIZE and SELECT apply lane by lane to vectors: two vectors of equal length pair their lanes, and a one-element vector or a scalar pairs with every lane of the other; any other pairing is an ERROR. EQ compares whole values.

- Level: QL-B
- Refines: AR-SYS-001
- Refined by: AR-LLR-014, AR-LLR-015
- Planned: Verified through AR-LLR-014 and AR-LLR-015.

### AR-HLR-023 — NIL passes through

A NIL operand of an arithmetic or order Word, or the data operand of MAP, FILTER, FOLD, SUM or EQ, is the result, unread and with its reason; of two NIL operands, the left one is. A truth operand reads a NIL as UNKNOWN instead (AR-HLR-013).

- Level: QL-B
- Refines: AR-SYS-010, AR-SYS-005
- Refined by: AR-LLR-014
- Planned: Verified through AR-LLR-014 and AR-LLR-015.

### AR-HLR-024 — Blocks

MAP replaces each element with what its block leaves for it; FILTER keeps the elements its block answers TRUE for (AR-HLR-014); FOLD threads an accumulator through its block, element by element from the left. A block runs on a stack holding only its inputs and leaves exactly one value.

- Level: QL-B
- Refines: AR-SYS-001
- Refined by: AR-LLR-013, AR-LLR-015
- Planned: Verified through AR-LLR-015.

### AR-HLR-025 — Malformed use is an ERROR

Malformed use stops the program with an ERROR that names the Word: too few operands, an operand of the wrong kind, vectors that do not pair, a result exponent outside the representable range, a width, exponent or shift that QUANTIZE, NARROW or RESCALE does not accept, and a block that does not leave one value. Before anything runs, the front end refuses an unknown Word, an unbalanced bracket, an integer literal outside the widest range, and a non-integer literal not quantized where it is written. ERROR is never a value; NIL is.

- Level: QL-B
- Refines: AR-SYS-001, AR-SYS-010
- Refined by: AR-LLR-012, AR-LLR-013, AR-LLR-015, AR-LLR-017
- Planned: Verified through AR-LLR-012, AR-LLR-013 and AR-LLR-015.

### AR-HLR-026 — QUANTIZE

`x w e QUANTIZE` answers x at width w ∈ {8, 16, 32, 64} and exponent e: raising the exponent rounds half to even, within half an ulp of the new scale; lowering it is exact; the result is fitted into the width, and is 1/0 outside it. A non-integer literal p/q quantized where it is written is round(p·2^(−e)/q), half to even, fitted into w; a zero denominator gives a wheel point.

- Level: QL-A
- Refines: AR-SYS-002, AR-SYS-009
- Proved in `formal/Arare/Rounding.lean` (AR-VER-103): `roundDiv_error`, `rescale_error`
- Proved in `formal/Arare/Codes.lean` (AR-VER-101): `fit_exact_or_inf`
- Refined by: AR-LLR-010, AR-LLR-012, AR-LLR-015

## Low-level requirements

### AR-LLR-001 — The executable specification mirrors the Lean model

rust/crates/arare-spec defines every function of formal/Arare/ that has computational content, under the same name in snake case, on i128; over the grid formal/Table.lean prints, its output is byte for byte the Lean model's.

- Level: QL-A
- Refines: AR-HLR-002, AR-HLR-003, AR-HLR-004, AR-HLR-005, AR-HLR-006, AR-HLR-007, AR-HLR-008, AR-HLR-009, AR-HLR-010, AR-HLR-011, AR-HLR-012, AR-HLR-013, AR-HLR-014, AR-HLR-015, AR-HLR-016
- Implemented in: `rust/crates/arare-spec/src/lib.rs`, `rust/crates/arare-spec/src/rounding.rs`, `rust/crates/arare-spec/src/wheel.rs`, `rust/crates/arare-spec/src/truth.rs`, `rust/crates/arare-spec/src/sql.rs`, `rust/crates/arare-spec/src/table.rs`
- Tested in `rust/crates/arare-spec/tests/table.rs` — **AR-VER-201**
- Tested in `formal/Table.lean` — **AR-VER-201**

### AR-LLR-002 — Widths and the width of a result

Width gives each of 8, 16, 32 and 64 bits its half-range M, its largest finite code M−2, and the codes of 1/0 (M−1) and 0/0 (−M). An integer literal takes the narrowest width holding it. The result of ADD or SUB takes the narrowest width whose finite range holds the sum of the operands' ranges after alignment; MUL, their product; DIV, the dividend's range shifted by the divisor's negative exponent; none holding it, int64.

- Level: QL-A
- Refines: AR-HLR-001, AR-HLR-003
- Implemented in: `rust/crates/arare-kernel/src/width.rs`
- Tested in `rust/crates/arare-kernel/tests/int8_exhaustive.rs` — **AR-VER-202**
- Tested in `rust/crates/arare-kernel/tests/scales.rs` — **AR-VER-203**
- Tested in `rust/crates/arare-kernel/tests/widths.rs` — **AR-VER-204**

### AR-LLR-003 — Codes and fitting

encode, decode and is_code realise AR-HLR-002 at each width; fit maps an exact integer to itself inside a width's finite range and to 1/0 outside it, and fit_val passes the wheel points through.

- Level: QL-A
- Refines: AR-HLR-002, AR-HLR-004, AR-HLR-009
- Implemented in: `rust/crates/arare-kernel/src/codes.rs`
- Tested in `rust/crates/arare-kernel/tests/widths.rs` — **AR-VER-204**
- Tested in `rust/crates/arare-kernel/src/tests.rs` — **AR-VER-206**
- Tested in `rust/crates/arare-kernel/src/proofs.rs` — **AR-VER-207** (Kani): `fit_is_exact_or_inf`, `codes_round_trip`

### AR-LLR-004 — ADD, SUB, NEG and MUL with scales

Scale exponents stay within ±31; an operation whose result exponent would leave that range is a ScaleOverflow error. ADD aligns both operands to the smaller exponent by an exact shift and adds; SUB adds the negation; NEG keeps width and scale; MUL adds the exponents and multiplies. Every intermediate is exact in i128, and the result is fitted into its width (AR-LLR-002). The wheel points follow AR-HLR-005 and AR-HLR-006.

- Level: QL-A
- Refines: AR-HLR-003, AR-HLR-004, AR-HLR-005, AR-HLR-006
- Derived: The ±31 exponent limit is not in the high-level requirements; it keeps every alignment shift within 62 bits, so that all arithmetic is exact in i128 (docs/design/kernel.md).
- Implemented in: `rust/crates/arare-kernel/src/num.rs`
- Tested in `rust/crates/arare-kernel/tests/int8_exhaustive.rs` — **AR-VER-202**
- Tested in `rust/crates/arare-kernel/tests/scales.rs` — **AR-VER-203**
- Tested in `rust/crates/arare-kernel/tests/widths.rs` — **AR-VER-204**
- Tested in `rust/crates/arare-kernel/src/tests.rs` — **AR-VER-206**
- Tested in `rust/crates/arare-kernel/src/proofs.rs` — **AR-VER-207** (Kani): `add_exact_w16_w32`, `add_w64_any_scales_no_overflow`, `mul_exact_w16`

### AR-LLR-005 — DIV at the dividend's scale

DIV of two finite numbers, the divisor nonzero, keeps the dividend's exponent e_a and answers round(a·2^(−e_b) / b) half to even, computed on the truncating quotient; division by zero and by 1/0 follows AR-HLR-007.

- Level: QL-A
- Refines: AR-HLR-007
- Derived: AR-HLR-007 leaves the result scale to the surface syntax; keeping the dividend's scale makes DIV need no extra operand, and a program wanting more fraction bits quantizes the dividend first (docs/design/kernel.md).
- Implemented in: `rust/crates/arare-kernel/src/num.rs`
- Tested in `rust/crates/arare-kernel/tests/int8_exhaustive.rs` — **AR-VER-202**
- Tested in `rust/crates/arare-kernel/tests/scales.rs` — **AR-VER-203**
- Tested in `rust/crates/arare-kernel/src/tests.rs` — **AR-VER-206**
- Tested in `rust/crates/arare-kernel/src/proofs.rs` — **AR-VER-207** (Kani): `div_half_unit_w16`

### AR-LLR-006 — RESCALE and NARROW

RESCALE by s raises the exponent by s and rounds the code half to even by shifting, keeping the width; NARROW to a width fits the value into it and keeps the scale. The wheel points pass through both.

- Level: QL-A
- Refines: AR-HLR-008, AR-HLR-009
- Implemented in: `rust/crates/arare-kernel/src/num.rs`
- Tested in `rust/crates/arare-kernel/tests/int8_exhaustive.rs` — **AR-VER-202**
- Tested in `rust/crates/arare-kernel/tests/widths.rs` — **AR-VER-204**
- Tested in `rust/crates/arare-kernel/src/tests.rs` — **AR-VER-206**
- Tested in `rust/crates/arare-kernel/src/proofs.rs` — **AR-VER-207** (Kani): `rescale_is_round_half_even_w32`

### AR-LLR-007 — EQ and LT across scales

EQ and LT compare two finite numbers by value after exact alignment to the smaller exponent; EQ compares wheel points by identity and is always TRUE or FALSE; LT with a wheel point is UNKNOWN.

- Level: QL-A
- Refines: AR-HLR-010, AR-HLR-011
- Implemented in: `rust/crates/arare-kernel/src/num.rs`
- Tested in `rust/crates/arare-kernel/tests/int8_exhaustive.rs` — **AR-VER-202**
- Tested in `rust/crates/arare-kernel/tests/scales.rs` — **AR-VER-203**

### AR-LLR-008 — Scalar truth

Truth's AND is the minimum and NOT the reversal of the ranks FALSE 0, UNKNOWN 1, TRUE 2; OR is NOT of the AND of the NOTs; Truth::of reads NIL as UNKNOWN.

- Level: QL-A
- Refines: AR-HLR-012, AR-HLR-013
- Implemented in: `rust/crates/arare-kernel/src/truth.rs`
- Tested in `rust/crates/arare-kernel/tests/truth.rs` — **AR-VER-205**

### AR-LLR-009 — Truth columns in Arrow's layout

A TruthColumn holds a validity and a value bitmap, lane i at bit i % 8 of byte i / 8, least significant bit first, each padded with zero bits to a multiple of 64 bytes, with no value bit set in a NIL lane. AND and NOT act 64 lanes at a time and agree with the scalar connectives on every lane.

- Level: QL-A
- Refines: AR-HLR-019, AR-HLR-012
- Implemented in: `rust/crates/arare-kernel/src/column.rs`
- Tested in `rust/crates/arare-kernel/tests/truth.rs` — **AR-VER-205**
- Tested in `rust/crates/arare-kernel/src/tests.rs` — **AR-VER-206**
- Tested in `rust/crates/arare-kernel/src/proofs.rs` — **AR-VER-207** (Kani): `and_word_is_lanewise_and`, `not_word_is_lanewise_not`

### AR-LLR-010 — Quantization in the kernel

Num::from_ratio quantizes an exact rational p/q (|p|, q < 2^63) at a width and exponent as round(p·2^(−e)/q) half to even on the truncating quotient, fitted into the width, with q = 0 giving 1/0 or, for p = 0, 0/0. Num::quantize moves a number to a width and exponent: up by RESCALE's rounding, down by an exact shift, then a fit. Num::exact reads a number back as the rational it denotes in lowest terms, 1/0 as (1, 0) and 0/0 as (0, 0).

- Level: QL-A
- Refines: AR-HLR-026, AR-HLR-018
- Implemented in: `rust/crates/arare-kernel/src/quantize.rs`
- Tested in `rust/crates/arare-kernel/tests/quantize.rs` — **AR-VER-208**
- Tested in `tests/oracle/corpus.json` — **AR-VER-209**

### AR-LLR-011 — Lexical grammar

Whitespace separates tokens and `[` and `]` are tokens of their own; `#` at a token's start comments out the rest of the line. A token is an integer (`-?digit+`), a decimal (`-?digit+.digit+`), a fraction (`-?digit+/digit+`) or otherwise a name; every token carries its 1-based line and column.

- Level: QL-B
- Refines: AR-HLR-021, AR-HLR-018
- Implemented in: `rust/crates/arare-front/src/lexer.rs`
- Tested in `rust/crates/arare-front/tests/front.rs` — **AR-VER-210**

### AR-LLR-012 — Parsing and the static checks

The parser nests quotes by brackets, maps names to the twenty Words of the Phase 2 vocabulary, keeps an integer literal within the widest finite range, and folds every decimal, fraction or wheel point with the integer width (8, 16, 32 or 64), integer exponent (within ±31) and QUANTIZE that must follow it into one number, reduced to lowest terms within 64 bits. Anything else is refused with the token's position.

- Level: QL-B
- Refines: AR-HLR-018, AR-HLR-021, AR-HLR-025, AR-HLR-026
- Implemented in: `rust/crates/arare-front/src/lib.rs`, `rust/crates/arare-front/src/parser.rs`
- Tested in `rust/crates/arare-front/tests/front.rs` — **AR-VER-210**

### AR-LLR-013 — The machine

A Machine holds a stack and a program and executes one top-level item per step: a literal pushes a number, a quote pushes itself, a Word pops its operands in the order they were pushed and pushes its result. A quote consumed as data is evaluated on a stack of its own, nested quotes included; a block runs on a stack holding only its inputs and must leave one value. An ERROR stops the machine and names the Word.

- Level: QL-B
- Refines: AR-HLR-021, AR-HLR-024, AR-HLR-025
- Implemented in: `rust/crates/arare-interp/src/lib.rs`, `rust/crates/arare-interp/src/machine.rs`
- Tested in `rust/crates/arare-interp/tests/words.rs` — **AR-VER-211**
- Tested in `rust/crates/arare-interp/tests/robustness.rs` — **AR-VER-212**

### AR-LLR-014 — Lifting

lift2 pairs two vectors lane by lane when their lengths are equal, reuses the lane of a one-element vector or a scalar, and refuses other pairings with their two lengths; a NIL operand is the result, the left of two. AND and NOT lift the same way but read NIL as UNKNOWN, and an UNKNOWN result is the NIL it came from, the left one's reason when both are NIL.

- Level: QL-B
- Refines: AR-HLR-022, AR-HLR-023, AR-HLR-012, AR-HLR-013
- Implemented in: `rust/crates/arare-interp/src/lift.rs`
- Tested in `rust/crates/arare-interp/tests/words.rs` — **AR-VER-211**
- Tested in `rust/crates/arare-interp/src/tests.rs` — **AR-VER-213**

### AR-LLR-015 — The Words

Each Word of the Phase 2 vocabulary does what AR-HLR-003 to AR-HLR-016 and AR-HLR-021 to AR-HLR-026 say through arare-kernel: arithmetic, NEG, LT, RESCALE, NARROW and QUANTIZE lifted; EQ on whole values; AND and NOT in strong Kleene logic; SELECT lane by lane with THEN only on TRUE; SUM skipping NIL; MAP, FILTER and FOLD over a block; TRUE, FALSE and NIL as literals. RESCALE's shift is 0 to 62, NARROW's and QUANTIZE's width 8, 16, 32 or 64, QUANTIZE's exponent within ±31, each an integer at any scale.

- Level: QL-B
- Refines: AR-HLR-003, AR-HLR-007, AR-HLR-008, AR-HLR-009, AR-HLR-010, AR-HLR-011, AR-HLR-014, AR-HLR-015, AR-HLR-016, AR-HLR-022, AR-HLR-024, AR-HLR-025, AR-HLR-026
- Implemented in: `rust/crates/arare-interp/src/words.rs`
- Tested in `rust/crates/arare-interp/tests/words.rs` — **AR-VER-211**
- Tested in `rust/crates/arare-interp/tests/robustness.rs` — **AR-VER-212**
- Tested in `tests/oracle/corpus.json` — **AR-VER-209**

### AR-LLR-016 — NIL reasons in the interpreter

A NIL carries one of the reasons Literal (written NIL), OrderOfPoint (an order asked of a wheel point) and EmptyAggregate (an aggregate with nothing to aggregate), and keeps it through every Word that passes it on or reads it as UNKNOWN.

- Level: QL-B
- Refines: AR-HLR-020, AR-HLR-013
- Implemented in: `rust/crates/arare-interp/src/value.rs`
- Tested in `rust/crates/arare-interp/tests/words.rs` — **AR-VER-211**
- Tested in `rust/crates/arare-interp/src/tests.rs` — **AR-VER-213**

### AR-LLR-017 — The command-line interface

`arare run [--json] [FILE | -]` runs a program with the reference interpreter and prints the final stack, one value per line as exact rationals; with --json, one document whose value nodes are Ajisai's (type, and a number's exact numerator and denominator as strings) plus Arare's width and scale. It exits 0 when the program ran, 1 when it was refused or stopped on an ERROR, 2 on misuse. `arare version` prints the version.

- Level: QL-C
- Refines: AR-HLR-021, AR-HLR-025
- Implemented in: `rust/crates/arare-cli/src/main.rs`
- Tested in `rust/crates/arare-cli/tests/cli.rs` — **AR-VER-214**
- Tested in `tests/oracle/corpus.json` — **AR-VER-209**

## Performance requirements

### AR-PERF-001 — int8 dot product

The compiled int8 dot product over 1,000,000 elements takes at most 1.5 times a hand-written Rust int8 loop, and is at least 20 times faster than Ajisai's native build on the same case.

- Level: QL-A
- Refines: AR-SYS-014, AR-SYS-004
- Planned: Phase 4: the benchmark of WORK_ORDER.md §6, against a baseline loop measured in the same run, blocking in CI.

### AR-PERF-002 — Fixed-point dot product

The compiled fixed-point counterpart of Ajisai's small-rational dot product is at least 300 times faster than Ajisai.

- Level: QL-A
- Refines: AR-SYS-014
- Planned: Phase 4: the benchmark of WORK_ORDER.md §6, against a baseline loop measured in the same run, blocking in CI.

### AR-PERF-003 — Logistic map

Twenty steps of the logistic map take at most 1 µs, with the error against Ajisai's exact result inside the inferred budget.

- Level: QL-A
- Refines: AR-SYS-014
- Planned: Phase 4: the benchmark of WORK_ORDER.md §6, against a baseline loop measured in the same run, blocking in CI.

### AR-PERF-004 — Harmonic sum

The harmonic sum to k = 20,000 is at least 1,000 times faster than Ajisai.

- Level: QL-A
- Refines: AR-SYS-014
- Planned: Phase 4: the benchmark of WORK_ORDER.md §6, against a baseline loop measured in the same run, blocking in CI.

### AR-PERF-005 — Top-level scalar Words

A top-level scalar Word takes at most 2 ns on the compiled route.

- Level: QL-A
- Refines: AR-SYS-014, AR-SYS-004
- Planned: Phase 4: the benchmark of WORK_ORDER.md §6, against a baseline loop measured in the same run, blocking in CI.

### AR-PERF-006 — Cost of wheel points and NIL

Carrying the wheel-point codes and NIL validity costs at most 1.1 times the same kernel without them, measured on a memory-bound and a compute-bound kernel.

- Level: QL-A
- Refines: AR-SYS-014, AR-SYS-009
- Planned: Phase 4: the benchmark of WORK_ORDER.md §6, against a baseline loop measured in the same run, blocking in CI.

### AR-PERF-007 — WebAssembly

The WASM SIMD128 build is within 3 times the native build. The figure is a target to be re-measured in Phase 4.

- Level: QL-A
- Refines: AR-SYS-014, AR-SYS-004
- Planned: Phase 4: the benchmark of WORK_ORDER.md §6, against a baseline loop measured in the same run, blocking in CI.
