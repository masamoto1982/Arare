//! AR-VER-216-R: the checked rationals (AR-LLR-018). Every operation is exact
//! where it answers, answers `None` where the exact result leaves 128 bits,
//! and the order is exact even where cross-multiplying would overflow.

use arare_budget::Q;
use core::cmp::Ordering;

fn q(n: i128, d: i128) -> Q {
    Q::new(n, d).unwrap()
}

#[test]
fn ar_ver_216_r_normal_form() {
    assert_eq!(q(6, 4), q(3, 2));
    assert_eq!(q(3, -6), q(-1, 2));
    assert_eq!(q(0, -5), Q::ZERO);
    assert_eq!((q(-6, 4).num(), q(-6, 4).den()), (-3, 2));
    assert_eq!(Q::new(1, 0), None);
    assert_eq!(Q::new(i128::MIN, 1), None);
    assert_eq!(Q::new(1, i128::MIN), None);
    assert_eq!(Q::int(7), Some(q(7, 1)));
    assert_eq!(q(-3, 4).to_string(), "-3/4");
    assert_eq!(Q::ONE.to_string(), "1/1");
}

#[test]
fn ar_ver_216_r_arithmetic_on_a_grid() {
    // Against i128 arithmetic on cross-multiplied forms.
    let vals: Vec<(i128, i128)> = (-6..=6)
        .flat_map(|n| (1..=6).map(move |d| (n, d)))
        .collect();
    for &(a, b) in &vals {
        for &(c, d) in &vals {
            let (x, y) = (q(a, b), q(c, d));
            assert_eq!(x.add(y), Some(q(a * d + c * b, b * d)));
            assert_eq!(x.sub(y), Some(q(a * d - c * b, b * d)));
            assert_eq!(x.mul(y), Some(q(a * c, b * d)));
            assert_eq!(x.compare(y), (a * d).cmp(&(c * b)));
            assert_eq!(x.le(y), a * d <= c * b);
            assert_eq!(x.lt(y), a * d < c * b);
        }
        let x = q(a, b);
        assert_eq!(x.neg(), q(-a, b));
        assert_eq!(x.half(), Some(q(a, 2 * b)));
        assert_eq!(x.is_zero(), a == 0);
        if a != 0 {
            assert_eq!(x.inv(), q(b, a));
            assert_eq!(x.mul(x.inv()), Some(Q::ONE));
        }
    }
    // 0⁻¹ = 0, as in Lean.
    assert_eq!(Q::ZERO.inv(), Q::ZERO);
}

#[test]
fn ar_ver_216_r_overflow_is_none() {
    let big = q(i128::MAX, 1);
    assert_eq!(big.add(Q::ONE), None);
    assert_eq!(big.mul(q(2, 1)), None);
    assert_eq!(big.neg().sub(q(2, 1)), None);
    // Denominators that multiply past 128 bits.
    let tiny = q(1, i128::MAX);
    assert_eq!(tiny.mul(q(1, 3)), None);
    assert_eq!(tiny.add(q(1, 3)), None);
    assert_eq!(q(1, i128::MAX - 1).half(), None);
    // Reductions keep a product that fits from overflowing.
    assert_eq!(big.mul(q(1, i128::MAX)), Some(Q::ONE));
    assert_eq!(q(i128::MAX, 3).half(), Some(q(i128::MAX, 6)));
    assert_eq!(q(i128::MAX - 1, 1).half(), Some(q(i128::MAX / 2, 1)));
}

#[test]
fn ar_ver_216_r_order_never_overflows() {
    // n1·d2 and n2·d1 both overflow i128 here; the order is still exact.
    let a = q(i128::MAX, i128::MAX - 2);
    let b = q(i128::MAX - 1, i128::MAX - 3);
    // a = 1 + 2/(M−2), b = 1 + 2/(M−3): a < b.
    assert_eq!(a.compare(b), Ordering::Less);
    assert_eq!(b.compare(a), Ordering::Greater);
    assert_eq!(a.compare(a), Ordering::Equal);
    assert_eq!(q(-7, 2).compare(q(-7, 3)), Ordering::Less);
    assert_eq!(q(5, 1).compare(q(9, 2)), Ordering::Greater);
    assert_eq!(q(9, 2).compare(q(5, 1)), Ordering::Less);
}

#[test]
fn ar_ver_216_r_powers_of_two() {
    assert_eq!(Q::pow2(0), Some(Q::ONE));
    assert_eq!(Q::pow2(10), Some(q(1024, 1)));
    assert_eq!(Q::pow2(-3), Some(q(1, 8)));
    assert_eq!(Q::pow2(126), Some(q(1 << 126, 1)));
    assert_eq!(Q::pow2(-126), Some(q(1, 1 << 126)));
    assert_eq!(Q::pow2(127), None);
    assert_eq!(Q::pow2(-127), None);
}
