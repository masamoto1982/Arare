//! AR-VER-204: codes and widths at every width (AR-LLR-002, AR-LLR-003,
//! AR-LLR-006): every int8 and int16 code decodes and encodes back, the
//! unused code is no code, the extremes of int32 and int64 behave, overflow at
//! the widest width is 1/0, and RESCALE agrees with the specification over
//! every int16 value.

mod common;
use arare_kernel::{decode, encode, fit, is_code, Num, Val, Width};
use common::{num, to_spec, values};

#[test]
fn ar_ver_204_codes_roundtrip() {
    for w in [Width::W8, Width::W16] {
        let m = w.half_range() as i64;
        for c in -m..m {
            assert_eq!(is_code(w, c), c != -(m - 1), "{w:?} {c}");
            if is_code(w, c) {
                assert_eq!(encode(w, decode(w, c)), c);
                assert_eq!(
                    to_spec(decode(w, c)),
                    arare_spec::decode(w.half_range(), c as i128)
                );
            }
        }
        assert!(!is_code(w, -m - 1) && !is_code(w, m));
    }
    for w in Width::ALL {
        assert_eq!(encode(w, Val::Bot), w.bot_code());
        assert_eq!(encode(w, Val::Inf), w.inf_code());
        assert_eq!(decode(w, w.max_finite()), Val::Fin(w.max_finite()));
        assert!(!is_code(w, -w.inf_code()));
    }
    assert_eq!(
        (Width::W64.bot_code(), Width::W64.inf_code()),
        (i64::MIN, i64::MAX)
    );
    assert_eq!(
        (
            Width::W8.bot_code(),
            Width::W8.inf_code(),
            Width::W8.max_finite()
        ),
        (-128, 127, 126)
    );
}

#[test]
fn ar_ver_204_fit_and_widest() {
    for w in Width::ALL {
        let max = w.max_finite() as i128;
        assert_eq!(fit(w, max), Val::Fin(max as i64));
        assert_eq!(fit(w, -max), Val::Fin(-max as i64));
        assert_eq!(fit(w, max + 1), Val::Inf);
        assert_eq!(fit(w, -max - 1), Val::Inf);
    }
    let big = num(Val::Fin(Width::W64.max_finite()), Width::W64, 0);
    assert_eq!(big.add(num(Val::Fin(1), Width::W8, 0)).val(), Val::Inf);
    assert_eq!(
        big.add(num(Val::Fin(-1), Width::W8, 0)).val(),
        Val::Fin(Width::W64.max_finite() - 1)
    );
    assert_eq!(big.mul(big).unwrap().val(), Val::Inf);
    assert_eq!(big.neg().val(), Val::Fin(-Width::W64.max_finite()));
    assert_eq!(
        num(Val::Fin(1000), Width::W16, 0).narrow(Width::W8).val(),
        Val::Inf
    );
    assert_eq!(
        num(Val::Fin(-126), Width::W16, 0).narrow(Width::W8).val(),
        Val::Fin(-126)
    );
    assert_eq!(
        num(Val::Bot, Width::W16, 0).narrow(Width::W8).val(),
        Val::Bot
    );
}

#[test]
fn ar_ver_204_literals() {
    assert_eq!(Num::int(0).unwrap().width(), Width::W8);
    assert_eq!(Num::int(126).unwrap().width(), Width::W8);
    assert_eq!(Num::int(127).unwrap().width(), Width::W16);
    assert_eq!(Num::int(-32766).unwrap().width(), Width::W16);
    assert_eq!(Num::int(32767).unwrap().width(), Width::W32);
    assert_eq!(Num::int(i64::MAX - 1).unwrap().width(), Width::W64);
    assert!(
        Num::int(i64::MAX).is_none()
            && Num::int(i64::MIN).is_none()
            && Num::int(i64::MIN + 1).is_none()
    );
    assert!(Num::new(Val::Fin(127), Width::W8, 0).is_none());
    assert!(Num::new(Val::Fin(1), Width::W8, 32).is_none());
    // Regression: `i32::MIN.abs()` overflows; the scale check must not use it.
    assert!(Num::new(Val::Fin(1), Width::W8, i32::MIN).is_none());
    assert!(Num::new(Val::Fin(1), Width::W8, -31).is_some());
}

#[test]
fn ar_ver_204_rescale_int16() {
    for a in values(Width::W16) {
        let x = num(a, Width::W16, 0);
        for s in 0..=17u32 {
            let expected = match a {
                Val::Fin(k) => arare_spec::W::Fin(arare_spec::rescale(k as i128, s)),
                p => to_spec(p),
            };
            assert_eq!(
                to_spec(x.rescale(s).unwrap().val()),
                expected,
                "{a:?} RESCALE {s}"
            );
        }
    }
}
