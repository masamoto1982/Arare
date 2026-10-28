//! Column kernels: the element-wise Words over one-dimensional dense
//! Tensors, and every instruction of a column walk (`fused_block_columns`),
//! computed on the columns themselves. A kernel writes its lanes into an
//! [`Out`]; a Word wraps that in a Tensor, and a walk keeps one per register
//! and runs each instruction a chunk at a time.
//!
//! A dense Tensor stores its lanes as two `i64` columns — numerators and
//! denominators, each pair already in lowest terms. The general routes for
//! `DIV`, `FLOOR`/`ROUND` and `LT`/`GT` read those columns back out as a
//! `Vec<Fraction>` (`FlatTensor`) or as one boxed `Value` per lane
//! (`lift_lanes`), apply the law, and rebuild: about 200 ns a lane for what is
//! a handful of machine instructions; `ADD`/`SUB`/`MUL` the same.
//!
//! These kernels take the columns as they are, for the shapes whose answer is
//! fixed by the lanes alone: a Tensor beside a plain scalar, or two Tensors of
//! one length. They answer exactly the `Value` the general route builds — the
//! same dense Tensor (`from_fractions` of the same lanes is `from_columns` of
//! the same columns), or for a comparison the same Vector of Booleans — and
//! decline (`None`) wherever that route could answer anything else: a lane
//! that no longer fits a machine word (a boxed Vector), any other shape. The
//! charges are untouched, because the dispatcher made them before choosing a
//! route (`charge_binary_schema`); which route ran is unobservable
//! (LANG.AUTHORITY.FREEDOM). `dense_kernels_tests` holds the two equal.
//!
//! A lane over zero — one of the two points `1/0` and `0/0` — is a
//! lane like any other, in an operand or in the answer. The pair laws here
//! assume a positive denominator, so such a lane, and a zero divisor, are
//! answered by the `Fraction`'s own total arithmetic (`fraction_extended`)
//! and written back as the reduced pair it answers: `[ 6 6 ] [ 1 0 ] DIV`
//! holds `6/1` and `1/0`, and keeps its columns.

mod lanes;
mod lanewise;
mod quotients;

use crate::interpreter::arithmetic::ExactArithmeticSchema;
use crate::types::small_rational::{self, Pair};
use crate::types::Value;
pub(crate) use lanes::{paired, Lanes, Out};
pub(crate) use lanewise::*;
#[cfg(test)]
pub(crate) use quotients::binary_gcd_x4;
use quotients::{integer_quotients, scaled_by_integer};

