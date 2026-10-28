//! The dense column kernels (`dense_kernels`) against the general routes they
//! stand in for.
//!
//! LANG.AUTHORITY.FREEDOM makes the route unobservable, so each program here
//! runs with the kernels on and off and the two runs must agree on the
//! stack's values (`Debug`, so a dense Tensor and a boxed Vector differ), the
//! error, the resource usage and the Word a failure is attributed to.

use crate::interpreter::Interpreter;
use proptest::prelude::*;

use crate::interpreter::route_observation::Limits;
pub(super) use crate::interpreter::route_observation::Observation;

pub(super) fn observe(source: &str, dense: bool, bits: Option<u64>) -> Observation {
    crate::interpreter::route_observation::observe(
        source,
        |interp| interp.set_dense_kernels_enabled(dense),
        Limits {
            bits,
            ..Limits::default()
        },
    )
}

pub(super) fn assert_same(source: &str, bits: Option<u64>) {
    assert_eq!(
        observe(source, true, bits),
        observe(source, false, bits),
        "routes disagree on `{source}` under bits {bits:?}"
    );
}

#[test]
fn hand_picked_programs_agree() {
    for source in [
        // Integer lanes, on both sides of a scalar and against a Tensor,
        // below and above the old SIMD threshold of eight lanes.
        "1 1000 RANGE 2 MUL",
        "3 1 1000 RANGE SUB",
        "1 5 RANGE 1 5 RANGE ADD",
        "1 1000 RANGE 1 1000 RANGE MUL",
        "[ 1 2 3 ] 10 ADD",
        // Rational lanes and division.
        "1 1000 RANGE 2 DIV",
        "1 1000 RANGE 7 DIV 1/3 ADD",
        "1 1 100 RANGE DIV 1 1 100 RANGE DIV SUB",
        "-3 3 RANGE 2 DIV",
        "1/2 1 10 RANGE DIV",
        // A scalar divisor's residue-gcd table: built (divisor at most the
        // lane count), not built (more residues than lanes, a divisor past
        // the table), and a negative divisor that builds it.
        "-50 50 RANGE 12 DIV",
        "-50 50 RANGE -12 DIV",
        "1 10 RANGE 360 DIV",
        "-5000 5000 RANGE 4096 DIV",
        "-5000 5000 RANGE 4097 DIV",
        "[ -9223372036854775808 9223372036854775807 6 4 ] 2 DIV",
        "[ -9223372036854775808 9223372036854775807 6 4 ] -2 DIV",
        // The reciprocal floor division: both sides of 2^52, numerators a
        // reciprocal rounds past, and a divisor below the table's bound.
        "[ 4503599627370495 4503599627370496 4503599627370497 -4503599627370497 ] 7 DIV",
        "[ 9007199254740993 -9007199254740993 9223372036854775807 ] 4095 DIV",
        "[ -1 -7 -8 7 8 0 ] 7 DIV",
        "[ -1 -7 -8 7 8 0 ] -7 DIV",
        "[ 4503599627370495 4503599627370497 -4503599627370497 ] 7 DIV FLOOR",
        "[ 9223372036854775807 -9223372036854775808 ] 3 DIV FLOOR",
        // A rational Tensor shifted by an integer, on either side, over
        // zero and past a machine word.
        "1 1000 RANGE 7 DIV 1 ADD",
        "1 1000 RANGE 7 DIV 1 SUB",
        "1 1 1000 RANGE 7 DIV SUB",
        "[ 1/2 1/0 -1/0 0/0 ] 3 ADD",
        "3 [ 1/2 1/0 -1/0 0/0 ] SUB",
        "[ 9223372036854775807/2 1/3 ] 2 ADD",
        "[ -9223372036854775807/2 1/3 ] 2 SUB",
        "-9223372036854775808 [ 1/2 1/3 ] SUB",
        // A zero divisor lane — in integer lanes, in rational lanes, under a
        // scalar divisor, in every lane, in one lane, and carried onward —
        // a NIL lane, a mismatch, a one-lane Tensor.
        "1 -3 3 RANGE DIV",
        "1 [ 1 0 2 ] DIV",
        "[ 1 2 3 ] [ 1 0 2 ] DIV",
        "[ 1/2 3 5/3 ] [ 0 2 0 ] DIV",
        "[ 1 1/2 ] [ 0 0 ] DIV",
        "[ 1 2 3 ] 0 DIV",
        "0 [ 0 0 ] DIV",
        "[ 6 ] [ 0 ] DIV",
        "[ 1 2 3 ] [ 1 0 2 ] DIV 1 ADD",
        "[ 1 2 3 ] [ 1 0 2 ] DIV FLOOR",
        "[ 1 2 3 ] [ 1 0 2 ] DIV 1 GET NIL-REASON",
        "[ 1 2 3 ] [ 1 0 2 ] DIV [ 0 1 2 ] DIV",
        "[ -9223372036854775808 4 ] [ 0 -1 ] DIV",
        "[ -9223372036854775808 4 ] [ -1 0 ] DIV",
        // An absent operand lane passes through, leftmost first, on every
        // kernel: integer lanes, rational lanes, a scalar on either side,
        // both operands absent in one lane, an absent lane beside a zero
        // divisor, a written NIL beside a computed one, and the Words after.
        "[ 1 NIL 3 ] [ 10 20 30 ] ADD",
        "[ 1 NIL 3 ] [ 10 NIL 30 ] SUB",
        "[ 1 2 3 ] [ 10 NIL 30 ] MUL",
        "[ 1 NIL 3 ] [ NIL 0 3 ] DIV",
        "[ 1 NIL 0 ] [ 0 NIL 0 ] DIV",
        "[ 1/2 NIL 3 ] [ 10 20 30 ] ADD",
        "[ 1/2 3 ] [ NIL 1/3 ] MUL",
        "[ 1/2 NIL ] [ 0 1/3 ] DIV",
        "2 [ 1 NIL 3 ] ADD",
        "2 [ 1 NIL 3 ] SUB",
        "2 [ 1 NIL 3 ] DIV",
        "[ 1 NIL 3 ] 0 DIV",
        "1/2 [ 1 NIL 3 ] MUL",
        "[ 1 NIL 3 ] 1/2 DIV",
        "[ 1 NIL 3 ] [ 2 ] ADD",
        "[ NIL ] [ 2 ] ADD",
        "[ NIL ] 2 ADD",
        "[ 1 NIL 3 ] [ 10 20 30 ] LT",
        "[ 1 NIL 3 ] 2 GT",
        "2 [ 1 NIL 3 ] GT",
        "[ 1/2 NIL 3 ] [ 1/3 NIL 3 ] LT",
        "[ 1 NIL 3 ] FLOOR",
        "[ 1 NIL 3 ] ROUND",
        "[ 1/2 NIL -3/2 ] FLOOR",
        "[ 1/2 NIL -3/2 ] ROUND",
        "[ 1 2 3 ] [ 1 0 2 ] DIV [ 1 1 1 ] ADD 2 MUL 3 DIV FLOOR 0 GT",
        "[ 1 2 3 ] [ 1 0 2 ] DIV [ 1 NIL 3 ] ADD 1 GET NIL-REASON",
        "[ 1 NIL 3 ] [ 1 0 2 ] DIV 1 GET NIL-REASON",
        "[ 1 2 3 ] [ 1 0 2 ] DIV [ 1 NIL 3 ] [ 1 0 2 ] DIV EQ",
        "[ 'x' 'y' ] [ ABSENT ] MAP 3 ADD",
        "[ 'x' 'y' ] [ ABSENT ] MAP [ 1 0 ] DIV 0 GET NIL-REASON",
        "[ 1 0 ] [ 'x' 'y' ] [ ABSENT ] MAP DIV",
        "[ 9223372036854775807 NIL ] 1 ADD",
        "[ 9223372036854775807 NIL ] [ 1 NIL ] ADD",
        "[ -9223372036854775808 NIL ] -1 DIV",
        "[ 1 NIL 3 ] 2 MUL",
        "[ 1 2 3 ] [ 1 2 ] ADD",
        "[ 1 2 3 ] [ 2 ] ADD",
        "[ 2 ] [ 1 2 3 ] LT",
        // Overflow out of a machine word.
        "[ 9223372036854775807 1 ] 1 ADD",
        "[ 4611686018427387904 2 ] 2 MUL",
        "[ -9223372036854775808 ] 1 SUB",
        // A quotient whose denominator is i64::MIN cannot be negated in a
        // machine word.
        "[ 7 -7 2 ] -9223372036854775808 DIV",
        "[ -9223372036854775808 4 ] -1 DIV",
        "[ 9223372036854775807 3 ] 2 DIV 3 DIV",
        "[ 9223372036854775807/2 ] 9223372036854775807/3 ADD",
        // Comparisons.
        "1 1000 RANGE 500 GT",
        "1 1000 RANGE 500 LT",
        "500 1 1000 RANGE LT",
        "1 1 100 RANGE DIV 1/7 GT",
        "1 100 RANGE 100 1 RANGE LT",
        "[ 1/2 -1/3 9223372036854775807 ] [ 1/3 -1/2 -9223372036854775808 ] GT",
        // Order over zero: neither point is ordered, so a lane over zero is
        // the general route's `domainMiss`.
        "[ 1/0 2 -1/0 ] 1 LT",
        "[ 1/0 2 -1/0 ] 1/2 GT",
        "1 [ 1/0 2 -1/0 ] LT",
        "[ 1/0 2 -1/0 ] [ 1/0 -1/0 1/0 ] LT",
        "[ 1/0 2 -1/0 ] [ 1/0 -1/0 1/0 ] GT",
        "[ 1 2 3 ] [ 1 0 -2 ] DIV [ -1/0 1/0 0 ] GT",
        "[ 0/0 2 ] 1 LT",
        "[ 0/0 2 ] 1 LT 0 GET NIL-REASON",
        "[ 1 2 ] 0/0 GT",
        "1/0 [ 1 2 ] GT",
        "[ 1 2 ] -1/0 GT",
        // FLOOR / ROUND.
        "1 1000 RANGE 3 DIV FLOOR",
        "-20 20 RANGE 3 DIV FLOOR",
        "-20 20 RANGE 2 DIV ROUND",
        "-20 20 RANGE 7 DIV ROUND",
        "1 1000 RANGE FLOOR",
        "[ 1/2 NIL ] FLOOR",
        "[ [ 1/2 3/2 ] [ 5/2 7/2 ] ] FLOOR",
    ] {
        assert_same(source, None);
    }
}

