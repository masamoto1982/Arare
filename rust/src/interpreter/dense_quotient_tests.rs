//! The gcd-reducing column kernels (`dense_kernels::quotients`) against the
//! general routes: integer quotients walked four lanes at a time, and a
//! rational column multiplied or divided by one small integer through its
//! residue table. Each program runs with the kernels on and off, and the two
//! runs must agree on everything observable (`dense_kernels_tests::observe`).

use super::dense_kernels_tests::{assert_same, kernel_hits, observe};
use crate::interpreter::dense_kernels::binary_gcd_x4;
use crate::types::fraction::binary_gcd_u64;
use proptest::prelude::*;

#[test]
fn hand_picked_programs_agree() {
    for source in [
        // Integer quotients of every length around the four-lane step, with
        // a zero divisor, a zero dividend and the edges of a machine word in
        // and out of a step.
        "1 1 RANGE -1 -1 RANGE DIV",
        "1 3 RANGE -1 1 RANGE DIV",
        "1 4 RANGE -2 1 RANGE DIV",
        "1 7 RANGE -3 3 RANGE DIV",
        "1 1000 RANGE -500 499 RANGE DIV",
        "-500 499 RANGE 1 1000 RANGE DIV",
        "[ -9223372036854775808 -9223372036854775808 4 6 0 ] [ -9223372036854775808 -1 0 4 0 ] DIV",
        "[ 9223372036854775807 -9223372036854775807 12 18 ] [ 3 -3 -8 -12 ] DIV",
        "[ 4503599627370497 9007199254740993 -9007199254740993 6 ] [ 3 9 -3 4 ] DIV",
        "[ 1 2 3 4 5 6 7 8 9 ] 4097 DIV",
        "4097 [ 1 2 3 4 5 6 7 8 9 ] DIV",
        // A rational column by one small integer: `MUL` on either side and
        // `DIV` on the right, negative, ±1, with a lane over zero, a zero
        // lane, and products that outgrow a machine word.
        "1 1000 RANGE 7 DIV 1 ADD 2 MUL",
        "1 1000 RANGE 7 DIV 1 ADD -6 MUL",
        "-6 1 1000 RANGE 7 DIV 1 ADD MUL",
        "1 1000 RANGE 7 DIV 1 ADD 3 DIV",
        "1 1000 RANGE 7 DIV 1 ADD -21 DIV",
        "1 1000 RANGE 7 DIV 1 MUL",
        "1 1000 RANGE 7 DIV -1 DIV",
        "-50 49 RANGE 1 100 RANGE DIV 1 ADD 2 MUL 3 DIV FLOOR 0 GT",
        "[ 1/2 1/0 -1/0 0/0 0 5/3 -7/4 ] 3 MUL",
        "[ 1/2 1/0 -1/0 0/0 0 5/3 -7/4 ] -3 MUL",
        "[ 1/2 1/0 -1/0 0/0 0 5/3 -7/4 ] 3 DIV",
        "[ 1/2 1/0 -1/0 0/0 0 5/3 -7/4 ] -3 DIV",
        "-2 [ 1/2 1/0 -1/0 0/0 0 5/3 -7/4 ] MUL",
        "[ 9223372036854775807/2 1/3 ] 2 MUL",
        "[ 9223372036854775807/3 1/3 ] 2 MUL",
        "[ 1/9223372036854775807 1/3 ] 2 DIV",
        "[ -9223372036854775808/3 1/3 ] -1 DIV",
        "[ -9223372036854775808/3 1/3 ] -1 MUL",
        "[ -9223372036854775807/2 1/3 ] -2 DIV",
    ] {
        assert_same(source, None);
    }
    for bits in [1, 8, 63, 64] {
        assert_same("1 300 RANGE 7 DIV 1 ADD 5 MUL", Some(bits));
        assert_same("1 300 RANGE 7 DIV 1 ADD 5 DIV", Some(bits));
    }
}

/// The equality tests would pass if the new paths never ran; this pins that
/// the chain the residue kernel is for is answered on the columns.
#[test]
fn the_scaled_chain_stays_on_the_kernels() {
    assert_eq!(
        kernel_hits("1 100 RANGE -50 49 RANGE DIV 1 ADD 2 MUL 3 DIV FLOOR 0 GT"),
        // The divisor 0 leaves a `1/0` lane, which has no order: the last
        // comparison answers it with the `domainMiss` NIL, on the columns too.
        6
    );
}

#[test]
fn the_four_lane_gcd_agrees_at_its_edges() {
    let edges = [0, 1, 2, 3, 6, 1 << 16, (1 << 16) + 1, 1 << 63, u64::MAX];
    for &a in &edges {
        for &b in &edges {
            let got = binary_gcd_x4([a, b, a, 0], [b, a, 7, a]);
            let want = [(a, b), (b, a), (a, 7), (0, a)].map(|(x, y)| binary_gcd_u64(x, y));
            assert_eq!(got, want, "gcd of {a} and {b}");
        }
    }
}

fn gcd_operand() -> impl Strategy<Value = u64> {
    prop_oneof![
        3 => 0u64..64,
        3 => 0u64..(1 << 20),
        2 => any::<u64>(),
        1 => any::<u64>().prop_map(|x| x << 20),
    ]
}

/// A rational lane of a column the residue kernel takes, over zero now and
/// then, and near the edge of a machine word.
fn rational_lane() -> impl Strategy<Value = String> {
    prop_oneof![
        6 => (-60i64..60, 1i64..30).prop_map(|(n, d)| format!("{n}/{d}")),
        1 => Just("1/0".to_string()),
        1 => Just("-1/0".to_string()),
        1 => Just("0/0".to_string()),
        1 => Just("4611686018427387903/2".to_string()),
        1 => Just("-9223372036854775807/5".to_string()),
    ]
}

/// The numerator a rational lane is written with, as an integer lane.
fn numerator(lane: &str) -> &str {
    lane.split('/').next().unwrap_or(lane)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn the_four_lane_gcd_agrees(
        a in prop::array::uniform4(gcd_operand()),
        b in prop::array::uniform4(gcd_operand()),
    ) {
        let want = [0, 1, 2, 3].map(|k| binary_gcd_u64(a[k], b[k]));
        prop_assert_eq!(binary_gcd_x4(a, b), want);
    }

    #[test]
    fn scaled_and_quotient_columns_agree(
        lanes in prop::collection::vec(rational_lane(), 1..40),
        divisors in prop::collection::vec(-40i64..40, 40),
        k in -40i64..40,
        word in prop_oneof![Just("MUL"), Just("DIV")],
        side in 0usize..3,
    ) {
        let column = format!("[ {} ]", lanes.join(" "));
        let source = match side {
            0 => format!("{column} {k} {word}"),
            1 => format!("{k} {column} {word}"),
            _ => {
                let dividends: Vec<&str> = lanes.iter().map(|l| numerator(l)).collect();
                let divisors: Vec<String> =
                    divisors[..lanes.len()].iter().map(i64::to_string).collect();
                format!("[ {} ] [ {} ] DIV", dividends.join(" "), divisors.join(" "))
            }
        };
        prop_assert_eq!(
            observe(&source, true, None),
            observe(&source, false, None),
            "`{}`", source
        );
    }
}