#[cfg(test)]
thread_local! {
    static KERNEL_HITS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Kernel answers on this thread, for tests that pin which route ran.
#[cfg(test)]
pub(crate) fn kernel_hits_on_this_thread() -> u64 {
    KERNEL_HITS.with(|c| c.get())
}

fn hit<T>(answer: Option<T>) -> Option<T> {
    #[cfg(test)]
    if answer.is_some() {
        KERNEL_HITS.with(|c| c.set(c.get() + 1));
    }
    answer
}

/// `out[i] = f(a[i], b[i])` on integer lanes, with the overflow flags of
/// every lane gathered rather than checked one by one, so the loop is free
/// to vectorise. A set flag declines the whole operation.
#[inline(always)]
fn integer_lanes(
    a: Lanes,
    b: Lanes,
    n: usize,
    out: &mut Out,
    f: impl Fn(i64, i64) -> (i64, bool),
) -> Option<()> {
    // Every lane is overwritten below, so a reused buffer keeps whatever it
    // held, and a new one is allocated zeroed, nearly free however long.
    if out.nums.capacity() < n {
        out.nums = vec![0; n];
    } else {
        out.nums.truncate(n);
        out.nums.resize(n, 0);
    }
    let mut bad = false;
    match (a, b) {
        (Lanes::Columns { nums: x, .. }, Lanes::Columns { nums: y, .. }) => {
            for ((o, &x), &y) in out.nums.iter_mut().zip(x).zip(y) {
                let (v, f) = f(x, y);
                *o = v;
                bad |= f;
            }
        }
        (Lanes::Columns { nums: x, .. }, Lanes::Splat(y, _)) => {
            for (o, &x) in out.nums.iter_mut().zip(x) {
                let (v, f) = f(x, y);
                *o = v;
                bad |= f;
            }
        }
        (Lanes::Splat(x, _), Lanes::Columns { nums: y, .. }) => {
            for (o, &y) in out.nums.iter_mut().zip(y) {
                let (v, f) = f(x, y);
                *o = v;
                bad |= f;
            }
        }
        (Lanes::Splat(x, _), Lanes::Splat(y, _)) => {
            let (v, f) = f(x, y);
            out.nums.fill(v);
            bad = f;
        }
    }
    if bad {
        return None;
    }
    out.integers();
    Some(())
}

/// `out.push(f(a[i], b[i]))` on numerators, one loop per operand shape so
/// each is a plain zip.
#[inline(always)]
fn zip_nums(a: Lanes, b: Lanes, n: usize, out: &mut Vec<i64>, f: impl Fn(i64, i64) -> i64) {
    match (a, b) {
        (Lanes::Columns { nums: x, .. }, Lanes::Columns { nums: y, .. }) => {
            out.extend(x[..n].iter().zip(&y[..n]).map(|(&x, &y)| f(x, y)))
        }
        (Lanes::Columns { nums: x, .. }, Lanes::Splat(y, _)) => {
            out.extend(x[..n].iter().map(|&x| f(x, y)))
        }
        (Lanes::Splat(x, _), Lanes::Columns { nums: y, .. }) => {
            out.extend(y[..n].iter().map(|&y| f(x, y)))
        }
        (Lanes::Splat(x, _), Lanes::Splat(y, _)) => out.extend(std::iter::repeat_n(f(x, y), n)),
    }
}

/// `out[i] = f(a[i], b[i])` on any lanes, one at a time; `None` when a lane
/// has no machine-word answer.
#[inline(always)]
fn pair_lanes(
    a: Lanes,
    b: Lanes,
    n: usize,
    out: &mut Out,
    f: impl Fn(Pair, Pair) -> Option<Pair>,
) -> Option<()> {
    out.clear();
    out.reserve(n);
    for i in 0..n {
        let (rn, rd) = f(a.at(i), b.at(i))?;
        out.push(rn, rd);
    }
    Some(())
}

/// The floor quotient and remainder of `x` by `m >= 1`: `x = q·m + r` with
/// `0 <= r < m`, as `div_euclid` / `rem_euclid` answer them.
///
/// A hardware 64-bit division costs tens of cycles a lane, and on the column
/// kernels it was most of the work. Below 2^52 every integer is a double, and
/// `x · (1/m)` lands within one of the true quotient (`m >= 2` keeps it under
/// 2^51, where the two roundings together err by less than one), so the
/// estimate is corrected by at most a step each way against the exact
/// integer remainder. Anything wider takes the division.
#[inline(always)]
pub(crate) fn floor_divmod(x: i64, m: i64, inv: f64) -> (i64, i64) {
    const EXACT: u64 = 1 << 52;
    if m == 1 {
        return (x, 0);
    }
    if x.unsigned_abs() >= EXACT || m as u64 >= EXACT {
        return (x.div_euclid(m), x.rem_euclid(m));
    }
    let mut q = (x as f64 * inv) as i64;
    let mut r = x - q * m;
    while r < 0 {
        q -= 1;
        r += m;
    }
    while r >= m {
        q += 1;
        r -= m;
    }
    (q, r)
}

/// `n ± k` for a column of rationals and one integer `k`, either side:
/// `n/d ± k` is `(n ± k·d)/d`, already in lowest terms because
/// `gcd(n ± k·d, d) = gcd(n, d) = 1`, so a lane needs no gcd. A point over
/// zero is left as it was — `k - 1/0` is `1/0`, the negation of the one
/// unsigned point — which is what `add_total` answers for it. `None`
/// for anything else, or when a lane overflows.
fn shifted_by_integer(
    schema: ExactArithmeticSchema,
    a: Lanes,
    b: Lanes,
    out: &mut Out,
) -> Option<()> {
    let subtract = match schema {
        ExactArithmeticSchema::Add => false,
        ExactArithmeticSchema::Sub => true,
        _ => return None,
    };
    let (nums, dens, k, k_first) = match (a, b) {
        (
            Lanes::Columns {
                nums,
                dens,
                integer: false,
            },
            Lanes::Splat(k, 1),
        ) => (nums, dens, k, false),
        (
            Lanes::Splat(k, 1),
            Lanes::Columns {
                nums,
                dens,
                integer: false,
            },
        ) => (nums, dens, k, true),
        _ => return None,
    };
    out.nums.clear();
    out.nums.resize(nums.len(), 0);
    let mut bad = false;
    for ((o, &n), &d) in out.nums.iter_mut().zip(nums).zip(dens) {
        let (kd, f) = small_rational::overflowing_mul(k, d);
        let (v, g) = match (subtract, k_first) {
            (false, _) => n.overflowing_add(kd),
            (true, false) => n.overflowing_sub(kd),
            (true, true) => kd.overflowing_sub(n),
        };
        *o = if d == 0 { n } else { v };
        bad |= f | g;
    }
    if bad {
        return None;
    }
    out.dens.clear();
    out.dens.extend_from_slice(dens);
    out.settle();
    Some(())
}

/// `x / m` for integer lanes and a power of two `m`: `x` over `m`, both
/// shifted right by the twos `x` shares with `m`, which is their gcd — a
/// shift and a mask a lane, never a division.
fn halved(nums: &[i64], m: i64, out: &mut Out) {
    let s = m.trailing_zeros();
    let mut odd = 0;
    if s == 1 {
        // `x / 2`: an even lane halves, an odd one is `x/2` as it stands.
        out.nums.extend(nums.iter().map(|&x| x >> (1 - (x & 1))));
        out.dens.extend(nums.iter().map(|&x| {
            odd |= x & 1;
            1 + (x & 1)
        }));
    } else {
        let twos = |x: i64| x.trailing_zeros().min(s);
        out.nums.extend(nums.iter().map(|&x| x >> twos(x)));
        out.dens.extend(nums.iter().map(|&x| {
            let d = m >> twos(x);
            odd |= d - 1;
            d
        }));
    }
    if odd == 0 {
        out.integers();
    } else {
        out.integer = false;
    }
}

/// `a schema b` over `n` lanes into `out`, or `None` when a lane has no
/// machine-word answer.
pub(crate) fn arith_into(
    schema: ExactArithmeticSchema,
    a: Lanes,
    b: Lanes,
    n: usize,
    out: &mut Out,
) -> Option<()> {
    if a.integer() && b.integer() {
        let done = match schema {
            ExactArithmeticSchema::Add => integer_lanes(a, b, n, out, i64::overflowing_add),
            ExactArithmeticSchema::Sub => integer_lanes(a, b, n, out, i64::overflowing_sub),
            ExactArithmeticSchema::Mul => {
                integer_lanes(a, b, n, out, small_rational::overflowing_mul)
            }
            ExactArithmeticSchema::Div => {
                out.clear();
                out.reserve(n);
                if let (Lanes::Columns { nums, .. }, Lanes::Splat(m, _)) = (a, b) {
                    if m > 0 && m & (m - 1) == 0 {
                        halved(nums, m, out);
                        return Some(());
                    }
                }
                return integer_quotients(a, b, n, out);
            }
        };
        if done.is_some() {
            return Some(());
        }
    }
    if shifted_by_integer(schema, a, b, out).is_some() {
        return Some(());
    }
    out.clear();
    out.reserve(n);
    if scaled_by_integer(schema, a, b, out).is_some() {
        return Some(());
    }
    // A lane over zero, or a zero divisor, is the `Fraction`'s own law
    // (`small_rational`'s total forms): its answer is one of the points or
    // zero, a small pair. A lane that outgrows a machine word is the general
    // route's to make.
    match schema {
        ExactArithmeticSchema::Add => {
            pair_lanes(a, b, n, out, |x, y| small_rational::add_total(x, y, false))
        }
        ExactArithmeticSchema::Sub => {
            pair_lanes(a, b, n, out, |x, y| small_rational::add_total(x, y, true))
        }
        ExactArithmeticSchema::Mul => pair_lanes(a, b, n, out, small_rational::mul_total),
        ExactArithmeticSchema::Div => pair_lanes(a, b, n, out, small_rational::div_total),
    }
}

/// The floor of a pair; a point over zero is its own floor.
#[inline(always)]
pub(crate) fn floor_pair((n, d): Pair) -> Pair {
    if d == 0 || d == 1 {
        (n, d)
    } else {
        (n.div_euclid(d), 1)
    }
}

/// `a b DIV FLOOR` over `n` lanes into `out`. On integers by one positive
/// integer it is a shift or a reciprocal multiply, never a gcd.
pub(crate) fn floor_div_into(a: Lanes, b: Lanes, n: usize, out: &mut Out) -> Option<()> {
    if let (
        Lanes::Columns {
            nums,
            integer: true,
            ..
        },
        Lanes::Splat(m, 1),
    ) = (a, b)
    {
        if m > 0 {
            out.nums.clear();
            if m & (m - 1) == 0 {
                let shift = m.trailing_zeros();
                out.nums.extend(nums.iter().map(|&x| x >> shift));
            } else {
                let inv = 1.0 / m as f64;
                out.nums
                    .extend(nums.iter().map(|&x| floor_divmod(x, m, inv).0));
            }
            out.integers();
            return Some(());
        }
    }
    pair_lanes(a, b, n, out, |x, y| {
        Some(floor_pair(small_rational::div_total(x, y)?))
    })
}

/// `a schema b` lane by lane, or `None` for the general route.
pub(crate) fn arithmetic(schema: ExactArithmeticSchema, a: &Value, b: &Value) -> Option<Value> {
    let (a, b) = (Lanes::of(a)?, Lanes::of(b)?);
    let n = paired(a, b)?;
    let mut out = Out::default();
    arith_into(schema, a, b, n, &mut out)?;
    hit(Some(out.into_value()))
}

/// [`fold_integers`] for the one arithmetic Word the block applies.
#[inline(always)]
pub(crate) fn fold_op(
    schema: ExactArithmeticSchema,
    flipped: bool,
    acc: i64,
    x: Lanes,
    n: usize,
    each: impl FnMut(i64),
) -> Option<i64> {
    match (schema, flipped) {
        (ExactArithmeticSchema::Add, _) => fold_integers(acc, x, n, each, i64::overflowing_add),
        (ExactArithmeticSchema::Mul, _) => {
            fold_integers(acc, x, n, each, small_rational::overflowing_mul)
        }
        (_, false) => fold_integers(acc, x, n, each, i64::overflowing_sub),
        (_, true) => fold_integers(acc, x, n, each, |a, x| x.overflowing_sub(a)),
    }
}

/// `acc = op(acc, x)` over `n` integer lanes, in a loop with nothing else in
/// it; `None` on an overflow.
#[inline(always)]
fn fold_integers(
    mut acc: i64,
    x: Lanes,
    n: usize,
    mut each: impl FnMut(i64),
    op: impl Fn(i64, i64) -> (i64, bool),
) -> Option<i64> {
    let lanes = match x {
        Lanes::Columns { nums, .. } => &nums[..n],
        Lanes::Splat(x, _) => {
            for _ in 0..n {
                let (v, bad) = op(acc, x);
                if bad {
                    return None;
                }
                acc = v;
                each(v);
            }
            return Some(acc);
        }
    };
    for &x in lanes {
        let (v, bad) = op(acc, x);
        if bad {
            return None;
        }
        acc = v;
        each(v);
    }
    Some(acc)
}