pub(super) fn kernel_hits(source: &str) -> u64 {
    let before = crate::interpreter::dense_kernels::kernel_hits_on_this_thread();
    let mut interp = Interpreter::new();
    let _ = crate::agent::block_on(interp.execute(source));
    crate::interpreter::dense_kernels::kernel_hits_on_this_thread() - before
}

/// The equality tests would pass if no kernel ever answered; this pins that
/// the shapes they are for do, and the ones they are not for do not.
#[test]
fn the_kernels_answer_where_they_apply() {
    assert_eq!(kernel_hits("1 1000 RANGE 2 MUL"), 1);
    assert_eq!(kernel_hits("1 1000 RANGE 2 DIV"), 1);
    assert_eq!(kernel_hits("1 1000 RANGE 2 DIV FLOOR"), 2);
    assert_eq!(kernel_hits("1 1000 RANGE 500 GT"), 1);
    assert_eq!(kernel_hits("1 1000 RANGE 1 1000 RANGE ADD"), 1);
    assert_eq!(kernel_hits("1 [ 1 0 2 ] DIV"), 1);
    assert_eq!(kernel_hits("[ 1 2 3 ] [ 1 0 2 ] DIV"), 1);
    assert_eq!(kernel_hits("[ 1/2 3 ] [ 0 2 ] DIV"), 1);
    // The quotient by zero is a lane over zero for the next Word, which the
    // arithmetic and rounding kernels compute by the Fraction's own law. A
    // point has no order, so the ordering kernel answers its lane with the
    // `domainMiss` NIL the scalar law projects there.
    assert_eq!(kernel_hits("[ 1 2 3 ] [ 1 0 2 ] DIV 1 ADD"), 2);
    assert_eq!(
        kernel_hits("[ 1 2 3 ] [ 1 0 2 ] DIV 1 ADD 2 MUL 3 DIV FLOOR 0 GT"),
        6
    );
    assert_eq!(kernel_hits("[ 1/0 2 ] 1 LT"), 1);
    assert_eq!(kernel_hits("[ 0/0 2 ] 1 LT"), 1);
    assert_eq!(kernel_hits("1 [ 0/0 2 ] GT"), 1);
    // A quotient by zero is a *number* and stays a lane; a written NIL is
    // not a number and keeps a Vector nested, off the kernels.
    assert_eq!(kernel_hits("[ 1 2 3 ] [ 1 0 1 ] DIV 2 MUL"), 2);
    assert_eq!(kernel_hits("[ 1 2 3 ] [ 1 0 1 ] DIV [ 1 0 2 ] DIV"), 2);
    assert_eq!(kernel_hits("[ 1 2 3 ] [ 1 0 1 ] DIV FLOOR"), 2);
    assert_eq!(kernel_hits("[ 1 2 3 ] [ 1 0 1 ] DIV 2 GT"), 2);
    assert_eq!(kernel_hits("[ 1 NIL 3 ] 2 MUL"), 0);
    assert_eq!(kernel_hits("[ 1 NIL 3 ] FLOOR"), 0);
    assert_eq!(kernel_hits("[ 9223372036854775807 1 ] 1 ADD"), 0);
    assert_eq!(kernel_hits("[ 1 2 3 ] [ 2 ] ADD"), 0);
    assert_eq!(kernel_hits("2 3 ADD"), 0);
}

