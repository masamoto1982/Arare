//! The column kernels that answer lane by lane — rounding, ordering,
//! equality, the extremes, the truth connectives, `SELECT` and `POW` — and the
//! two Words' wrappers over a Tensor. A child module of `dense_kernels`.

use super::{
    floor_divmod, floor_pair, hit, integer_lanes, pair_lanes, paired, zip_nums, Lanes, Out,
};
use crate::interpreter::comparison::OrderingKind;
use crate::types::small_rational;
use crate::types::Value;

/// `FLOOR`/`ROUND` of `n` lanes into `out`. Every rational rounds to an
/// integer; a point over zero is its own floor and its own rounding.
pub(crate) fn rounded_into(rounding: Rounding, a: Lanes, n: usize, out: &mut Out) -> Option<()> {
    out.clear();
    out.reserve(n);
    let (nums, dens) = match a {
        Lanes::Columns {
            nums,
            integer: true,
            ..
        } => {
            out.nums.extend_from_slice(nums);
            out.integers();
            return Some(());
        }
        Lanes::Columns { nums, dens, .. } => (nums, dens),
        Lanes::Splat(..) => {
            let x = a.at(0);
            let (rn, rd) = match rounding {
                Rounding::Floor => floor_pair(x),
                Rounding::Round if x.1 == 0 => x,
                Rounding::Round => (small_rational::round_half_away_from_zero(x.0, x.1), 1),
            };
            out.extend(Lanes::Splat(rn, rd), n);
            return Some(());
        }
    };
    // A column's denominators repeat (after `7 DIV` they are 7 or 1), so the
    // reciprocal `floor_divmod` multiplies by is kept for the last one met;
    // an integer lane is its own floor and rounding and leaves it alone.
    let (mut last_d, mut inv) = (1i64, 1.0f64);
    let mut over_zero = false;
    out.nums.extend(nums.iter().zip(dens).map(|(&n, &d)| {
        if d == 1 {
            return n;
        }
        if d == 0 {
            over_zero = true;
            return n;
        }
        match rounding {
            Rounding::Floor => {
                if d != last_d {
                    (last_d, inv) = (d, 1.0 / d as f64);
                }
                floor_divmod(n, d, inv).0
            }
            Rounding::Round => small_rational::round_half_away_from_zero(n, d),
        }
    }));
    if !over_zero {
        out.integers();
        return Some(());
    }
    // The two points over zero keep their own denominator.
    out.dens.extend(dens.iter().map(|&d| i64::from(d != 0)));
    out.settle();
    Some(())
}

/// `a LT b` / `a GT b` over `n` lanes into `out` as truth lanes (`0`/`1`), or
/// `None` at a lane over zero, which has no order.
pub(crate) fn ordering_into(
    kind: OrderingKind,
    a: Lanes,
    b: Lanes,
    n: usize,
    out: &mut Out,
) -> Option<()> {
    let truth = |ordering| i64::from(kind.apply_ordering(ordering));
    out.nums.clear();
    if a.integer() && b.integer() {
        match kind {
            OrderingKind::Lt => zip_nums(a, b, n, &mut out.nums, |x, y| i64::from(x < y)),
            OrderingKind::Gt => zip_nums(a, b, n, &mut out.nums, |x, y| i64::from(x > y)),
        }
    } else {
        for i in 0..n {
            out.nums
                .push(truth(small_rational::order_total(a.at(i), b.at(i))?));
        }
    }
    out.integers();
    Some(())
}

/// `EQ` over `n` lanes into `out`: a pair in lowest terms with its sign on
/// the numerator is a rational's unique form, so lanes are equal exactly
/// when their pairs are.
pub(crate) fn eq_into(a: Lanes, b: Lanes, n: usize, out: &mut Out) {
    out.nums.clear();
    if a.integer() && b.integer() {
        zip_nums(a, b, n, &mut out.nums, |x, y| i64::from(x == y));
    } else {
        out.nums
            .extend((0..n).map(|i| i64::from(a.at(i) == b.at(i))));
    }
    out.integers();
}

/// `MIN` / `MAX` over `n` lanes into `out`: the operand the order picks, the
/// left on a tie; `None` at a lane over zero.
pub(crate) fn extremum_into(max: bool, a: Lanes, b: Lanes, n: usize, out: &mut Out) -> Option<()> {
    if a.integer() && b.integer() {
        return integer_lanes(a, b, n, out, |x, y| {
            (if max { x.max(y) } else { x.min(y) }, false)
        });
    }
    pair_lanes(a, b, n, out, |x, y| {
        let take_right = if max {
            small_rational::order_total(x, y)? == std::cmp::Ordering::Less
        } else {
            small_rational::order_total(y, x)? == std::cmp::Ordering::Less
        };
        Some(if take_right { y } else { x })
    })
}

/// `NOT` of truth lanes into `out`.
pub(crate) fn not_into(a: Lanes, n: usize, out: &mut Out) {
    out.nums.clear();
    zip_nums(a, a, n, &mut out.nums, |x, _| i64::from(x == 0));
    out.integers();
}

/// `AND` of truth lanes into `out`.
pub(crate) fn and_into(a: Lanes, b: Lanes, n: usize, out: &mut Out) {
    out.nums.clear();
    zip_nums(a, b, n, &mut out.nums, |x, y| i64::from(x != 0 && y != 0));
    out.integers();
}

