//! Which tier of the walk (`fused_block`) answers it: the column tier
//! (`fused_block_columns`) takes every walk whose values are machine-word
//! rationals and truths, numbers over zero among them, and steps aside for
//! what does not fit it. `fused_block_tests` holds the tiers' answers to the
//! interpreted walk's.

use crate::interpreter::Interpreter;

fn column_runs(source: &str) -> u64 {
    let before = crate::interpreter::fused_block_columns::column_runs_on_this_thread();
    let mut interp = Interpreter::new();
    let _ = crate::agent::block_on(interp.execute(source));
    crate::interpreter::fused_block_columns::column_runs_on_this_thread() - before
}

/// The column tier answers integer and fractional walks alike, and steps
/// aside for what does not fit a machine word.
#[test]
fn the_column_tier_is_taken_where_it_applies() {
    assert_eq!(column_runs("1 1 1000 RANGE DIV [ 2 MUL ] MAP"), 1);
    assert_eq!(column_runs("1 1000 RANGE [ 7 DIV 1/3 ADD FLOOR ] MAP"), 1);
    assert_eq!(column_runs("1 1 1000 RANGE DIV [ 1/2 GT ] FILTER"), 1);
    assert_eq!(column_runs("1 1 30 RANGE DIV 0 [ ADD ] FOLD"), 1);
    assert_eq!(column_runs("1 1000 RANGE [ 2 MUL ] MAP"), 1);
    // A sum past a machine word leaves.
    assert_eq!(column_runs("1 1 60 RANGE DIV 0 [ ADD ] FOLD"), 0);
    // A zero divisor answers a point over zero, which is a pair like any
    // other here (`small_rational::div_total`).
    assert_eq!(column_runs("1 1 60 RANGE DIV [ 0 DIV ] MAP"), 1);
}

/// Elements over zero, and a zero divisor, stay on the column tier
/// (`small_rational`'s total forms); an order asked of a point has no answer
/// on it.
#[test]
fn points_over_zero_stay_on_the_column_tier() {
    assert_eq!(column_runs("1 [ 0 1 2 ] DIV [ 2 MUL ] MAP"), 1);
    assert_eq!(column_runs("[ 1 0 2 ] [ 'X' BIND 1 X DIV ] MAP"), 1);
    assert_eq!(column_runs("[ 1/0 0/0 5/2 ] [ FLOOR ] MAP"), 1);
    assert_eq!(column_runs("[ 1/0 2 ] [ 1/2 GT ] FILTER"), 0);
    assert_eq!(column_runs("[ 1 2 -3 ] 1/0 [ MUL ] SCAN"), 1);
    assert_eq!(column_runs("[ 1/0 0/0 2 ] [ 1/2 GT ] FILTER"), 0);
    assert_eq!(column_runs("[ 0/0 1 ] [ 3 MIN ] MAP"), 0);
}

/// The shortcuts the kernels take on the way — a division by a power of two
/// as a shift, an integer beside a fraction and a small factor without a
/// full gcd — answer what
/// the interpreted walk does: negative and odd lanes, the extremes of a
/// machine word, and quotients that are not exact among them.
#[test]
fn shortcut_arithmetic_agrees_with_the_interpreted_walk() {
    for source in [
        "-20 20 RANGE [ 2 DIV ] MAP",
        "-20 20 RANGE [ 4 DIV FLOOR ] MAP",
        "-20 20 RANGE [ 1 DIV FLOOR ] MAP",
        "-20 20 RANGE [ 'N' BIND N 2 DIV N 3 MUL 1 ADD N 2 DIV FLOOR 2 MUL N EQ SELECT ] MAP",
        "[ -9223372036854775808 9223372036854775807 -1 ] [ 8 DIV FLOOR ] MAP",
        "[ -9223372036854775808 9223372036854775807 -8 ] [ 'N' BIND N 8 DIV N 8 DIV FLOOR N 0 LT SELECT ] MAP",
        "-20 20 RANGE [ 1/3 ADD 2 MUL ] MAP",
        "-20 20 RANGE [ 5/8 SUB 4 MUL ] MAP",
        "1 1 20 RANGE DIV [ 3 ADD 6 MUL ] MAP",
        "1 1 20 RANGE DIV [ 'X' BIND 3 X SUB 8 MUL ] MAP",
        "[ 9223372036854775807 -9223372036854775808 ] [ 1/2 ADD ] MAP",
        "1 1 20 RANGE DIV 0 [ ADD ] FOLD",
    ] {
        super::fused_block_tests::assert_same(source, Default::default());
    }
}