/// A zero divisor answers its own lane's point over zero and nothing else —
/// not the Tensor's columns. On every route a `DIV` result whose lanes fit a
/// machine word is a dense Tensor, the point over zero a lane like any other;
/// it used to be a boxed Vector on the lift and a nested Vector on the
/// one-lane fast path, so one zero divisor cost the vector its representation
/// for every Word after it.
#[test]
fn a_zero_divisor_keeps_the_result_dense() {
    use crate::types::ValueData;
    for (source, lanes, shape) in [
        (
            "[ 1 2 3 ] [ 1 0 2 ] DIV",
            vec!["1/1", "1/0", "3/2"],
            vec![3],
        ),
        (
            "[ 1/2 3 -5/3 ] [ 0 2 0 ] DIV",
            vec!["1/0", "3/2", "1/0"],
            vec![3],
        ),
        ("1 [ 1 0 2 ] DIV", vec!["1/1", "1/0", "1/2"], vec![3]),
        ("[ 1 0 -3 ] 0 DIV", vec!["1/0", "0/0", "1/0"], vec![3]),
        ("[ 6 ] [ 0 ] DIV", vec!["1/0"], vec![1]),
        ("[ [ 6 ] ] [ [ 0 ] ] DIV", vec!["1/0"], vec![1, 1]),
        (
            "[ [ 1 2 ] [ 3 4 ] ] [ [ 1 0 ] [ 0 2 ] ] DIV",
            vec!["1/1", "1/0", "1/0", "2/1"],
            vec![2, 2],
        ),
    ] {
        for dense in [true, false] {
            let mut interp = Interpreter::new();
            interp.set_dense_kernels_enabled(dense);
            crate::agent::block_on(interp.execute(source)).expect(source);
            let top = interp.get_stack().last().cloned().expect(source);
            let ValueData::Tensor { data, shape: got } = &top.data else {
                panic!("`{source}` (kernels {dense}) answered {top:?}, not a dense Tensor");
            };
            assert_eq!(
                got.as_slice(),
                shape.as_slice(),
                "`{source}` (kernels {dense})"
            );
            assert!(!data.is_pure_integer, "`{source}` (kernels {dense})");
            let read: Vec<String> = (0..data.len())
                .map(|i| crate::types::Value::from_dense_lane(data, i).to_string())
                .collect();
            assert_eq!(read, lanes, "`{source}` (kernels {dense})");
        }
    }
}