/// `SELECT`: lane by lane the first operand where the mask is true, else
/// the second.
pub(crate) fn select_into(t: Lanes, f: Lanes, mask: Lanes, n: usize, out: &mut Out) {
    out.clear();
    let (mut p, mut q, mut r) = (Vec::new(), Vec::new(), Vec::new());
    let m = mask.nums(n, &mut r);
    let pick = |x: &[i64], y: &[i64], out: &mut Vec<i64>| {
        out.extend(
            m.iter()
                .zip(x.iter().zip(y))
                .map(|(&m, (&x, &y))| if m != 0 { x } else { y }),
        );
    };
    pick(t.nums(n, &mut p), f.nums(n, &mut q), &mut out.nums);
    if t.integer() && f.integer() {
        return out.integers();
    }
    // The denominators chosen, read where they lie: a column's, or one
    // value for every lane (an integer column's implicit 1s, a constant's).
    fn den(lanes: Lanes<'_>, n: usize) -> Result<i64, &[i64]> {
        match lanes {
            Lanes::Columns {
                dens,
                integer: false,
                ..
            } => Err(&dens[..n]),
            Lanes::Columns { .. } => Ok(1),
            Lanes::Splat(_, d) => Ok(d),
        }
    }
    let choose = |m: i64, x: i64, y: i64| if m != 0 { x } else { y };
    // Most often every lane chosen is an integer — a fraction computed and
    // not chosen, as in the Collatz step — and then no column is written.
    let whole = match (den(t, n), den(f, n)) {
        (Err(x), Ok(1)) => m.iter().zip(x).all(|(&m, &x)| m == 0 || x == 1),
        (Ok(1), Err(y)) => m.iter().zip(y).all(|(&m, &y)| m != 0 || y == 1),
        _ => false,
    };
    if whole {
        return out.integers();
    }
    let dens = &mut out.dens;
    match (den(t, n), den(f, n)) {
        (Err(x), Err(y)) => dens.extend(
            m.iter()
                .zip(x.iter().zip(y))
                .map(|(&m, (&x, &y))| choose(m, x, y)),
        ),
        (Err(x), Ok(c)) => dens.extend(m.iter().zip(x).map(|(&m, &x)| choose(m, x, c))),
        (Ok(c), Err(y)) => dens.extend(m.iter().zip(y).map(|(&m, &y)| choose(m, c, y))),
        (Ok(c), Ok(e)) => dens.extend(m.iter().map(|&m| choose(m, c, e))),
    }
    out.settle();
}

/// `POW` lane by lane, as `quickened::small_power` answers it.
pub(crate) fn pow_into(a: Lanes, b: Lanes, n: usize, out: &mut Out) -> Option<()> {
    pair_lanes(a, b, n, out, crate::interpreter::quickened::small_power)
}

/// `a LT b` / `a GT b` lane by lane: the Vector `lift_lanes` builds, or
/// `None` for the general route. A lane over zero has no order to answer
/// (LANG.VALUES.EXACT, `small_rational::order_total`), so its lane is the
/// `domainMiss` NIL.
pub(crate) fn ordering(kind: OrderingKind, a: &Value, b: &Value) -> Option<Value> {
    let (a, b) = (Lanes::of(a)?, Lanes::of(b)?);
    let n = paired(a, b)?;
    // Integers compare straight into the truth column, with no column of
    // their own on the way.
    let truth = |x: i64, y: i64| kind.apply_ordering(x.cmp(&y));
    match (a, b) {
        (
            Lanes::Columns {
                nums: x,
                integer: true,
                ..
            },
            Lanes::Splat(y, 1),
        ) => {
            return hit(Some(Value::from_truths(x.iter().map(|&x| truth(x, y)))));
        }
        _ if a.integer() && b.integer() => {
            return hit(Some(Value::from_truths(
                (0..n).map(|i| truth(a.at(i).0, b.at(i).0)),
            )));
        }
        _ => {}
    }
    let mut out = Out::default();
    if ordering_into(kind, a, b, n, &mut out).is_some() {
        return hit(Some(Value::from_truths(out.nums.iter().map(|&t| t != 0))));
    }
    hit(Some(Value::from_vector(
        (0..n)
            .map(|i| match small_rational::order_total(a.at(i), b.at(i)) {
                Some(o) => Value::from_bool(kind.apply_ordering(o)),
                None => crate::interpreter::comparison::unordered_projection(),
            })
            .collect(),
    )))
}

/// Which rounding `rounded` applies.
#[derive(Clone, Copy)]
pub(crate) enum Rounding {
    Floor,
    /// Half away from zero, as `Fraction::round`.
    Round,
}

/// `FLOOR`/`ROUND` of every lane, or `None` for the general route. For a
/// Tensor that already was pure-integer, that is the operand itself.
pub(crate) fn rounded(rounding: Rounding, value: &Value) -> Option<Value> {
    let lanes = Lanes::of(value)?;
    let Lanes::Columns { nums, integer, .. } = lanes else {
        return None;
    };
    if integer {
        return hit(Some(value.clone()));
    }
    let mut out = Out::with_capacity(nums.len());
    rounded_into(rounding, lanes, nums.len(), &mut out)?;
    hit(Some(out.into_value()))
}
