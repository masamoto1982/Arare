//! AR-VER-203: across scales, ADD, SUB, DIV, EQ and LT agree with the
//! specification applied to the exactly aligned values, and the result width
//! is the narrowest that holds the aligned operand ranges (AR-LLR-002,
//! AR-LLR-004, AR-LLR-005, AR-LLR-007). Every pair of int8 values, every
//! pair of exponents from {−2, 0, 3}.

mod common;
use arare_kernel::{Val, Width};
use arare_spec::{eq_t, fit, lt_t, W};
use common::{m, num, spec_truth, to_spec, values};

const SCALES: [i32; 3] = [-2, 0, 3];

fn shift(v: W, d: i32) -> W {
    match v {
        W::Fin(k) => W::Fin(k << d),
        p => p,
    }
}

/// The narrowest width holding every integer of magnitude ≤ bound, stated
/// independently of the kernel.
fn narrowest(bound: i128) -> Width {
    *Width::ALL
        .iter()
        .find(|w| bound <= w.max_finite() as i128)
        .unwrap_or(&Width::W64)
}

#[test]
fn ar_ver_203_aligned() {
    let max8 = Width::W8.max_finite() as i128;
    for ea in SCALES {
        for eb in SCALES {
            let e = ea.min(eb);
            let add_width = narrowest((max8 << (ea - e)) + (max8 << (eb - e)));
            let div_width = narrowest(if eb < 0 { max8 << -eb } else { max8 });
            for a in values(Width::W8) {
                for b in values(Width::W8) {
                    let (x, y) = (num(a, Width::W8, ea), num(b, Width::W8, eb));
                    let (sa, sb) = (shift(to_spec(a), ea - e), shift(to_spec(b), eb - e));

                    let r = x.add(y);
                    assert_eq!((r.width(), r.scale()), (add_width, e));
                    assert_eq!(
                        to_spec(r.val()),
                        fit(m(add_width), sa.add(sb)),
                        "{a:?}@{ea} + {b:?}@{eb}"
                    );
                    assert_eq!(to_spec(x.sub(y).val()), fit(m(add_width), sa.sub(sb)));

                    assert_eq!(
                        x.eq(y),
                        spec_truth(eq_t(sa, sb)),
                        "{a:?}@{ea} EQ {b:?}@{eb}"
                    );
                    assert_eq!(
                        x.lt(y),
                        spec_truth(lt_t(sa, sb)),
                        "{a:?}@{ea} LT {b:?}@{eb}"
                    );

                    // DIV at the dividend's scale: round(a·2^(−eb) / b).
                    let (n, d) = match (to_spec(a), to_spec(b)) {
                        (W::Fin(p), W::Fin(q)) if eb < 0 => (W::Fin(p << -eb), W::Fin(q)),
                        (W::Fin(p), W::Fin(q)) => (W::Fin(p), W::Fin(q << eb)),
                        (p, q) => (p, q),
                    };
                    let r = x.div(y);
                    assert_eq!((r.width(), r.scale()), (div_width, ea));
                    assert_eq!(
                        to_spec(r.val()),
                        fit(m(div_width), n.div(d)),
                        "{a:?}@{ea} / {b:?}@{eb}"
                    );
                }
            }
        }
    }
}

#[test]
fn ar_ver_203_mul_scales() {
    for (ea, eb) in [(-2, 3), (31, -31), (16, 15)] {
        let r = num(Val::Fin(3), Width::W8, ea)
            .mul(num(Val::Fin(-5), Width::W8, eb))
            .unwrap();
        assert_eq!(
            (r.val(), r.scale(), r.width()),
            (Val::Fin(-15), ea + eb, Width::W16)
        );
    }
    assert!(num(Val::Fin(1), Width::W8, 16)
        .mul(num(Val::Fin(1), Width::W8, 16))
        .is_err());
    assert!(num(Val::Fin(1), Width::W8, 30).rescale(2).is_err());
}
