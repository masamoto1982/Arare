//! The Words that run by their plain law (`fusion_contract`) — `GCD`, `NIL?`,
//! `DEPTH` — on each route that runs them, against the route that does not:
//! fused against interpreted and quickened against dispatched, compared on
//! everything a run leaves behind (`route_observation`), under step, work and
//! size ceilings. These are what hold each kernel to its Word's dispatch.

use crate::interpreter::route_observation::{observe, Limits, Observation};
use proptest::prelude::*;

fn fused(source: &str, on: bool, limits: Limits) -> Observation {
    observe(
        source,
        |interp| {
            interp.set_fused_block_enabled(on);
            interp.set_scalar_fastpath_enabled(true);
        },
        limits,
    )
}

fn quickened(source: &str, on: bool, limits: Limits) -> Observation {
    observe(
        source,
        |interp| {
            interp.set_quickening_enabled(on);
        },
        limits,
    )
}

fn assert_routes_agree(source: &str, limits: Limits) {
    assert_eq!(
        fused(source, true, limits),
        fused(source, false, limits),
        "fused and interpreted disagree on `{source}` under {limits:?}"
    );
    assert_eq!(
        quickened(source, true, limits),
        quickened(source, false, limits),
        "quickened and dispatched disagree on `{source}` under {limits:?}"
    );
}

const PROGRAMS: &[&str] = &[
    // Each Word alone, at the top level and in every walk.
    "12 18 GCD",
    "-12 18 GCD",
    "0 0 GCD",
    "1/2 4 GCD",
    "TRUE 4 GCD",
    "2 SQRT 4 GCD",
    "9223372036854775807 6 GCD",
    "170141183460469231731687303715884105727 12345678901234567890 GCD",
    "5 NIL?",
    "TRUE NIL?",
    "NIL NIL?",
    "7 DEPTH",
    "FALSE DEPTH",
    "1 60 RANGE [ 12 GCD ] MAP",
    "1 60 RANGE [ 36 SWAP GCD ] MAP",
    "1 60 RANGE [ 'X' BIND X X 2 MUL 1 ADD GCD ] MAP",
    "1 60 RANGE [ 6 GCD 1 EQ ] FILTER",
    "1 60 RANGE 0 [ GCD ] FOLD",
    "1 60 RANGE 0 [ 'E' BIND 'A' BIND A E GCD A ADD ] SCAN",
    "1 60 RANGE [ 0 ] [ GCD ] FOLD",
    "1 60 RANGE [ 1/2 GCD ] MAP",
    "1 60 RANGE [ 2 DIV 4 GCD ] MAP",
    "1 60 RANGE [ NIL? ] MAP",
    "1 60 RANGE [ NIL? NOT ] FILTER",
    "1 60 RANGE [ DEPTH ADD ] MAP",
    "1 60 RANGE [ 'X' BIND X DEPTH X ADD ] MAP",
    "1 60 RANGE [ 3 MUL 1 ADD DEPTH ] MAP",
    "[ 1 2 NIL 3 ] [ NIL? ] MAP",
    "[ [ 1 ] [ 2 ] ] [ DEPTH ] MAP",
    "[ 1 TRUE 3 ] [ 4 GCD ] MAP",
    "[ 4611686018427387904 9223372036854775807 ] [ 4611686018427387904 GCD ] MAP",
    // The two points over zero are plain values on every route: `GCD`
    // answers NIL for them (no plain answer), `NIL?` FALSE, `DEPTH` 0.
    "1/0 6 GCD",
    "0/0 0/0 GCD",
    "6 -1/0 GCD",
    "1/0 NIL?",
    "0/0 NIL?",
    "-1/0 DEPTH",
    "[ 1/0 4 ] [ 6 GCD ] MAP",
    "[ 1/0 0/0 -1/0 ] [ NIL? ] MAP",
    "[ 1/0 2 ] [ DEPTH ] MAP",
    "1 [ 0 1 2 ] DIV [ 'X' BIND X 2 GCD X NIL? SELECT ] MAP",
    "[ 'X' BIND X 6 GCD X ADD ] 'F' DEF 1/0 F 0/0 F",
    // In segments: a User Word body, and a block that does not fuse (it
    // leaves a Vector).
    "[ 'X' BIND X 6 GCD X ADD ] 'F' DEF 5 F 12 F 1/2 F",
    "[ 'X' BIND X NIL? NOT X DEPTH X 4 GCD ADD SELECT ] 'F' DEF 9 F",
    "[ 2 MUL 9223372036854775807 GCD ] 'F' DEF 5 F",
    "[ 12 GCD 'Y' BIND Y Y ADD [ 1 ] ] 'B' DEF 1 9 RANGE [ B ] MAP",
    "1 9 RANGE [ 12 GCD 'Y' BIND Y Y ADD [ 1 ] ] MAP",
    "[ GCD ] 'G2' DEF [ 3 G2 4 G2 ] 'F' DEF 60 F",
    // Through User Words.
    "[ 30 GCD ] 'G' DEF 1 60 RANGE [ G ] MAP",
    "[ 'X' BIND X 6 GCD X NIL? NOT SELECT ] 'H' DEF 1 60 RANGE [ H ] MAP 5 H",
];