/// A lane over zero computes by the Fraction's own law on every route, and
/// the result stays a dense Tensor: the column kernels hand such a lane to
/// `Fraction` and write back the reduced pair it answers.
#[test]
fn a_lane_over_zero_computes_dense() {
    use crate::types::ValueData;
    for (source, lanes) in [
        (
            "[ 1 2 3 ] [ 1 0 1 ] DIV [ 10 20 30 ] ADD",
            vec!["11/1", "1/0", "33/1"],
        ),
        (
            "[ 1 2 3 ] [ 1 0 2 ] DIV [ 10 20 30 ] [ 1 0 1 ] DIV MUL",
            vec!["10/1", "1/0", "45/1"],
        ),
        (
            "[ 10 20 30 ] [ 1 1 0 ] DIV [ 1 2 3 ] [ 1 0 2 ] DIV MUL",
            vec!["10/1", "1/0", "1/0"],
        ),
        (
            "[ 1/2 3 -3/2 ] [ 1 0 1 ] DIV FLOOR",
            vec!["0/1", "1/0", "-2/1"],
        ),
        (
            "[ 1 2 3 ] [ 0 0 0 ] DIV [ 1 0 -1 ] MUL",
            vec!["1/0", "0/0", "1/0"],
        ),
        ("[ 1 2 ] [ 0 0 ] DIV [ 1 -1 ] 0 DIV ADD", vec!["0/0", "0/0"]),
        ("[ 5 7 ] [ 1 0 ] DIV 0 DIV", vec!["1/0", "1/0"]),
    ] {
        for dense in [true, false] {
            let mut interp = Interpreter::new();
            interp.set_dense_kernels_enabled(dense);
            crate::agent::block_on(interp.execute(source)).expect(source);
            let top = interp.get_stack().last().cloned().expect(source);
            let ValueData::Tensor { data, .. } = &top.data else {
                panic!("`{source}` (kernels {dense}) answered {top:?}, not a dense Tensor");
            };
            let read: Vec<String> = (0..data.len())
                .map(|i| crate::types::Value::from_dense_lane(data, i).to_string())
                .collect();
            assert_eq!(read, lanes, "`{source}` (kernels {dense})");
        }
    }
}

