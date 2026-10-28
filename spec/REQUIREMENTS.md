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
- Planned: Phase 2: width and scale bookkeeping in arare-kernel, by requirements-based tests against arare-spec.

### AR-HLR-002 — Value codes

For a width with half-range M, code −M is 0/0, code M−1 is 1/0, the finite numbers are the codes in [−(M−2), M−2], and code −(M−1) is never produced. Every value has exactly one code and every code but −(M−1) is the code of exactly one value. Negation keeps a finite number inside the finite range.

- Level: QL-A
- Refines: AR-SYS-009, AR-SYS-010
- Proved in `formal/Arare/Codes.lean` (AR-VER-101): `decode_encode`, `encode_range`, `encode_decode`, `neg_valid`

### AR-HLR-003 — Exact integer arithmetic

ADD, SUB, MUL and NEG of finite numbers compute the exact result. Before ADD and SUB the operand with the larger scale exponent is shifted to the smaller one, which is exact; MUL adds the exponents. The result takes the narrowest of the four widths whose finite range holds the result's statically inferred range. A wheel point that enters ADD, SUB, MUL or NEG never comes out as a finite number.

- Level: QL-A
- Refines: AR-SYS-002, AR-SYS-009
- Proved in `formal/Arare/Wheel.lean` (AR-VER-102): `W.fin_add`, `W.fin_sub`, `W.fin_mul`, `W.fin_neg`, `W.add_point_left`, `W.add_point_right`, `W.mul_point_left`, `W.mul_point_right`, `W.neg_point`
- Proved in `formal/Arare/Closed.lean` (AR-VER-105): `Expr.closed_evalW`
- Planned: Phase 2: scale alignment and width selection, by tests against arare-spec; the static range inference behind width selection is Phase 3.

### AR-HLR-004 — Overflow is 1/0

An arithmetic result outside the finite range of the widest width is 1/0. No arithmetic result wraps around: whatever a machine answers is either the exact value or a wheel point. Associativity of addition holds for the exact values but not across an intermediate overflow.

- Level: QL-A
- Refines: AR-SYS-009
- Proved in `formal/Arare/Codes.lean` (AR-VER-101): `fit_valid`, `fit_exact_or_inf`
- Proved in `formal/Arare/Closed.lean` (AR-VER-105): `Expr.closed_evalM_of_fits`, `Expr.closed_evalM_sound`, `Expr.machine_add_not_assoc`

### AR-HLR-005 — The wheel points under addition

0/0 plus anything is 0/0; 1/0 plus a finite number is 1/0; 1/0 plus 1/0 is 0/0; 1/0 is its own negation. Addition is commutative and associative with identity 0 over the exact values, wheel points included. x − x is 0 for every finite x but not for 1/0, and an implementation does not restore it by special case.

- Level: QL-A
- Refines: AR-SYS-009
- Proved in `formal/Arare/Wheel.lean` (AR-VER-102): `W.add_comm`, `W.add_assoc`, `W.add_zero`, `W.bot_add`, `W.neg_neg`, `W.inf_sub_inf`, `W.not_sub_self`

### AR-HLR-006 — The wheel points under multiplication

0/0 times anything is 0/0; 1/0 times a nonzero finite number or 1/0 is 1/0; 1/0 times 0 is 0/0. Multiplication is commutative and associative with identity 1, and the exact values satisfy the wheel axioms that do not mention the reciprocal: (x+y)z + 0z = xz + yz, 0·0 = 0, (x + 0y)z = xz + 0y. 0·x is 0 for every finite x but not for 1/0.

- Level: QL-A
- Refines: AR-SYS-009
- Proved in `formal/Arare/Wheel.lean` (AR-VER-102): `W.mul_comm`, `W.mul_assoc`, `W.mul_one`, `W.wheel_distrib`, `W.zero_mul_zero`, `W.wheel_zero_term`, `W.bot_mul`, `W.zero_mul_inf`, `W.not_mul_zero`

### AR-HLR-007 — Division

DIV of a nonzero finite number by 0 is 1/0 and of 0 by 0 is 0/0; a finite number over 1/0 is 0; 1/0 over 1/0 is 0/0; 1/0 over anything else is 1/0; 0/0 absorbs. DIV of two finite numbers, the divisor nonzero, is the quotient at the result's scale rounded half to even, within half an ulp of the exact quotient. DIV is therefore a rounding point.

- Level: QL-A
- Refines: AR-SYS-009
- Derived: WORK_ORDER.md §3.3 named RESCALE and NARROW as the only rounding points; a fixed-point quotient is in general not on the grid, so DIV must round too (ADR-0002).
- Proved in `formal/Arare/Wheel.lean` (AR-VER-102): `W.div_by_zero`, `W.zero_div_zero`, `W.fin_div_inf`, `W.inf_div_inf`, `W.div_error`
- Proved in `formal/Arare/Rounding.lean` (AR-VER-103): `roundDiv_error`
- Planned: Phase 2: how the result scale is chosen is fixed with the surface syntax (AR-HLR-018) and tested then.

### AR-HLR-008 — RESCALE

