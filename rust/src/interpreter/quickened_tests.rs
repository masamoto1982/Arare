//! The quickened scalar call (`quickened`) against the full dispatch.
//!
//! LANG.AUTHORITY.FREEDOM makes the route unobservable, so a program must
//! leave the same stack, error, resource usage, epochs and trace whether its
//! scalar calls are quickened or dispatched. The programs run their
//! arithmetic inside blocks the fused walk declines (each holds a Vector
//! literal), inside User Word bodies, and at the top level.

use crate::interpreter::route_observation::{self, Limits, Observation};
use crate::interpreter::Interpreter;
use proptest::prelude::*;

/// A run with quickening on or off, so every scalar call takes one of the
/// two routes compared here.
fn observe(source: &str, quickened: bool, limits: Limits) -> Observation {
    route_observation::observe(
        source,
        |interp| {
            interp.set_quickening_enabled(quickened);
        },
        limits,
    )
}

fn assert_same(source: &str, limits: Limits) {
    assert_eq!(
        observe(source, true, limits),
        observe(source, false, limits),
        "routes disagree on `{source}` under {limits:?}"
    );
}

#[test]
fn hand_picked_programs_agree() {
    for source in [
        // Each Word on small integers and fractions, in an unfused block.
        "1 50 RANGE [ [ 7 ] LENGTH ADD 2 MUL 1 SUB 3 DIV ] MAP",
        "1 50 RANGE [ [ 7 ] LENGTH ADD 1/3 ADD 2/5 MUL ] MAP",
        "1 50 RANGE [ [ 7 ] LENGTH ADD 25 LT ] MAP",
        "1 50 RANGE [ [ 7 ] LENGTH ADD 25 GT ] MAP",
        "1 50 RANGE [ [ 7 ] LENGTH ADD 25 EQ ] MAP",
        "1 50 RANGE [ [ 7 ] LENGTH ADD 25 MIN 30 MAX ] MAP",
        "[ 1/2 2/4 3 ] [ [ 7 ] LENGTH 7 SUB ADD 1/2 MIN ] MAP",
        // Declined: a zero divisor (a NIL), overflow, a BigInt, a NIL, a
        // Boolean, a String, a Vector operand, an irrational.
        "[ 0 1 2 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND 6 X DIV ] MAP",
        "[ 9223372036854775807 2 ] [ [ 7 ] LENGTH 7 SUB ADD 9223372036854775807 MUL ] MAP",
        "[ 1 2 ] [ [ 7 ] LENGTH 7 SUB ADD 99999999999999999999999 ADD ] MAP",
        "[ 1 2 ] [ [ 7 ] LENGTH 7 SUB ADD NIL ADD ] MAP",
        "[ 1 2 ] [ [ 7 ] LENGTH 7 SUB ADD TRUE ADD ] MAP",
        "[ 1 2 ] [ [ 7 ] LENGTH 7 SUB ADD 'a' ADD ] MAP",
        "[ 1 2 ] [ [ 7 ] LENGTH 7 SUB ADD [ 1 2 ] ADD ] MAP",
        "[ 1 2 ] [ [ 7 ] LENGTH 7 SUB ADD 2 SQRT ADD ] MAP",
        "[ 1 2 ] [ [ 7 ] LENGTH 7 SUB ADD TRUE EQ ] MAP",
        "[ -9223372036854775808 ] [ [ 7 ] LENGTH 7 SUB ADD -1 DIV ] MAP",
        "[ -9223372036854775808 ] [ [ 7 ] LENGTH 7 SUB ADD -1 MUL ] MAP",
        // POW and LENGTH: each costs a step and nothing else.
        "1 50 RANGE [ [ 7 ] LENGTH ADD 2 POW ] MAP",
        "[ 0 1 2 3 -1 -2 1/2 -3/4 ] [ [ 7 ] LENGTH 7 SUB ADD 3 POW ] MAP",
        "[ 0 1 2 3 -1 -2 1/2 -3/4 ] [ [ 7 ] LENGTH 7 SUB ADD 0 POW ] MAP",
        "[ 0 1 2 3 -1 -2 1/2 -3/4 ] [ [ 7 ] LENGTH 7 SUB ADD 1 POW ] MAP",
        "[ 0 1 2 3 -1 -2 1/2 -3/4 ] [ [ 7 ] LENGTH 7 SUB ADD -2 POW ] MAP",
        "[ 0 1 2 3 -1 -2 1/2 -3/4 ] [ [ 7 ] LENGTH 7 SUB ADD 1/2 POW ] MAP",
        "[ 0 1 2 3 -1 -2 1/2 -3/4 ] [ [ 7 ] LENGTH 7 SUB ADD 2 SQRT POW ] MAP",
        "[ 0 4 9 1/4 ] [ [ 7 ] LENGTH 7 SUB ADD 3/2 POW ] MAP",
        "[ 2 3 -2 -3 10 ] [ [ 7 ] LENGTH 7 SUB ADD 62 POW ] MAP",
        "[ 2 3 -2 -3 10 ] [ [ 7 ] LENGTH 7 SUB ADD 63 POW ] MAP",
        "[ 2 -2 ] [ [ 7 ] LENGTH 7 SUB ADD 64 POW ] MAP",
        "[ 1/2 -1/2 3/5 ] [ [ 7 ] LENGTH 7 SUB ADD 40 POW ] MAP",
        "[ 0 1 -1 2 ] [ [ 7 ] LENGTH 7 SUB ADD 524288 POW ] MAP",
        "[ 0 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 524289 POW ] MAP",
        "[ 0 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 9223372036854775807 POW ] MAP",
        "[ 0 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND X TRUE POW ] MAP",
        "[ 2 3 ] [ [ 7 ] LENGTH 7 SUB ADD NIL POW ] MAP",
        "[ 2 3 ] [ [ 7 ] LENGTH 7 SUB ADD [ 2 ] POW ] MAP",
        "[ 0 ] [ [ 7 ] LENGTH 7 SUB ADD 0 POW ] MAP",
        "[ 0 ] [ [ 7 ] LENGTH 7 SUB ADD -1 POW ] MAP",
        "[ 1 2 3 ] [ 2 POW LENGTH ] MAP",
        "[ 1 2 3 ] LENGTH [ ] LENGTH [ NIL 1 ] LENGTH ADD ADD",
        "[ [ 1 2 ] [ 3 ] [ ] ] [ LENGTH ] MAP",
        "[ 1 2 3 ] [ 'X' BIND [ X X ] LENGTH ] MAP",
        "NIL LENGTH",
        "'abc' LENGTH",
        "5 LENGTH",
        "LENGTH",
        "TRUE LENGTH",
        "[ 1 2 3 ] 2 SQRT LENGTH",
        "3 POW",
        "POW",
        // User Word bodies, called from the top level and from a block.
        "[ 2 MUL 1 ADD ] 'F' DEF 5 F 6 F ADD",
        "[ 'X' BIND X X MUL X 3 DIV SUB ] 'G' DEF 1 30 RANGE [ [ 7 ] LENGTH G ] MAP",
        "[ 0 DIV ] 'Z' DEF 5 Z",
        "[ ADD ] 'P' DEF 1 2 P 3 P",
        "[ ADD ] 'P' DEF 1 P",
        // Rounding, logic and SELECT on plain values, and declined on others.
        "[ 7/2 -7/2 5 -1/2 1/2 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND X FLOOR X ROUND ADD ] MAP",
        "[ 9223372036854775807/2 ] [ [ 7 ] LENGTH 7 SUB ADD ROUND ] MAP",
        "[ 1 -1 0 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND X 0 LT NOT X 0 GT AND ] MAP",
        "[ 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND X 0 LT X 0 GT EQ ] MAP",
        "[ 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND X X 2 MUL X 0 LT SELECT ] MAP",
        "[ 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND TRUE FALSE X 0 LT SELECT ] MAP",
        "[ 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND 'a' 'b' X 0 LT SELECT ] MAP",
        "[ 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND [ 1 2 ] 9 X 0 LT SELECT ] MAP",
        // A Vector candidate lifts the choice lane by lane: `[ 9 9 ]`, not 9.
        "[ 7 8 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND [ 1 2 ] 9 X 0 LT SELECT ] MAP",
        "[ 1 2 ] 9 FALSE SELECT",
        "[ 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND X 9 NIL SELECT ] MAP",
        "[ 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND NIL NOT ] MAP",
        "[ 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND X NOT ] MAP",
        "[ 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND TRUE X AND ] MAP",
        "[ 1 -1 ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND TRUE X EQ ] MAP",
    ] {
        assert_same(source, Limits::default());
    }
}