#[test]
fn the_size_ceiling_agrees() {
    for bits in [1, 4, 8, 63, 64] {
        assert_same("1 300 RANGE 1000 MUL", Some(bits));
        assert_same("1 300 RANGE 7 DIV", Some(bits));
    }
}

fn lane() -> impl Strategy<Value = String> {
    prop_oneof![
        6 => (-30i64..30).prop_map(|n| n.to_string()),
        2 => (-9i64..9, 1i64..9).prop_map(|(n, d)| format!("{n}/{d}")),
        1 => Just("0".to_string()),
        1 => Just("9223372036854775807".to_string()),
        1 => Just("-9223372036854775808".to_string()),
        1 => Just("NIL".to_string()),
    ]
}

fn operand() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => prop::collection::vec(lane(), 1..12).prop_map(|l| format!("[ {} ]", l.join(" "))),
        2 => lane(),
        1 => (-5i64..5, 1i64..40).prop_map(|(a, n)| format!("{a} {} RANGE", a + n)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn random_operations_agree(
        a in operand(),
        b in operand(),
        word in prop_oneof![
            Just("ADD"), Just("SUB"), Just("MUL"), Just("DIV"), Just("LT"), Just("GT"),
        ],
        unary in prop_oneof![Just(""), Just("FLOOR"), Just("ROUND")],
        bits in prop::option::of(1u64..130),
    ) {
        let source = format!("{a} {b} {word} {unary}");
        prop_assert_eq!(
            observe(&source, true, bits),
            observe(&source, false, bits),
            "`{}` under bits {:?}", source, bits
        );
    }
}

