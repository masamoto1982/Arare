//! A block walked again and again — an inner higher-order Word, once per
//! element of an outer one — is lowered once and its compiled forms kept
//! (`ExecutableCode::fused`). Against the
//! interpreted walk (`fused_block_tests` holds the comparison), on the things
//! a kept lowering could go stale over: a binding read from outside that
//! changes per element, a called Word whose plan the first walk builds, a
//! dictionary that moves, input types that change from walk to walk, the
//! call depth the walk starts at, and the ceilings.

use crate::interpreter::fused_block_tests::{assert_same, Limits};

const PROGRAMS: [&str; 13] = [
    "[ 1 2 3 ] [ 'K' BIND [ 1 2 ] [ K ADD ] MAP ] MAP",
    "[ 1 2 3 ] [ 'K' BIND [ 1 2 ] [ 0 ] [ K MUL ADD ] FOLD ] MAP",
    "[ 2 MUL ] 'D' DEF [ [ 1 2 ] [ 3 4 ] [ 5 6 ] ] [ [ D 1 ADD ] MAP ] MAP",
    "[ 2 MUL ] 'D' DEF [ [ 1 2 ] [ 3 4 ] ] [ [ D ] MAP [ 9 ] 'D' DEF ] MAP",
    "[ [ 1 2 ] [ 1/2 3 ] [ 9223372036854775807 1 ] [ 4 5 ] ] [ [ 0 ] [ ADD ] FOLD ] MAP",
    "[ [ 1 2 ] [ 1/2 3 ] [ 4 5 ] ] [ 0 [ ADD ] FOLD ] MAP",
    "[ [ TRUE FALSE ] [ 1 2 ] [ FALSE FALSE ] ] [ [ NOT ] MAP ] MAP",
    "[ [ 1 2 ] [ 3 4 ] ] [ [ 1 ] [ MUL ] FOLD ] MAP",
    "[ [ 1 NIL ] [ 3 4 ] ] [ [ 0 ] [ ADD ] FOLD ] MAP",
    "[ [ 0 ] [ ADD ] FOLD ] 'S' DEF [ S ] 'T' DEF [ [ 1 2 ] [ 3 4 ] ] [ S ] MAP [ [ 5 6 ] ] [ T ] MAP",
    "0 199 RANGE [ 'X' BIND X X 2 ADD RANGE ] MAP [ [ 0 ] [ ADD ] FOLD ] MAP",
    "0 49 RANGE [ 'X' BIND X X 3 ADD RANGE ] MAP [ [ 2 GT ] FILTER ] MAP",
    "[ [ 1 2 3 ] [ 4 5 6 ] ] [ [ 0 ] [ ADD ] SCAN ] MAP",
];

#[test]
fn blocks_walked_again_agree() {
    for source in PROGRAMS {
        assert_same(source, Limits::default());
    }
}

#[test]
fn ceilings_agree_on_blocks_walked_again() {
    let source = "0 29 RANGE [ 'X' BIND X X 2 ADD RANGE ] MAP [ [ 0 ] [ ADD ] FOLD ] MAP";
    for steps in [40, 120, 150, 160, 170, 200, 260, 400] {
        assert_same(
            source,
            Limits {
                steps: Some(steps),
                ..Limits::default()
            },
        );
    }
    for work in [0, 10, 59, 60, 61, 90, 200] {
        assert_same(
            source,
            Limits {
                work: Some(work),
                ..Limits::default()
            },
        );
    }
}
