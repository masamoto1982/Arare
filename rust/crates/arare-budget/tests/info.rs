//! AR-VER-216-I: the checker's numeric functions (AR-LLR-019), case by case
//! and as MC/DC tables: every condition of every decision shown to change the
//! outcome on its own, in the form of Ajisai's `fraction_mcdc_tests.rs`
//! (docs/quality/VERIFICATION_PLAN.md).

use arare_budget::info::{self, Info, Range};
use arare_budget::Q;
use arare_kernel::{Val, Width};

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d).unwrap()
}

fn num(lo: (i128, i128), hi: (i128, i128), err: (i128, i128), s: i32) -> Info {
    Info::Num(Range {
        lo: q(lo.0, lo.1),
        hi: q(hi.0, hi.1),
        err: q(err.0, err.1),
        smin: s,
        smax: s,
    })
}

fn exact(k: i128) -> Info {
    num((k, 1), (k, 1), (0, 1), 0)
}

#[test]
fn ar_ver_216_i_literals() {
    // 0.1 at 2^-12 is code 410: off by |410/4096 − 1/10| = 1/10240.
    assert_eq!(
        info::lit(1, 10, Width::W16, -12, Val::Fin(410)),
        num((1, 10), (1, 10), (1, 10240), -12)
    );
    assert_eq!(info::lit(1, 0, Width::W8, 0, Val::Inf), Info::Inf);
    assert_eq!(info::lit(-3, 0, Width::W8, 0, Val::Inf), Info::Inf);
    assert_eq!(info::lit(0, 0, Width::W8, 0, Val::Bot), Info::Bot);
    // A literal the kernel could not fit is known as nothing.
    assert_eq!(info::lit(200, 1, Width::W8, 0, Val::Inf), Info::Top);
    // A code past the width's finite range is refused by the fit check.
    assert_eq!(
        info::lit(i64::MAX, 1, Width::W64, 0, Val::Fin(i64::MAX)),
        Info::Top
    );
    let n = arare_kernel::Num::int(-5).unwrap();
    assert_eq!(info::int(n), exact(-5));
    let p = arare_kernel::Num::new(Val::Inf, Width::W8, 0).unwrap();
    assert_eq!(info::int(p), Info::Top);
}

#[test]
fn ar_ver_216_i_points() {
    use Info::*;
    let one = exact(1);
    let zero = exact(0);
    let fuzzy = num((0, 1), (1, 1), (0, 1), 0); // may be zero
    for (a, b, add, mul, div) in [
        (Bot, Top, Bot, Bot, Bot),
        (Top, Bot, Bot, Bot, Bot),
        (Inf, Inf, Bot, Inf, Bot),
        (Inf, one, Inf, Inf, Inf),
        (one, Inf, Inf, Inf, exact(0)),
        (Inf, zero, Inf, Bot, Inf),
        (zero, Inf, Inf, Bot, exact(0)),
        (Inf, fuzzy, Inf, Top, Inf),
        (Inf, Top, Top, Top, Top),
        (Top, one, Top, Top, Top),
        (one, zero, one, zero, Inf),
        (zero, zero, zero, zero, Bot),
        (fuzzy, zero, fuzzy, zero, Top),
    ] {
        assert_eq!(info::add(a, b), add, "{a:?} + {b:?}");
        assert_eq!(info::mul(a, b), mul, "{a:?} · {b:?}");
        assert_eq!(info::div(a, b), div, "{a:?} / {b:?}");
    }
    assert_eq!(info::neg(Inf), Inf);
    assert_eq!(info::rescale(Bot, 3), Bot);
    assert_eq!(info::narrow(Inf, Width::W8), Inf);
    assert_eq!(info::quantize(Top, Width::W8, 0), Top);
}

