//! AR-VER-229: the checker with names and long loops (AR-LLR-020, AR-LLR-028,
//! AR-LLR-030): a name carries what is known of the value bound to it, scope
//! by scope as the interpreter binds; a block of names takes a vector's
//! lanes; and coarsening keeps a twenty-step loop bounded where its exact
//! rationals would outgrow 128 bits.

use arare_budget::info::{self, coarsen, Info, Range};
use arare_budget::{analyze, budget, Abs, Analysis, Q};
use arare_front::parse;

fn check(src: &str) -> Analysis {
    analyze(&parse(src).unwrap())
}

fn budgets(src: &str) -> Vec<String> {
    check(src)
        .stack
        .iter()
        .map(|a| budget(a).map_or("?".to_string(), |q| q.to_string()))
        .collect()
}

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d).unwrap()
}

#[test]
fn ar_ver_229_names() {
    for (src, want) in [
        // A name carries the value's budget, wherever it is read.
        ("0.1 16 -12 QUANTIZE 'T' BIND T T ADD", vec!["1/5120"]),
        (
            "0.1 16 -12 QUANTIZE 'T' BIND [ 1 2 ] [ T MUL ] MAP",
            vec!["1/5120"],
        ),
        // Binding again replaces what is known.
        ("0.1 16 -12 QUANTIZE 'T' BIND 1 'T' BIND T", vec!["0/1"]),
        // A block of names: the lanes of a vector of known length …
        (
            "[ 1 0.5 8 -1 QUANTIZE ] [ 'A' 'B' ] BIND B A",
            vec!["0/1", "0/1"],
        ),
        // … any lane of one of unknown length …
        (
            "[ 1 2 ] [ 3 LT ] FILTER [ 'A' 'B' ] BIND A B ADD",
            vec!["0/1"],
        ),
        // … and nothing of a value that is not a vector, or a missing lane.
        ("5 [ 'A' ] BIND A", vec!["?"]),
        ("[ 1 ] [ 'A' 'B' ] BIND B", vec!["?"]),
        // A name read where nothing is known of it, or not bound there.
        ("[ 1 ] [ 'K' BIND K ] MAP K", vec!["0/1", "?"]),
        ("X 'X'", vec!["?", "0/1"]),
        // Text is the same in both meanings; EQ of it agrees.
        ("'A' 'A' EQ", vec!["0/1"]),
    ] {
        assert_eq!(budgets(src), want, "{src}");
    }
    // A FOLD block's names, bound afresh for each lane.
    let a = check("[ 1 2 3 ] 0 [ 'I' BIND 'X' BIND X I ADD ] FOLD");
    assert!(a.closed() && budget(&a.stack[0]) == Some(Q::ZERO));
    // A names operand that is not text or a block binds nothing.
    assert_eq!(budgets("1 2 BIND 3"), vec!["0/1"]);
    assert_eq!(check("1 [ 2 ] BIND").stack, Vec::<Abs>::new());
}

#[test]
fn ar_ver_229_long_loop() {
    // Twenty steps of the logistic map (AR-PERF-003's program): exact
    // rationals of 2^20 bits, bounded through coarsening.
    let lanes = (1..=20)
        .map(|k| k.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let src = format!(
        "[ {lanes} ] 0 16 -15 QUANTIZE [ 'I' BIND 'Y' BIND 5/16 16 -15 QUANTIZE 13 Y MUL \
         Y MUL 2 RESCALE 1/4 8 -2 QUANTIZE MUL SUB 15 RESCALE 16 NARROW ] FOLD \
         1/2 16 -15 QUANTIZE ADD"
    );
    let a = check(&src);
    assert!(a.closed(), "{:?}", a.exits);
    let b = budget(&a.stack[0]).unwrap();
    // Within four units of 2^-16, rounded outward to 2^-48.
    assert!(b.le(q(1, 16384)) && q(1, 65536).le(b), "{b}");
    assert_eq!(b.den() & (b.den() - 1), 0, "{b} is not on the 2^-48 grid");
}

#[test]
fn ar_ver_229_coarsen() {
    // Rounded outward only where a denominator exceeds the grid's.
    let r = |lo: Q, hi: Q, err: Q| {
        Info::Num(Range {
            lo,
            hi,
            err,
            smin: -8,
            smax: -4,
        })
    };
    let x = r(q(-1, 3), q(5, 7), q(1, 1000));
    // Denominators 3 and 7 fit eighths; 1000 does not.
    assert_eq!(coarsen(x, 3), r(q(-1, 3), q(5, 7), q(1, 8)));
    assert_eq!(coarsen(x, 1), r(q(-1, 2), q(1, 1), q(1, 2)));
    // 1000 fits 2^10: all three are kept.
    assert_eq!(coarsen(x, 10), x);
    assert_eq!(coarsen(x, 9), r(q(-1, 3), q(5, 7), q(1, 512)));
    assert_eq!(coarsen(x, 0), r(q(-1, 1), q(1, 1), q(1, 1)));
    for i in [Info::Top, Info::Inf, Info::Bot] {
        assert_eq!(coarsen(i, 3), i);
    }
    // A grid of 2^127 or beyond is no grid: nothing is known.
    assert_eq!(coarsen(x, 127), Info::Top);
    // The rounded numerator never leaves 128 bits: with a denominator past
    // the grid, |n · 2^d / den| is at most |n|.
    let big = r(q(-1, 3), q(i128::MAX, 5), Q::ZERO);
    // ⌈(2^127 − 1)·4/5⌉ / 4.
    let hi = q(68056473384187692692674921486353642291, 2);
    assert_eq!(coarsen(big, 2), r(q(-1, 3), hi, Q::ZERO));
    assert!(q(i128::MAX, 5).le(hi) && hi.sub(q(1, 4)).unwrap().lt(q(i128::MAX, 5)));
    assert_eq!(info::budget(coarsen(x, 1)), Some(q(1, 2)));
}

#[test]
fn ar_ver_229_down_up() {
    assert_eq!(q(7, 5).down(2), Some(q(5, 4)));
    assert_eq!(q(7, 5).up(2), Some(q(3, 2)));
    assert_eq!(q(-7, 5).down(2), Some(q(-3, 2)));
    assert_eq!(q(-7, 5).up(2), Some(q(-5, 4)));
    // On the grid, or with a denominator within it: unchanged.
    assert_eq!(q(5, 4).down(2), Some(q(5, 4)));
    assert_eq!(q(1, 3).down(2), Some(q(1, 3)));
    assert_eq!(Q::ZERO.up(0), Some(Q::ZERO));
    // The digits of a denominator near 2^127 do not overflow.
    let tiny = q(1, i128::MAX);
    assert_eq!(tiny.down(48), Some(Q::ZERO));
    assert_eq!(tiny.up(48), Some(q(1, 1 << 48)));
}
