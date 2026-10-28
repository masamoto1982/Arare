//! Fused walks over a one-lane seed, through User Word calls, with an
//! inexact `DIV` on the integer tier, and with `POW` and `LENGTH` of a
//! literal, against the interpreted walk (`fused_block_tests` holds the
//! comparison).

use crate::interpreter::fused_block_tests::{assert_same, fused_runs, Limits};

#[test]
fn lane_and_call_programs_agree() {
    for source in [
        // A one-lane seed, the idiomatic `[ 0 ]`: the lane beside a scalar
        // (no fast-path hit) and beside itself (a hit), on both tiers, as
        // FOLD and SCAN; and the Words that do not treat a lane as its value.
        "1 600 RANGE [ 0 ] [ ADD ] FOLD",
        "1 600 RANGE [ 0 ] [ ADD ] SCAN",
        "1 30 RANGE [ 1 ] [ MUL ] FOLD",
        "1 70 RANGE [ 1 ] [ MUL ] FOLD",
        "1 600 RANGE [ 7 ] [ SUB ] FOLD",
        "1 600 RANGE [ 7 ] [ 'E' BIND 'A' BIND E A SUB ] FOLD",
        "1 600 RANGE [ 0 ] [ 'E' BIND 'A' BIND A A ADD E SUB ] SCAN",
        "1 600 RANGE [ 0 ] [ 'E' BIND 'A' BIND A E E MUL ADD ] FOLD",
        "1 600 RANGE [ 0 ] [ 'E' BIND 'A' BIND E ] FOLD",
        "1 600 RANGE [ 0 ] [ 'E' BIND ] FOLD",
        "1 600 RANGE [ 1/2 ] [ ADD ] FOLD",
        "1 1 60 RANGE DIV [ 0 ] [ ADD ] FOLD",
        "1 1 60 RANGE DIV [ 0 ] [ ADD ] SCAN",
        "1 1 30 RANGE DIV [ 0 ] [ ADD ] FOLD",
        "[ 1 2 3 ] [ 10 ] [ DIV ] FOLD",
        "[ 1 0 2 ] [ 1 ] [ DIV ] FOLD",
        "[ 3 0 2 ] [ 1 ] [ 'E' BIND 'A' BIND E A DIV ] FOLD",
        "1 600 RANGE [ 9223372036854775000 ] [ ADD ] FOLD",
        "[ 1 NIL 2 ] [ 0 ] [ ADD ] FOLD",
        "[ 1 2 ] [ 0 ] [ 2 SQRT ADD ] FOLD",
        "1 20 RANGE [ 0 ] [ 'E' BIND 'A' BIND A 10 LT ] FOLD",
        "1 20 RANGE [ 0 ] [ 'E' BIND 'A' BIND A 5 EQ ] FOLD",
        "1 20 RANGE [ 0 ] [ ADD FLOOR ] FOLD",
        "1 20 RANGE [ 0 ] [ 'E' BIND 'A' BIND A A A TRUE SELECT ] FOLD",
        "1 20 RANGE [ 0 ] [ 'E' BIND 'A' BIND E 1 ADD ] FOLD",
        "1 20 RANGE [ 0 ] [ 'E' BIND 'A' BIND A 5 LT 'T' BIND A E ADD ] FOLD",
        "1 20 RANGE [ 0 ] [ 'E' BIND 'A' BIND A FLOOR 'T' BIND A E ADD ] FOLD",
        // A lane that outgrows a machine word part way and comes back.
        "[ 9223372036854775807 2 0 ] [ 1 ] [ MUL ] FOLD",
        "[ 9223372036854775807 2 0 ] [ 1/3 ] [ MUL ] SCAN",
        "1 20 RANGE [ 0 1 ] [ ADD ] FOLD",
        // User Words, inlined: a first call that builds the plan, a plan
        // already current, a Word called twice and one calling another, a
        // body binding the same name as the block, a body reading a name
        // only the caller bound (an ERROR across the barrier), a body that
        // consumes the caller's values, and bodies outside the subset.
        "[ 3 MUL 1 ADD ] 'F' DEF 1 600 RANGE [ F ] MAP",
        "[ 3 MUL 1 ADD ] 'F' DEF 1 2 RANGE [ F ] MAP 1 600 RANGE [ F ] MAP",
        "[ 2 MUL ] 'D' DEF [ D 1 ADD D ] 'G' DEF 1 600 RANGE [ G ] MAP",
        "[ 2 MUL ] 'D' DEF [ D 1 ADD D ] 'G' DEF 1 600 RANGE [ G D ] MAP",
        "[ 'X' BIND X X MUL ] 'SQ' DEF 1 600 RANGE [ 'X' BIND X SQ X ADD ] MAP",
        "[ X 1 ADD ] 'BADX' DEF 1 20 RANGE [ 'X' BIND X BADX ] MAP",
        "[ K 1 ADD ] 'BADK' DEF 5 'K' BIND 1 20 RANGE [ BADK ] MAP",
        "[ ADD ] 'PLUS' DEF 1 600 RANGE 0 [ PLUS ] FOLD",
        "[ ADD ] 'PLUS' DEF 1 600 RANGE [ 0 ] [ PLUS ] FOLD",
        "[ ADD ] 'PLUS' DEF 1 600 RANGE [ 5 PLUS ] MAP",
        "[ ADD ] 'PLUS' DEF 1 20 RANGE [ PLUS ] MAP",
        "[ 3 GT ] 'BIG' DEF 1 600 RANGE [ BIG ] FILTER",
        "[ 1 SWAP DIV ] 'INV' DEF [ 1 0 2 ] [ INV ] MAP",
        "[ 1 DIV ] 'INV' DEF [ 1 0 2 ] [ 'X' BIND 1 X INV ] MAP",
        "[ [ 1 ] ADD ] 'VADD' DEF 1 20 RANGE [ VADD ] MAP",
        "[ 2 MUL ] 'D' DEF 1 20 RANGE [ D ] MAP [ 3 ADD ] 'D' DEF 1 20 RANGE [ D ] MAP",
        // A plan made stale by an unrelated DEF is rebuilt, not reused.
        "[ 2 MUL ] 'D' DEF 1 2 RANGE [ D ] MAP [ 1 ] 'OTHER' DEF 1 600 RANGE [ D ] MAP",
        "[ 2 MUL ] 'D' DEF [ D ] 'E' DEF [ E ] 'F' DEF [ F ] 'G' DEF 1 600 RANGE [ G ] MAP",
        "1 20 RANGE [ [ 0 ] ] [ ADD ] FOLD",
        "1 20 RANGE [ TRUE ] [ ADD ] FOLD",
        // A `DIV` whose quotient may be a fraction, on the integer tier: a
        // fraction computed and not chosen, one chosen for some lanes (the
        // small-rational tier's walk then), an exact quotient of i64::MIN,
        // i64::MIN / -1, a zero divisor, a quotient bound and read back, both
        // candidates inexact, and a quotient fed to a Word that is not SELECT.
        "0 600 RANGE [ 'N' BIND N 2 DIV N 3 MUL 1 ADD N 2 DIV FLOOR 2 MUL N EQ SELECT ] MAP",
        "0 600 RANGE [ 'N' BIND N 2 DIV N 3 MUL 1 ADD N 2 DIV FLOOR 2 MUL N EQ NOT SELECT ] MAP",
        "0 600 RANGE [ 'N' BIND N 3 DIV N N 3 DIV FLOOR 3 MUL N EQ NOT SELECT ] MAP",
        "[ -9223372036854775808 4 ] [ 'N' BIND N 1 DIV N TRUE SELECT ] MAP",
        "[ -9223372036854775808 4 ] [ 'N' BIND N -1 DIV N FALSE SELECT ] MAP",
        "[ 4 0 6 ] [ 'N' BIND 12 N DIV 7 N 0 EQ NOT SELECT ] MAP",
        "1 600 RANGE [ 'N' BIND N 4 DIV 'Q' BIND Q N N 4 DIV FLOOR 4 MUL N EQ NOT SELECT ] MAP",
        "1 600 RANGE [ 'N' BIND N 2 DIV N 3 DIV N 6 GT SELECT ] MAP",
        "1 600 RANGE [ 'N' BIND N 2 DIV N 3 DIV N 0 GT SELECT ] MAP",
        "1 600 RANGE [ 2 DIV 1 ADD ] MAP",
        "1 600 RANGE [ 2 DIV 3 GT ] FILTER",
        "1 600 RANGE [ 2 DIV ] MAP",
        "[ 2 4 6 ] [ 2 DIV ] MAP",
        "1 600 RANGE 0 [ 'E' BIND 'A' BIND E 2 DIV A E E 2 DIV FLOOR 2 MUL EQ NOT SELECT ] FOLD",
        "1 600 RANGE 0 [ 'E' BIND 'A' BIND A E 2 DIV A E E 2 DIV FLOOR 2 MUL EQ SELECT ] SCAN",
        "1 600 RANGE [ 'E' BIND E 2 DIV E E 2 MUL EQ SELECT 0 LT ] FILTER",
        // MIN and MAX: a step each, no fast-path hit and no work, the left
        // operand on a tie; on each tier, as a FOLD's whole body, and on a
        // Boolean (nonNumeric) or a one-lane seed (declined).
        "-300 300 RANGE [ 0 MAX ] MAP",
        "-300 300 RANGE [ 0 MIN 5 MAX ] MAP",
        "-300 300 RANGE 0 [ MAX ] FOLD",
        "-300 300 RANGE 0 [ MIN ] SCAN",
        "1 1 600 RANGE DIV [ 1/7 MAX ] MAP",
        "1 1 600 RANGE DIV 1 [ MIN ] FOLD",
        "[ 9223372036854775807 2 ] [ 9223372036854775807 MUL 5 MAX ] MAP",
        "[ 1/2 2/4 3 ] [ 1/2 MIN ] MAP",
        "[ TRUE FALSE ] [ 1 MAX ] MAP",
        "1 20 RANGE [ 0 ] [ MAX ] FOLD",
        "[ 3 1 2 ] [ 'X' BIND X 2 MAX X 2 MIN SUB ] MAP",
    ] {
        assert_same(source, Limits::default());
    }
}