#[test]
fn ceilings_agree_at_their_boundaries() {
    let source = "[ 2 MUL 1 ADD 3 LT ] 'F' DEF 1 20 RANGE [ [ 7 ] LENGTH F ] MAP";
    for steps in [0, 1, 5, 40, 79, 80, 81, 100, 150] {
        let limits = Limits {
            steps: Some(steps),
            ..Limits::default()
        };
        assert_same(source, limits);
    }
    for work in [0, 1, 20, 39, 40, 41, 100] {
        let limits = Limits {
            work: Some(work),
            ..Limits::default()
        };
        assert_same(source, limits);
    }
    for bits in [1, 8, 32, 63, 64, 65] {
        let limits = Limits {
            bits: Some(bits),
            ..Limits::default()
        };
        assert_same(
            "[ 3 MUL ] 'T' DEF 1 40 RANGE [ [ 7 ] LENGTH T ] MAP",
            limits,
        );
    }
}

/// The equality above would hold if nothing were ever quickened; this pins
/// that a compiled scalar call is.
#[test]
fn compiled_scalar_calls_are_quickened() {
    let steps_without_dispatch = |quickened: bool| {
        let mut interp = Interpreter::new();
        interp.set_quickening_enabled(quickened);
        crate::agent::block_on(interp.execute("[ 2 MUL 1 ADD ] 'F' DEF 5 F")).unwrap();
        (
            interp.resource_usage().execution_steps,
            interp.get_stack().len(),
        )
    };
    assert_eq!(steps_without_dispatch(true), steps_without_dispatch(false));
    let mut interp = Interpreter::new();
    let before = crate::interpreter::quickened::quickened_calls_on_this_thread();
    crate::agent::block_on(interp.execute("[ 2 MUL 1 ADD ] 'F' DEF 5 F")).unwrap();
    assert_eq!(
        crate::interpreter::quickened::quickened_calls_on_this_thread() - before,
        2
    );
}

