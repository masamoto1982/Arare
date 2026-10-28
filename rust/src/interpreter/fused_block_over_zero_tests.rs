//! The two points over zero on the fused walk (`fused_block`) against
//! the interpreted walk: a number like any other on either route, and a walk
//! that stays fused wherever only an order asked of `0/0`, a law that answers
//! NIL, or a Word outside the subset would make it leave.

use super::fused_block_tests::{assert_same, fused_runs, Limits};

/// The two points over zero — `1/0` (spelled `-1/0` too) and `0/0` — as elements, as
/// literals, as seeds and as the quotient of a zero divisor: a number like
/// any other on either route, with the same values, NILs and charges. Only an
/// order asked of `0/0`, and a law with no plain answer, leave the fused walk.
const OVER_ZERO: [&str; 32] = [
    // Elements over zero.
    "1 [ 0 1 2 ] DIV [ 2 MUL 1 ADD ] MAP",
    "[ 1/0 -1/0 0/0 3 ] [ 1 ADD ] MAP",
    "[ 1/0 -1/0 0/0 3/2 ] [ 2 MUL 1/3 SUB ] MAP",
    "1 0 20 RANGE DIV [ 2 MUL 1 ADD ] MAP",
    "1 0 20 RANGE DIV 0 MUL 1 21 RANGE ADD [ 2 MUL 1 ADD ] MAP",
    // A zero divisor in the block.
    "[ 1 0 2 ] [ 'X' BIND 1 X DIV ] MAP",
    "[ 1 0 -2 ] [ 'X' BIND X X DIV ] MAP",
    "[ 3 0 4 ] [ 'D' BIND 10 10 D DIV FLOOR D MUL SUB ] MAP",
    "[ 1/2 0 -3/4 ] [ 0 DIV 1 ADD ] MAP",
    // Order: neither point is ordered.
    "[ 1/0 -1/0 2 ] [ 1/2 GT ] FILTER",
    "[ 1/0 -1/0 2 ] [ 1/2 LT ] MAP",
    "[ 1/0 -1/0 2 ] [ 3 MIN ] MAP",
    "[ 1/0 -1/0 2 ] [ 3 MAX ] MAP",
    "[ 1/0 0/0 2 ] [ 1/2 GT ] FILTER",
    "[ 0/0 1 ] [ 3 MIN ] MAP",
    "[ 1 2 ] [ 0/0 LT ] MAP",
    // Rounding: each point is its own floor and its own rounding.
    "[ 1/0 -1/0 0/0 5/2 ] [ FLOOR ] MAP",
    "[ 1/0 -1/0 0/0 5/2 ] [ ROUND ] MAP",
    // Equality, and the finiteness test it makes.
    "1 [ 0 1 2 ] DIV [ 0 MUL 0 EQ ] FILTER",
    "[ 1/0 0/0 2 ] [ 0/0 EQ ] FILTER",
    // Seeds over zero.
    "[ 1 2 3 ] [ 1/0 ] [ ADD ] FOLD",
    "[ 1 2 -3 ] 1/0 [ MUL ] SCAN",
    "[ 1 2 3 ] 1/0 [ ADD ] FOLD",
    "[ 1 2 3 ] [ -1/0 ] [ 0 MUL ADD ] SCAN",
    // Literals and lanes over zero in the block.
    "[ 1 2 3 ] [ 1/0 ADD ] MAP",
    "[ 1 2 3 ] [ [ 1/0 ] MUL ] MAP",
    "[ 1 0 3 ] [ 0/0 SWAP DIV ] MAP",
    // Plain laws.
    "[ 1/0 4 ] [ 6 GCD ] MAP",
    "[ 1/0 0/0 ] [ NIL? ] MAP",
    "[ 1/0 ] [ DEPTH ] MAP",
    // POW on a base over zero.
    "[ 1/0 -1/0 0/0 2 ] [ 3 POW ] MAP",
    "[ 1/0 2 ] [ 'X' BIND X X 2 POW ADD ] MAP",
];

#[test]
fn points_over_zero_agree() {
    for source in OVER_ZERO {
        assert_same(source, Limits::default());
    }
    for steps in [1, 5, 10, 11, 12] {
        let limits = Limits {
            steps: Some(steps),
            ..Limits::default()
        };
        assert_same("1 [ 0 1 2 ] DIV [ 2 MUL 1 ADD ] MAP", limits);
    }
    for work in [0, 1, 3, 5, 6, 7] {
        let limits = Limits {
            work: Some(work),
            ..Limits::default()
        };
        assert_same("[ 1/0 -1/0 0/0 3 ] [ 1 ADD ] MAP", limits);
    }
}

/// A point over zero keeps the walk fused; only an order asked of a point, a
/// law that answers NIL (`GCD`) and Words outside the subset leave it.
#[test]
fn points_over_zero_stay_fused() {
    for source in [
        "1 [ 0 1 2 ] DIV [ 2 MUL 1 ADD ] MAP",
        "[ 1/0 -1/0 0/0 3 ] [ 1 ADD ] MAP",
        "[ 1/0 0/0 5/2 ] [ FLOOR ] MAP",
        "[ 1/0 0/0 5/2 ] [ ROUND ] MAP",
        "1 [ 0 1 2 ] DIV [ 0 MUL 0 EQ ] FILTER",
        "[ 1 2 3 ] [ 1/0 ] [ ADD ] FOLD",
        "[ 1 2 -3 ] 1/0 [ MUL ] SCAN",
        "[ 1/0 0/0 ] [ NIL? ] MAP",
        "[ 1/0 ] [ DEPTH ] MAP",
        "[ 1 0 2 ] [ 'X' BIND 1 X DIV ] MAP",
        "[ 1 2 3 ] [ 1/0 ADD ] MAP",
    ] {
        assert_eq!(fused_runs(source), 1, "`{source}` was not fused");
    }
    for source in [
        "[ 1/0 0/0 2 ] [ 1/2 GT ] FILTER",
        "[ 1/0 2 ] [ 1/2 GT ] FILTER",
        "[ 0/0 1 ] [ 3 MIN ] MAP",
        "[ 1/0 4 ] [ 6 GCD ] MAP",
    ] {
        assert_eq!(fused_runs(source), 0, "`{source}` was fused");
    }
}