/// The Collatz step stays on the column tier whether its fraction is chosen
/// for a lane or not.
#[test]
fn a_fraction_chosen_or_not_keeps_the_column_tier() {
    let column_runs = |source: &str| {
        let before = crate::interpreter::fused_block_columns::column_runs_on_this_thread();
        let mut interp = crate::interpreter::Interpreter::new();
        crate::agent::block_on(interp.execute(source)).unwrap();
        crate::interpreter::fused_block_columns::column_runs_on_this_thread() - before
    };
    let collatz = "[ 'N' BIND N 2 DIV N 3 MUL 1 ADD N 2 DIV FLOOR 2 MUL N EQ SELECT ] MAP";
    assert_eq!(column_runs(&format!("0 1000 RANGE {collatz}")), 1);
    let chosen = "[ 'N' BIND N 2 DIV N N 2 DIV FLOOR 2 MUL N EQ NOT SELECT ] MAP";
    assert_eq!(column_runs(&format!("0 1000 RANGE {chosen}")), 1);
}

/// `POW` and `LENGTH` of a literal Vector fuse; `LENGTH` of the element, or
/// of a literal NIL, does not.
#[test]
fn pow_and_literal_length_fuse() {
    assert_eq!(fused_runs("1 9 RANGE [ 2 POW ] MAP"), 1);
    assert_eq!(fused_runs("1 9 RANGE [ [ 1 2 3 ] LENGTH ADD ] MAP"), 1);
    assert_eq!(fused_runs("1 9 RANGE 0 [ 3 POW ADD ] FOLD"), 1);
    assert_eq!(fused_runs("[ [ 1 ] [ 2 3 ] ] [ LENGTH ] MAP"), 0);
}