fn operand() -> impl Strategy<Value = String> {
    prop_oneof![
        6 => (-20i64..20).prop_map(|n| n.to_string()),
        2 => (-9i64..9, 1i64..9).prop_map(|(n, d)| format!("{n}/{d}")),
        1 => Just("0".to_string()),
        1 => Just("9223372036854775807".to_string()),
        1 => Just("-9223372036854775808".to_string()),
        1 => Just("4611686018427387904".to_string()),
        1 => Just("NIL".to_string()),
        1 => Just("TRUE".to_string()),
        1 => Just("'s'".to_string()),
        1 => Just("2 SQRT".to_string()),
        1 => Just("X".to_string()),
    ]
}

fn word() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("ADD"),
        Just("SUB"),
        Just("MUL"),
        Just("DIV"),
        Just("LT"),
        Just("GT"),
        Just("EQ"),
        Just("MIN"),
        Just("MAX"),
        Just("AND"),
        Just("POW"),
    ]
}

/// One step of a body: a binary Word on a pushed operand, a unary Word on
/// the top, or a SELECT between two pushed candidates by a pushed mask.
fn step() -> impl Strategy<Value = String> {
    let mask = prop_oneof![
        Just("TRUE"),
        Just("FALSE"),
        Just("NIL"),
        Just("1"),
        Just("X 0 LT"),
    ];
    prop_oneof![
        6 => (operand(), word()).prop_map(|(o, w)| format!("{o} {w}")),
        2 => prop_oneof![Just("FLOOR"), Just("ROUND"), Just("NOT")].prop_map(String::from),
        1 => (prop_oneof![Just("[ ]"), Just("[ 1 2 ]"), Just("[ NIL ]"), Just("X"), Just("'ab'"), Just("NIL")], word())
            .prop_map(|(v, w)| format!("{v} LENGTH {w}")),
        1 => (operand(), operand(), mask).prop_map(|(a, b, m)| format!("{a} {b} {m} SELECT")),
    ]
}

fn body() -> impl Strategy<Value = String> {
    prop::collection::vec(step(), 1..7).prop_map(|steps| steps.join(" "))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn random_compiled_programs_agree(
        elements in prop::collection::vec(operand(), 1..8),
        code in body(),
        mode in 0usize..3,
        steps in prop::option::of(0usize..120),
        work in prop::option::of(0u64..60),
        bits in prop::option::of(1u64..130),
    ) {
        let elements = elements
            .into_iter()
            .map(|e| if e == "X" || e == "2 SQRT" { "1".to_string() } else { e })
            .collect::<Vec<_>>()
            .join(" ");
        let source = match mode {
            0 => format!("[ 'X' BIND X {code} ] 'W' DEF [ {elements} ] [ [ 7 ] LENGTH 7 SUB ADD W ] MAP"),
            1 => format!("[ {elements} ] [ [ 7 ] LENGTH 7 SUB ADD 'X' BIND X {code} ] MAP"),
            // The top level.
            _ => format!("[ {elements} ] 1 GET 'X' BIND X {code}"),
        };
        let limits = Limits { steps, work, bits };
        prop_assert_eq!(
            observe(&source, true, limits),
            observe(&source, false, limits),
            "`{}` under {:?}", source, limits
        );
    }
}

/// `POW` on plain operands and `LENGTH` of a Vector are quickened; a negative
/// exponent, an overflow and a NIL are left to the dispatch.
#[test]
fn pow_and_length_are_quickened_where_they_apply() {
    let quickened = |source: &str| {
        let mut interp = Interpreter::new();
        let before = crate::interpreter::quickened::quickened_calls_on_this_thread();
        let _ = crate::agent::block_on(interp.execute(source));
        crate::interpreter::quickened::quickened_calls_on_this_thread() - before
    };
    assert_eq!(quickened("[ 3 POW ] 'P' DEF 5 P"), 1);
    assert_eq!(quickened("[ 0 POW ] 'P' DEF 1/2 P"), 1);
    assert_eq!(quickened("[ LENGTH ] 'L' DEF [ 1 2 3 ] L"), 1);
    // A body that does not fuse (it leaves a Vector) is quickened per element.
    assert_eq!(quickened("[ 1 2 3 ] [ [ ] LENGTH ADD 1 COLLECT ] MAP"), 6);
    assert_eq!(quickened("[ -1 POW ] 'P' DEF 5 P"), 0);
    assert_eq!(quickened("[ 63 POW ] 'P' DEF 3 P"), 0);
    assert_eq!(quickened("[ 2 POW ] 'P' DEF NIL P"), 0);
    assert_eq!(quickened("[ LENGTH ] 'L' DEF NIL L"), 0);
}