#[test]
fn ar_ver_216_i_arithmetic() {
    let a = num((1, 1), (2, 1), (1, 8), -3);
    let b = num((-1, 1), (3, 1), (1, 4), -2);
    assert_eq!(info::add(a, b), num((0, 1), (5, 1), (3, 8), -3));
    assert_eq!(info::sub(a, b), num((-2, 1), (3, 1), (3, 8), -3));
    // |ab − xy| ≤ (2 + 1/8)·1/4 + 3·1/8 = 29/32; xy ∈ [−2, 6].
    assert_eq!(
        info::mul(a, b),
        Info::Num(Range {
            lo: q(-2, 1),
            hi: q(6, 1),
            err: q(29, 32),
            smin: -5,
            smax: -5
        })
    );
    // x/y for y ∈ [2, 4] ± 1/2: low = 3/2; x/y ∈ [1/4, 1];
    // 2^-3/2 + (1/8)/(3/2) + 2·(1/2)/(9/4) = 1/16 + 1/12 + 4/9.
    let c = num((2, 1), (4, 1), (1, 2), 0);
    assert_eq!(info::div(a, c), num((1, 4), (1, 1), (85, 144), -3));
    // A negative divisor: y ∈ [−4, −2] ± 1/2, low = 3/2 again.
    let d = num((-4, 1), (-2, 1), (1, 2), 0);
    assert_eq!(info::div(a, d), num((-1, 1), (-1, 4), (85, 144), -3));
    assert_eq!(info::neg(a), num((-2, 1), (-1, 1), (1, 8), -3));
    // RESCALE by 2 from 2^-3: half of 2^-1 more.
    assert_eq!(info::rescale(a, 2), num((1, 1), (2, 1), (3, 8), -1));
    // QUANTIZE up rounds; down does not.
    assert_eq!(
        info::quantize(a, Width::W16, 0),
        num((1, 1), (2, 1), (5, 8), 0)
    );
    assert_eq!(
        info::quantize(a, Width::W16, -6),
        num((1, 1), (2, 1), (1, 8), -6)
    );
    assert_eq!(
        info::quantize(a, Width::W16, -3),
        num((1, 1), (2, 1), (1, 8), -3)
    );
}

#[test]
fn ar_ver_216_i_scales_beyond_31_know_nothing() {
    let a = num((1, 1), (1, 1), (0, 1), -20);
    assert_eq!(info::mul(a, a), Info::Top); // the kernel refuses 2^-40
    let b = num((1, 1), (1, 1), (0, 1), 30);
    assert_eq!(info::rescale(b, 2), Info::Top);
    // An exact rational that leaves 128 bits is known as nothing too.
    let wide = num((1, i128::MAX), (1, i128::MAX), (0, 1), 0);
    assert_eq!(info::add(wide, exact(1)), Info::Top);
}

/// MC/DC of `guard`'s one decision: the magnitude fits the width at the
/// smallest scale (A); also the case its arithmetic overflows.
#[test]
fn ar_ver_216_i_mcdc_guard() {
    // half-range 128: the finite range is ±126.
    let at = |k: i128, err: (i128, i128)| num((k, 1), (k, 1), err, 0);
    for (i, fits) in [
        (at(126, (0, 1)), true),
        (at(126, (1, 2)), false),
        (at(-126, (0, 1)), true),
        (at(-127, (0, 1)), false),
    ] {
        let want = if fits { i } else { Info::Top };
        assert_eq!(info::narrow(i, Width::W8), want, "{i:?}");
    }
    // A bound whose arithmetic leaves 128 bits does not pass.
    let huge = num((1, 1), (1, 1), (i128::MAX, 1), 0);
    assert_eq!(info::narrow(huge, Width::W64), Info::Top);
}

/// MC/DC of `nonzero`, through DIV by `1/0` and MUL by `1/0`: surely above
/// zero (A) or surely below (B).
#[test]
fn ar_ver_216_i_mcdc_nonzero() {
    for (y, inf) in [
        (num((1, 1), (2, 1), (1, 2), 0), true),   // A
        (num((-2, 1), (-1, 1), (1, 2), 0), true), // B
        (num((-1, 1), (1, 1), (0, 1), 0), false), // neither
        (num((1, 2), (1, 1), (1, 2), 0), false),  // lo − err = 0 is not above
    ] {
        assert_eq!(info::mul(Info::Inf, y) == Info::Inf, inf, "{y:?}");
    }
}

/// MC/DC of `isZero`: low end zero (A), high end zero (B), no error (C).
#[test]
fn ar_ver_216_i_mcdc_is_zero() {
    for (y, bot) in [
        (num((0, 1), (0, 1), (0, 1), 0), true),
        (num((-1, 1), (0, 1), (0, 1), 0), false),
        (num((0, 1), (1, 1), (0, 1), 0), false),
        (num((0, 1), (0, 1), (1, 4), 0), false),
    ] {
        assert_eq!(info::mul(Info::Inf, y) == Info::Bot, bot, "{y:?}");
    }
}

/// MC/DC of DIV's decision: the divisor surely zero (A), else surely nonzero
/// (B).
#[test]
fn ar_ver_216_i_mcdc_div() {
    let x = exact(3);
    assert_eq!(info::div(x, exact(0)), Info::Inf); // A
    assert!(info::div(x, exact(2)).is_num()); // ¬A, B
    assert_eq!(info::div(x, num((-1, 1), (1, 1), (0, 1), 0)), Info::Top); // ¬A, ¬B
}

/// MC/DC of QUANTIZE's decision: the smallest scale below the new exponent.
#[test]
fn ar_ver_216_i_mcdc_quantize() {
    let a = num((1, 1), (1, 1), (0, 1), -2);
    assert_eq!(
        info::quantize(a, Width::W8, -1),
        num((1, 1), (1, 1), (1, 4), -1)
    );
    assert_eq!(info::quantize(a, Width::W8, -2), a);
}