#[test]
fn pow_and_literal_length_programs_agree() {
    for source in [
        "1 9 RANGE [ 2 POW ] MAP",
        "-9 9 RANGE [ 3 POW ] MAP",
        "1 9 RANGE [ 1/2 ADD 5 POW ] MAP",
        "1 70 RANGE [ 'X' BIND 2 X POW ] MAP",
        "1 9 RANGE [ -1 POW ] MAP",
        "1 9 RANGE [ 0 POW ] MAP",
        "[ 0 1 2 ] [ 0 SWAP POW ] MAP",
        "1 9 RANGE [ 1/2 POW ] MAP",
        "1 600 RANGE 0 [ 2 POW ADD ] FOLD",
        "1 600 RANGE [ 0 ] [ 2 POW ADD ] SCAN",
        "1 9 RANGE [ [ 1 2 3 ] LENGTH ADD ] MAP",
        "1 9 RANGE [ [ ] LENGTH MUL ] MAP",
        "1 9 RANGE [ [ 1 NIL ] LENGTH ADD ] MAP",
        "1 9 RANGE [ [ [ 1 2 ] [ 3 4 ] ] LENGTH ADD ] MAP",
        "1 9 RANGE [ [ 1 2 3 ] LENGTH POW ] MAP",
    ] {
        for steps in [None, Some(0), Some(3), Some(20)] {
            for work in [None, Some(0), Some(5), Some(40)] {
                assert_same(
                    source,
                    Limits {
                        steps,
                        work,
                        bits: None,
                    },
                );
            }
        }
        for bits in [8, 32, 63, 64, 65] {
            assert_same(
                source,
                Limits {
                    steps: None,
                    work: None,
                    bits: Some(bits),
                },
            );
        }
    }
}

