//! Every accelerated route against the plain dispatch at once, on Vectors
//! that hold the points over zero (LANG.VALUES.EXACT, LANG.AUTHORITY.FREEDOM).
//!
//! The points are where the routes' pair laws, which assume a positive
//! denominator, hand over to the `Fraction`'s own arithmetic, and where an
//! order question projects instead of answering — the places one route can
//! drift from another unnoticed. Each case builds a random Vector of small
//! rationals salted with `1/0`, `-1/0`, `0/0` and `0`, runs one of the shapes
//! below on it with every route on (fused blocks, quickened calls, dense
//! kernels) and with every route off, and requires the two runs to leave the
//! same observation (`route_observation`): stack, outcome, charges and trace.
//! The scalar fast path stays on in both, as in
//! `fusion_contract_route_tests`, where the scalar law it runs is held to the
//! dispatch.

use crate::interpreter::route_observation::{observe, Limits, Observation};
use proptest::prelude::*;

fn run(source: &str, routes: bool) -> Observation {
    observe(
        source,
        |interp| {
            interp.set_fused_block_enabled(routes);
            interp.set_quickening_enabled(routes);
            interp.set_dense_kernels_enabled(routes);
        },
        Limits::default(),
    )
}

/// One element: mostly a small rational, sometimes a point or zero.
fn element() -> impl Strategy<Value = String> {
    prop_oneof![
        5 => (-9i64..10).prop_map(|n| n.to_string()),
        3 => (-9i64..10, 1i64..7).prop_map(|(n, d)| format!("{n}/{d}")),
        1 => Just("1/0".to_string()),
        1 => Just("-1/0".to_string()),
        1 => Just("0/0".to_string()),
        1 => Just("0".to_string()),
    ]
}

fn vector() -> impl Strategy<Value = String> {
    prop::collection::vec(element(), 1..12).prop_map(|items| format!("[ {} ]", items.join(" ")))
}

/// What is done with the Vector `V` (and a second one, `W`, of any length):
/// element-wise arithmetic and order, each walk with a block that divides,
/// compares or rounds, and the Words that ask for every element's order.
const SHAPES: &[&str] = &[
    "V 3 ADD",
    "V 3 SUB",
    "3 V SUB",
    "V -2 MUL",
    "V 0 DIV",
    "1 V DIV",
    "V V ADD",
    "V V DIV",
    "V W ADD",
    "V W MUL",
    "V 1/2 LT",
    "V W GT",
    "V FLOOR",
    "V ROUND",
    "V [ 2 MUL 1 ADD ] MAP",
    "V [ 'X' BIND 1 X DIV ] MAP",
    "V [ 0 MUL ] MAP",
    "V [ 1/2 GT ] MAP",
    "V [ 3 MIN ] MAP",
    "V [ FLOOR 1 ADD ] MAP",
    "V [ 0 GT ] FILTER",
    "V [ 0 MUL 0 EQ ] FILTER",
    "V 0 [ ADD ] FOLD",
    "V 1 [ MUL ] FOLD",
    "V [ 0 ] [ ADD ] FOLD",
    "V 0 [ ADD ] SCAN",
    "V SORT",
    "V ORDER",
    "V V EQ",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn every_route_agrees_with_the_plain_dispatch_on_the_points(
        v in vector(),
        w in vector(),
        shape in prop::sample::select(SHAPES),
    ) {
        let source = format!("{v} 'V' BIND {w} 'W' BIND {shape}");
        prop_assert_eq!(run(&source, true), run(&source, false), "`{}`", source);
    }
}