#[test]
fn hand_picked_programs_agree() {
    for source in PROGRAMS {
        assert_routes_agree(source, Limits::default());
        for steps in [0, 1, 2, 3, 5, 30, 61, 62, 120] {
            assert_routes_agree(
                source,
                Limits {
                    steps: Some(steps),
                    ..Limits::default()
                },
            );
        }
        for work in [0, 1, 5, 40] {
            assert_routes_agree(
                source,
                Limits {
                    work: Some(work),
                    ..Limits::default()
                },
            );
        }
        for bits in [8, 63, 64, 65] {
            assert_routes_agree(
                source,
                Limits {
                    bits: Some(bits),
                    ..Limits::default()
                },
            );
        }
    }
}

/// The routes are not merely equal; the plain law is what ran.
#[test]
fn the_kernel_words_take_the_fast_routes() {
    let fused_runs = |source: &str| {
        let before = crate::interpreter::fused_block::fused_runs_on_this_thread();
        let _ = fused(source, true, Limits::default());
        crate::interpreter::fused_block::fused_runs_on_this_thread() - before
    };
    assert_eq!(fused_runs("1 60 RANGE [ 12 GCD ] MAP"), 1);
    assert_eq!(fused_runs("1 60 RANGE [ NIL? NOT ] FILTER"), 1);
    assert_eq!(fused_runs("1 60 RANGE [ 'X' BIND X DEPTH X ADD ] MAP"), 1);
    assert_eq!(fused_runs("1 60 RANGE [ 1/2 GCD ] MAP"), 0);

    let quickened_calls = |source: &str| {
        let before = crate::interpreter::quickened::quickened_calls_on_this_thread();
        let _ = quickened(source, true, Limits::default());
        crate::interpreter::quickened::quickened_calls_on_this_thread() - before
    };
    assert_eq!(quickened_calls("12 18 GCD"), 1);
    assert_eq!(quickened_calls("5 NIL?"), 1);
    assert_eq!(quickened_calls("7 DEPTH"), 1);
    assert_eq!(quickened_calls("1/2 4 GCD"), 0);
    assert_eq!(quickened_calls("NIL NIL?"), 0);
}

fn literal() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => (-40i64..40).prop_map(|n| n.to_string()),
        1 => (-9i64..9, 1i64..9).prop_map(|(n, d)| format!("{n}/{d}")),
        1 => Just("9223372036854775807".to_string()),
        1 => Just("4611686018427387904".to_string()),
        1 => Just("TRUE".to_string()),
    ]
}

fn token() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => literal(),
        4 => prop_oneof![Just("GCD"), Just("NIL?"), Just("DEPTH")].prop_map(String::from),
        3 => prop_oneof![Just("ADD"), Just("SUB"), Just("MUL"), Just("DIV"), Just("EQ"), Just("LT")]
            .prop_map(String::from),
        1 => prop_oneof![Just("NOT"), Just("FLOOR"), Just("SELECT")].prop_map(String::from),
        1 => prop_oneof![Just("'X' BIND"), Just("X")].prop_map(String::from),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn random_walks_and_calls_agree(
        code in prop::collection::vec(token(), 1..7).prop_map(|t| t.join(" ")),
        elements in prop::collection::vec(literal(), 0..10).prop_map(|t| t.join(" ")),
        seed in literal(),
        walk in 0usize..6,
        steps in prop::option::of(0usize..60),
        work in prop::option::of(0u64..100),
        bits in prop::option::of(1u64..130),
    ) {
        let source = match walk {
            0 => format!("[ {elements} ] [ {code} ] MAP"),
            1 => format!("[ {elements} ] [ {code} ] FILTER"),
            2 => format!("[ {elements} ] {seed} [ {code} ] FOLD"),
            3 => format!("[ {elements} ] {seed} [ {code} ] SCAN"),
            4 => format!("{elements} {code}"),
            // A User Word body: the typed-segment route.
            _ => format!("[ {code} ] 'F' DEF {elements} F"),
        };
        let limits = Limits { steps, work, bits };
        prop_assert_eq!(
            fused(&source, true, limits),
            fused(&source, false, limits),
            "fused and interpreted disagree on `{}` under {:?}", source, limits
        );
        prop_assert_eq!(
            quickened(&source, true, limits),
            quickened(&source, false, limits),
            "quickened and dispatched disagree on `{}` under {:?}", source, limits
        );
    }
}