/// A one-lane literal in a `MAP` block (`[ 1 ] ADD`) is walked as its lane:
/// beside a scalar (no fast-path hit) and beside another lane (a hit), on
/// the integer and small-rational tiers, bound and read back, dead, inside a
/// User Word, past a machine word, and beside a zero divisor (whose quotient
/// is a point over zero the pair tier leaves to the dispatch); and declined
/// where a lane meets a Word that does not treat it as its value, and
/// outside `MAP`.
#[test]
fn one_lane_literals_in_map_agree() {
    let fused = [
        "1 600 RANGE [ [ 1 ] ADD ] MAP",
        "[ 1 0 2 ] [ 'X' BIND [ 1 ] X DIV ] MAP",
        "1 600 RANGE [ 'X' BIND [ 7 ] X SUB ] MAP",
        "1 600 RANGE [ [ 2 ] [ 3 ] MUL ADD ] MAP",
        "1 600 RANGE [ [ 1/3 ] ADD 2 MUL ] MAP",
        "1 600 RANGE [ [ 2 ] 'L' BIND L MUL L ADD ] MAP",
        "1 600 RANGE [ [ 1 ] 'L' BIND 2 MUL ] MAP",
        "1 600 RANGE [ 3 DIV [ 1 ] ADD ] MAP",
        "[ [ 1 ] ADD ] 'INC1' DEF 1 600 RANGE [ INC1 ] MAP",
        // Past a machine word: the general tier, and the lift's Vector.
        "[ 9223372036854775807 1 ] [ [ 1 ] ADD ] MAP",
    ];
    let declined = [
        "1 20 RANGE [ [ 5 ] LT ] MAP",
        "1 20 RANGE [ [ 5 ] EQ ] MAP",
        "1 20 RANGE [ [ 1 ] ADD FLOOR ] MAP",
        "1 20 RANGE [ [ 1 2 ] ADD ] MAP",
        "1 20 RANGE [ [ [ 1 ] ] ADD ] MAP",
        "1 20 RANGE [ [ 1 ] ADD 5 GT ] FILTER",
        "1 20 RANGE 0 [ [ 1 ] ADD ADD ] FOLD",
        "1 20 RANGE [ 0 ] [ [ 1 ] ADD ] SCAN",
        "[ ] [ [ 1 ] ADD ] MAP",
    ];
    for source in fused.iter().chain(&declined) {
        for steps in [None, Some(0), Some(3), Some(700)] {
            for work in [None, Some(0), Some(5), Some(700)] {
                assert_same(
                    source,
                    Limits {
                        steps,
                        work,
                        bits: None,
                    },
                );
            }
        }
        for bits in [8, 32, 63, 64, 65] {
            assert_same(
                source,
                Limits {
                    steps: None,
                    work: None,
                    bits: Some(bits),
                },
            );
        }
    }
    for source in fused {
        assert!(fused_runs(source) > 0, "`{source}` was not fused");
    }
    for source in &declined[..6] {
        assert_eq!(fused_runs(source), 0, "`{source}` was fused");
    }
}
