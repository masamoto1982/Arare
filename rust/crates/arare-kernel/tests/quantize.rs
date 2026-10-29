//! AR-VER-208: QUANTIZE and exact values (AR-LLR-010). A ratio quantized at
//! a width and scale is the specification's rounded quotient fitted into the
//! width, over every ratio with |p| ≤ 300 and 1 ≤ q ≤ 40 and every exponent
//! in {−6, −1, 0, 2}; a number re-quantized is RESCALE or an exact shift, then
//! a fit; `exact` reads back the rational a number denotes.

mod common;
use arare_kernel::{Num, Val, Width};
use arare_spec::{fit, rescale, round_div, W};
use common::{m, num, to_spec, values};

#[test]
fn ar_ver_208_from_ratio() {
    for e in [-6, -1, 0, 2] {
        for p in -300i64..=300 {
            for q in 1i64..=40 {
                let (n, d) = if e <= 0 {
                    ((p as i128) << -e, q as i128)
                } else {
                    (p as i128, (q as i128) << e)
                };
                let r = Num::from_ratio(p, q, Width::W16, e).unwrap();
                assert_eq!(
                    to_spec(r.val()),
                    fit(m(Width::W16), W::Fin(round_div(n, d))),
                    "{p}/{q} at e={e}"
                );
                assert_eq!((r.width(), r.scale()), (Width::W16, e));
            }
        }
    }
    assert_eq!(Num::from_ratio(1, 0, Width::W8, 0).unwrap().val(), Val::Inf);
    assert_eq!(
        Num::from_ratio(-1, 0, Width::W8, 0).unwrap().val(),
        Val::Inf
    );
    assert_eq!(Num::from_ratio(0, 0, Width::W8, 0).unwrap().val(), Val::Bot);
    assert_eq!(
        Num::from_ratio(3, 4, Width::W8, -4).unwrap().val(),
        Val::Fin(12)
    ); // 0.75 = 12·2^−4
    assert_eq!(
        Num::from_ratio(1, 10, Width::W16, -12).unwrap().val(),
        Val::Fin(410)
    ); // round(409.6)
    assert_eq!(
        Num::from_ratio(1000, 1, Width::W8, 0).unwrap().val(),
        Val::Inf
    ); // does not fit int8
    assert!(Num::from_ratio(1, 3, Width::W8, -32).is_err());
}

#[test]
fn ar_ver_208_quantize() {
    for a in values(Width::W8) {
        for from in [-3, 0, 2] {
            let x = num(a, Width::W8, from);
            for to in [-5, -3, 0, 1, 2, 6] {
                for w in [Width::W8, Width::W16] {
                    let r = x.quantize(w, to).unwrap();
                    let expected = match to_spec(a) {
                        W::Fin(k) if to >= from => {
                            fit(m(w), W::Fin(rescale(k, (to - from) as u32)))
                        }
                        W::Fin(k) => fit(m(w), W::Fin(k << (from - to))),
                        p => p,
                    };
                    assert_eq!(to_spec(r.val()), expected, "{a:?}@{from} to {w:?}@{to}");
                    assert_eq!((r.width(), r.scale()), (w, to));
                }
            }
        }
    }
    assert!(num(Val::Fin(1), Width::W8, 0)
        .quantize(Width::W8, 32)
        .is_err());
}

#[test]
fn ar_ver_208_exact() {
    assert_eq!(num(Val::Fin(12), Width::W8, -4).exact(), (3, 4));
    assert_eq!(num(Val::Fin(-6), Width::W8, -1).exact(), (-3, 1));
    assert_eq!(num(Val::Fin(5), Width::W8, 3).exact(), (40, 1));
    assert_eq!(num(Val::Fin(0), Width::W8, -7).exact(), (0, 1));
    assert_eq!(num(Val::Fin(1), Width::W8, -31).exact(), (1, 1 << 31));
    assert_eq!(num(Val::Inf, Width::W8, 0).exact(), (1, 0));
    assert_eq!(num(Val::Bot, Width::W8, 0).exact(), (0, 0));
    // Every int8 code at exponent −3: k/8 in lowest terms.
    for k in -126i64..=126 {
        let (n, d) = num(Val::Fin(k), Width::W8, -3).exact();
        assert_eq!(n * 8, k as i128 * d);
        assert!(d > 0 && (d == 1 || n % 2 != 0));
    }
}