/// Shapes the kernels are for — one length, no absent lane — so most cases
/// reach them rather than declining at the shape check; the lanes still
/// reach zero and the edges of a machine word.
fn kernel_lane() -> impl Strategy<Value = String> {
    prop_oneof![
        6 => (-30i64..30).prop_map(|n| n.to_string()),
        3 => (-9i64..9, 1i64..9).prop_map(|(n, d)| format!("{n}/{d}")),
        1 => Just("4611686018427387904".to_string()),
        1 => Just("-9223372036854775808".to_string()),
        1 => Just("NIL".to_string()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn kernel_shaped_operations_agree(
        (a, b) in (1usize..24).prop_flat_map(|n| (
            prop::collection::vec(kernel_lane(), n),
            prop::collection::vec(kernel_lane(), n),
        )),
        scalar in kernel_lane(),
        shape in 0usize..3,
        word in prop_oneof![
            Just("ADD"), Just("SUB"), Just("MUL"), Just("DIV"), Just("LT"), Just("GT"),
        ],
        unary in prop_oneof![Just(""), Just("FLOOR"), Just("ROUND")],
    ) {
        let (a, b) = (format!("[ {} ]", a.join(" ")), format!("[ {} ]", b.join(" ")));
        let source = match shape {
            0 => format!("{a} {b} {word} {unary}"),
            1 => format!("{a} {scalar} {word} {unary}"),
            _ => format!("{scalar} {b} {word} {unary}"),
        };
        prop_assert_eq!(
            observe(&source, true, None),
            observe(&source, false, None),
            "`{}`", source
        );
    }
}

fn check_floor_divmod(x: i64, m: i64) {
    let got = crate::interpreter::dense_kernels::floor_divmod(x, m, 1.0 / m as f64);
    assert_eq!(got, (x.div_euclid(m), x.rem_euclid(m)), "{x} by {m}");
}

#[test]
fn the_reciprocal_floor_division_is_exact_at_its_edges() {
    let edge = 1i64 << 52;
    for m in [1, 2, 3, 7, 4095, 4096, 1 << 26, edge - 1, edge, i64::MAX] {
        for x in [
            0,
            1,
            -1,
            m - 1,
            m,
            m.saturating_add(1),
            -m,
            (-m).saturating_sub(1),
            edge - 1,
            edge,
            -(edge - 1),
            -edge,
            edge - 1 - (edge - 1) % m.max(1),
            i64::MAX,
            i64::MIN,
        ] {
            check_floor_divmod(x, m);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    #[test]
    fn the_reciprocal_floor_division_agrees(
        x in prop_oneof![any::<i64>(), -(1i64 << 53)..(1i64 << 53)],
        m in prop_oneof![1i64..5000, 1i64..(1i64 << 53), 1i64..i64::MAX],
    ) {
        check_floor_divmod(x, m);
    }
}

#[test]
fn a_vector_of_truths_is_the_vector_of_its_booleans() {
    use crate::types::Value;
    for truths in [vec![], vec![true], vec![false, true, true, false]] {
        let built = Value::from_truths(truths.iter().copied());
        let boxed = Value::from_vector(truths.iter().map(|&t| Value::from_bool(t)).collect());
        assert!(matches!(built.data, crate::types::ValueData::Truths(_)));
        assert_eq!(built, boxed);
        assert_eq!(
            crate::test_support::representation(std::slice::from_ref(&built)),
            crate::test_support::representation(std::slice::from_ref(&boxed))
        );
        assert_eq!(built.nesting(), boxed.nesting());
    }
}
