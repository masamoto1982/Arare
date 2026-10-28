//! AR-VER-202: every pair of int8 values at scale 0, under ADD, SUB, MUL,
//! DIV, EQ and LT, agrees with the executable specification, and every int8
//! value under NEG and RESCALE does too (AR-LLR-003 to AR-LLR-007).
//! 255 × 255 = 65,025 pairs; the unused code is not a value.

mod common;
use arare_kernel::{Val, Width};
use arare_spec::{eq_t, fit, lt_t, rescale};
use common::{m, num, spec_truth, to_spec, values};

#[test]
fn ar_ver_202_pairs() {
    let mut pairs = 0u32;
    for a in values(Width::W8) {
        for b in values(Width::W8) {
            let (x, y) = (num(a, Width::W8, 0), num(b, Width::W8, 0));
            let (sa, sb) = (to_spec(a), to_spec(b));

            let r = x.add(y);
            assert_eq!(r.width(), Width::W16, "{a:?} + {b:?}");
            assert_eq!(
                to_spec(r.val()),
                fit(m(Width::W16), sa.add(sb)),
                "{a:?} + {b:?}"
            );

            let r = x.sub(y);
            assert_eq!(r.width(), Width::W16);
            assert_eq!(
                to_spec(r.val()),
                fit(m(Width::W16), sa.sub(sb)),
                "{a:?} - {b:?}"
            );

            let r = x.mul(y).unwrap();
            assert_eq!(r.width(), Width::W16);
            assert_eq!(
                to_spec(r.val()),
                fit(m(Width::W16), sa.mul(sb)),
                "{a:?} * {b:?}"
            );

            let r = x.div(y);
            assert_eq!(r.width(), Width::W8);
            assert_eq!(
                to_spec(r.val()),
                fit(m(Width::W8), sa.div(sb)),
                "{a:?} / {b:?}"
            );

            assert_eq!(x.eq(y), spec_truth(eq_t(sa, sb)), "{a:?} EQ {b:?}");
            assert_eq!(x.lt(y), spec_truth(lt_t(sa, sb)), "{a:?} LT {b:?}");
            pairs += 1;
        }
    }
    assert_eq!(pairs, 255 * 255);
}

#[test]
fn ar_ver_202_unary() {
    for a in values(Width::W8) {
        let x = num(a, Width::W8, 0);
        assert_eq!(to_spec(x.neg().val()), to_spec(a).neg());
        assert_eq!(x.neg().width(), Width::W8);
        for s in 0..=8u32 {
            let r = x.rescale(s).unwrap();
            let expected = match a {
                Val::Fin(k) => arare_spec::W::Fin(rescale(k as i128, s)),
                point => to_spec(point),
            };
            assert_eq!(to_spec(r.val()), expected, "{a:?} RESCALE {s}");
            assert_eq!(r.scale(), s as i32);
        }
    }
}