/// MC/DC of the order agreeing: both exact (A, B), or certainly below (C),
/// or certainly not below (D).
#[test]
fn ar_ver_216_i_mcdc_lt_agrees() {
    let e = |lo: i128, hi: i128, err: (i128, i128)| num((lo, 1), (hi, 1), err, 0);
    for (x, y, agree) in [
        (e(0, 5, (0, 1)), e(0, 5, (0, 1)), true),  // A ∧ B
        (e(0, 5, (1, 2)), e(0, 5, (0, 1)), false), // ¬A
        (e(0, 5, (0, 1)), e(0, 5, (1, 2)), false), // ¬B
        (e(0, 1, (1, 4)), e(2, 3, (1, 4)), true),  // C
        (e(2, 3, (1, 4)), e(0, 1, (1, 4)), true),  // D
        (e(0, 1, (1, 2)), e(2, 3, (1, 2)), false), // touching
    ] {
        assert_eq!(info::lt_agrees(x, y), Some(agree), "{x:?} < {y:?}");
    }
    assert_eq!(info::lt_agrees(Info::Top, exact(1)), None);
}

/// MC/DC of equality agreeing: points, both exact (A, B), or apart (C, D).
#[test]
fn ar_ver_216_i_mcdc_eq_agrees() {
    let e = |lo: i128, hi: i128, err: (i128, i128)| num((lo, 1), (hi, 1), err, 0);
    for (x, y, agree) in [
        (Info::Inf, Info::Bot, true),
        (Info::Inf, e(0, 1, (1, 2)), true),
        (e(0, 1, (1, 2)), Info::Bot, true),
        (e(0, 5, (0, 1)), e(0, 5, (0, 1)), true),
        (e(0, 5, (1, 2)), e(0, 5, (0, 1)), false),
        (e(0, 5, (0, 1)), e(0, 5, (1, 2)), false),
        (e(0, 1, (1, 4)), e(2, 3, (1, 4)), true),
        (e(2, 3, (1, 4)), e(0, 1, (1, 4)), true),
        (Info::Top, e(0, 1, (0, 1)), false),
    ] {
        assert_eq!(info::eq_agrees(x, y), agree, "{x:?} = {y:?}");
    }
}

/// MC/DC of `known_int`: a single value (A), exact (B), an integer (C).
#[test]
fn ar_ver_216_i_mcdc_known_int() {
    assert_eq!(info::known_int(exact(16)), Some(16));
    assert_eq!(info::known_int(num((16, 1), (17, 1), (0, 1), 0)), None);
    assert_eq!(info::known_int(num((16, 1), (16, 1), (1, 2), 0)), None);
    assert_eq!(info::known_int(num((1, 2), (1, 2), (0, 1), 0)), None);
    assert_eq!(info::known_int(Info::Inf), None);
    assert_eq!(info::known_int(exact(i128::from(i64::MAX) + 1)), None);
}

#[test]
fn ar_ver_216_i_join_cross_budget() {
    let a = num((1, 1), (2, 1), (1, 8), -3);
    let b = num((4, 1), (5, 1), (1, 2), 0);
    let j = Info::Num(Range {
        lo: q(1, 1),
        hi: q(5, 1),
        err: q(1, 2),
        smin: -3,
        smax: 0,
    });
    assert_eq!(info::join(a, b), j);
    assert_eq!(info::join(Info::Inf, Info::Inf), Info::Inf);
    assert_eq!(info::join(Info::Bot, Info::Bot), Info::Bot);
    assert_eq!(info::join(Info::Inf, a), Info::Top);
    // The hull is [1, 5]: one side's machine against the other's exact value
    // may be off by 1/2 + 4.
    assert_eq!(
        info::cross(a, b),
        Info::Num(Range {
            err: q(9, 2),
            ..match j {
                Info::Num(r) => r,
                _ => unreachable!(),
            }
        })
    );
    assert_eq!(info::cross(Info::Inf, Info::Inf), Info::Inf);
    assert_eq!(info::cross(Info::Bot, Info::Bot), Info::Bot);
    assert_eq!(info::cross(Info::Bot, a), Info::Top);
    let wide = num((1, i128::MAX), (1, i128::MAX), (0, 1), 0);
    assert_eq!(info::cross(wide, num((1, 3), (1, 3), (0, 1), 0)), Info::Top);
    assert_eq!(info::budget(a), Some(q(1, 8)));
    assert_eq!(info::budget(Info::Bot), Some(Q::ZERO));
    assert_eq!(info::budget(Info::Top), None);
}
