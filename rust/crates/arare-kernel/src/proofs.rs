//! AR-VER-207: Kani proofs of the kernel (docs/quality/FORMAL_VERIFICATION.md).
//! Each harness states its law a second way — Euclidean division in a wider
//! type, the truth order's ranks, exact `i128` sums — rather than restating
//! the kernel, so it can disagree with it.

use crate::codes::{decode, encode, fit, is_code, Val};
use crate::column::{and_word, not_word};
use crate::num::{rescale_code, Num, MAX_SCALE};
use crate::truth::Truth;
use crate::width::Width;

fn any_width() -> Width {
    match kani::any::<u8>() % 4 {
        0 => Width::W8,
        1 => Width::W16,
        2 => Width::W32,
        _ => Width::W64,
    }
}

fn any_fin(w: Width) -> i64 {
    let k: i64 = kani::any();
    kani::assume(k.unsigned_abs() <= w.max_finite() as u64);
    k
}

fn lane(valid: u64, value: u64, i: u32) -> Truth {
    match ((valid >> i) & 1, (value >> i) & 1) {
        (0, _) => Truth::Unknown,
        (_, 0) => Truth::False,
        _ => Truth::True,
    }
}

/// AR-LLR-009: the word-wide Kleene AND is the scalar AND on every one of the
/// 64 lanes, for every pair of canonical words, and stays canonical.
#[kani::proof]
#[kani::unwind(65)]
fn and_word_is_lanewise_and() {
    let (va, xa, vb, xb): (u64, u64, u64, u64) =
        (kani::any(), kani::any(), kani::any(), kani::any());
    kani::assume(xa & !va == 0 && xb & !vb == 0);
    let (v, x) = and_word(va, xa, vb, xb);
    assert_eq!(x & !v, 0);
    let i: u32 = kani::any();
    kani::assume(i < 64);
    assert_eq!(lane(v, x, i), lane(va, xa, i).and(lane(vb, xb, i)));
}

/// AR-LLR-009: the word-wide NOT is the scalar NOT on every lane.
#[kani::proof]
fn not_word_is_lanewise_not() {
    let (v, x): (u64, u64) = (kani::any(), kani::any());
    kani::assume(x & !v == 0);
    let (nv, nx) = not_word(v, x);
    assert_eq!(nx & !nv, 0);
    let i: u32 = kani::any();
    kani::assume(i < 64);
    assert_eq!(lane(nv, nx, i), lane(v, x, i).not());
}

/// AR-LLR-003: `fit` answers the integer itself exactly when it lies in the
/// width's finite range, and `1/0` otherwise, for every `i128` and width.
#[kani::proof]
fn fit_is_exact_or_inf() {
    let w = any_width();
    let x: i128 = kani::any();
    let max = w.max_finite() as i128;
    match fit(w, x) {
        Val::Fin(k) => assert!(k as i128 == x && -max <= x && x <= max),
        Val::Inf => assert!(x < -max || x > max),
        Val::Bot => panic!("fit never answers 0/0"),
    }
}

/// AR-LLR-003: every code of every width decodes to a value that encodes
/// back to it, and decodes to a finite value only inside the finite range.
#[kani::proof]
fn codes_round_trip() {
    let w = any_width();
    let c: i64 = kani::any();
    kani::assume(is_code(w, c));
    let v = decode(w, c);
    assert_eq!(encode(w, v), c);
    if let Val::Fin(k) = v {
        assert!(k.unsigned_abs() <= w.max_finite() as u64);
    }
}

/// AR-LLR-006: RESCALE's shift-based rounding equals round half to even of
/// the Euclidean quotient, for every int32 code and every shift up to 31.
#[kani::proof]
fn rescale_is_round_half_even_w32() {
    let k = any_fin(Width::W32);
    let s: u32 = kani::any();
    kani::assume(s <= 31);
    let d = 1i64 << s;
    let (q, r) = (k.div_euclid(d), k.rem_euclid(d));
    let expected = if 2 * r < d || (2 * r == d && q.rem_euclid(2) == 0) {
        q
    } else {
        q + 1
    };
    assert_eq!(rescale_code(k, s), expected);
}

/// AR-LLR-004: ADD of two int16 numbers at scale 0 is the exact sum, at int32;
/// of two int32 numbers, the exact sum at int64.
#[kani::proof]
fn add_exact_w16_w32() {
    for (w, out) in [(Width::W16, Width::W32), (Width::W32, Width::W64)] {
        let (a, b) = (any_fin(w), any_fin(w));
        let r = Num::new(Val::Fin(a), w, 0)
            .unwrap()
            .add(Num::new(Val::Fin(b), w, 0).unwrap());
        assert_eq!((r.val(), r.width()), (Val::Fin(a + b), out));
    }
}

/// AR-LLR-004: ADD at the widest width, at any two scales, never overflows
/// an intermediate, and answers the exact aligned sum or `1/0`.
#[kani::proof]
fn add_w64_any_scales_no_overflow() {
    let (a, b) = (any_fin(Width::W64), any_fin(Width::W64));
    let (ea, eb): (i32, i32) = (kani::any(), kani::any());
    kani::assume((-MAX_SCALE..=MAX_SCALE).contains(&ea) && (-MAX_SCALE..=MAX_SCALE).contains(&eb));
    let r = Num::new(Val::Fin(a), Width::W64, ea)
        .unwrap()
        .add(Num::new(Val::Fin(b), Width::W64, eb).unwrap());
    let e = ea.min(eb);
    let exact = ((a as i128) << (ea - e)) + ((b as i128) << (eb - e));
    match r.val() {
        Val::Fin(k) => assert_eq!(k as i128, exact),
        Val::Inf => assert!(exact.unsigned_abs() > Width::W64.max_finite() as u128),
        Val::Bot => panic!("finite operands never sum to 0/0"),
    }
}

/// AR-LLR-004: MUL of two int16 numbers is the exact product, at int32.
#[kani::proof]
fn mul_exact_w16() {
    let (a, b) = (any_fin(Width::W16), any_fin(Width::W16));
    let r = Num::new(Val::Fin(a), Width::W16, 0)
        .unwrap()
        .mul(Num::new(Val::Fin(b), Width::W16, 0).unwrap())
        .unwrap();
    assert_eq!((r.val(), r.width()), (Val::Fin(a * b), Width::W32));
}

/// AR-LLR-005: DIV of two int16 numbers, the divisor nonzero, is within half
/// a unit of the exact quotient — the bound `Arare.W.div_error` proves for
/// the model — and a tie goes to the even quotient.
#[kani::proof]
fn div_half_unit_w16() {
    let (a, b) = (any_fin(Width::W16), any_fin(Width::W16));
    kani::assume(b != 0);
    let r = Num::new(Val::Fin(a), Width::W16, 0)
        .unwrap()
        .div(Num::new(Val::Fin(b), Width::W16, 0).unwrap());
    let Val::Fin(q) = r.val() else {
        panic!("a finite quotient")
    };
    let twice_err = (2 * (a as i64 - b as i64 * q)).abs();
    assert!(twice_err <= b.abs());
    if twice_err == b.abs() {
        assert_eq!(q % 2, 0);
    }
}