RESCALE by s ≥ 0 bits raises a number's scale exponent by s and rounds its code half to even: the error is at most half an ulp of the new scale, a tie goes to the even code, a code already on the new grid is unchanged, and s = 0 changes nothing. The wheel points pass through unchanged.

- Level: QL-A
- Refines: AR-SYS-002
- Proved in `formal/Arare/Rounding.lean` (AR-VER-103): `rescale_error`, `rescale_zero`, `roundDivPos_error`, `roundDivPos_tie_even`, `roundDivPos_exact`, `two_pow_pos_int`

### AR-HLR-009 — NARROW

NARROW to a width keeps a finite number inside that width's finite range and turns any other finite number into 1/0; the wheel points pass through unchanged. NARROW twice is NARROW once.

- Level: QL-A
- Refines: AR-SYS-009
- Proved in `formal/Arare/Codes.lean` (AR-VER-101): `fit_of_valid`, `fit_valid`, `fit_exact_or_inf`, `fit_idem`

### AR-HLR-010 — Equality

EQ compares what numbers denote: two finite numbers are equal when their values are equal, whatever their widths and scales; 1/0 equals 1/0; 0/0 equals 0/0; a wheel point never equals a finite number. EQ always answers TRUE or FALSE.

- Level: QL-A
- Refines: AR-SYS-010
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `eqT_bot`, `eqT_inf`, `eqT_decided`
- Planned: Phase 2: equality across widths and scales, by tests against arare-spec.

### AR-HLR-011 — Order

LT and GT order finite numbers by value and always answer TRUE or FALSE for them; when either operand is a wheel point they answer UNKNOWN. MIN, MAX and SORT, asked to order a wheel point, answer NIL.

- Level: QL-A
- Refines: AR-SYS-009, AR-SYS-010
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `ltT_point_left`, `ltT_point_right`, `ltT_fin_decided`
- Planned: Phase 2: MIN, MAX and SORT on wheel points, by tests.

### AR-HLR-012 — Strong Kleene connectives

AND is the meet and OR the join of the truth order FALSE < UNKNOWN < TRUE, and NOT reverses it. AND and OR are commutative, associative and idempotent, absorb and distribute over each other, and satisfy De Morgan's laws and Kleene's law a ∧ ¬a ≤ b ∨ ¬b; excluded middle fails exactly at UNKNOWN.

- Level: QL-A
- Refines: AR-SYS-005
- Proved in `formal/Arare/Truth.lean` (AR-VER-104): `Truth.and_rank`, `Truth.or_rank`, `Truth.not_rank`, `Truth.and_comm`, `Truth.and_assoc`, `Truth.and_idem`, `Truth.and_true`, `Truth.and_false`, `Truth.or_comm`, `Truth.or_assoc`, `Truth.not_not`, `Truth.de_morgan_and`, `Truth.absorb`, `Truth.distrib`, `Truth.kleene`, `Truth.excluded_middle_fails`

### AR-HLR-013 — UNKNOWN is NIL in truth position

A NIL read where a truth value is expected is UNKNOWN, and it is still that NIL: it keeps its reason where reasons are kept (AR-HLR-020).

- Level: QL-A
- Refines: AR-SYS-005, AR-SYS-010
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `truthOf_nil`

### AR-HLR-014 — FILTER

FILTER keeps exactly the elements its predicate calls TRUE; an element whose predicate is FALSE, UNKNOWN or NIL is dropped, as SQL's WHERE drops a row.

- Level: QL-A
- Refines: AR-SYS-005
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `mem_filterT`

### AR-HLR-015 — Branching on UNKNOWN

A branch takes its THEN side only when its condition is TRUE; on FALSE, UNKNOWN or NIL it takes its ELSE side.

- Level: QL-A
- Refines: AR-SYS-007
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `selectT_true`, `selectT_false`, `selectT_unknown`, `selectT_nil`

### AR-HLR-016 — Aggregates skip NIL

SUM, and every aggregate, skips the NIL elements of its input; an aggregate whose input holds no element that is not NIL is NIL.

- Level: QL-A
- Refines: AR-SYS-008
- Proved in `formal/Arare/Sql.lean` (AR-VER-106): `sumT_all_nil`, `sumT_skip_nil`, `sumT_two`
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
- Planned: Phase 2: front-end tests over every literal form and QUANTIZE.

### AR-HLR-019 — Arrow layout of truth and NIL

A vector whose lanes may be NIL carries a validity bitmap in which a cleared bit marks a NIL lane; a vector of truth values carries, besides it, a value bitmap. UNKNOWN is a lane whose validity bit is cleared. Bit order, alignment and padding are Apache Arrow's.

- Level: QL-A
- Refines: AR-SYS-006
- Not proved formally: A memory layout is not a law about values; it is verified by tests against the Arrow specification, not by a Lean theorem.
- Planned: Phase 2: kernel tests of the bitmaps; Phase 6: a round trip through the Arrow C Data Interface.

### AR-HLR-020 — Reasons for NIL

The reference interpreter keeps, with each NIL, the reason it arose; the compiled route keeps only the validity bit. A reason never changes a value or a truth value.

- Level: QL-B
- Refines: AR-SYS-011
- Planned: Phase 2: interpreter tests; Phase 4: the compiled route drops reasons and nothing else, by translation validation.

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
