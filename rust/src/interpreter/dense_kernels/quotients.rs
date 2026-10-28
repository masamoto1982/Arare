//! The column kernels that reduce by a gcd: `DIV` of integer lanes, and `MUL`
//! / `DIV` of rational lanes by one small integer. A child module of
//! `dense_kernels`, which holds the laws they answer and the tests that hold
//! them to the general route.
//!
//! A gcd is most of what these cost. Stein's walk is a chain of dependent
//! steps whose length varies lane to lane, so one lane at a time leaves the
//! machine waiting on each step and mispredicts every walk's last; where the
//! divisor is one small integer, the gcds are read from a table instead.

use super::lanes::{Lanes, Out};
use super::{floor_divmod, small_rational, ExactArithmeticSchema};
use crate::types::fraction::binary_gcd_u64;

/// The largest scalar divisor whose residue gcds `integer_quotients` tables:
/// a gcd of it fits a `u16`, and the table is a few kilobytes at most.
const GCD_TABLE_MAX: u64 = 4096;

/// Every integer's split against one modulus `m` with `1 <= m <=
/// GCD_TABLE_MAX`: `x = q·m + r` with `0 <= r < m` ([`floor_divmod`]), and
/// `g = gcd(m, r) = gcd(x, m)`, which divides both `m` and `r` — so `x / g` is
/// `q·(m/g) + r/g` exactly, two table reads and a multiply with no hardware
/// division.
struct Residues {
    m: i64,
    inv: f64,
    /// Per residue `r`: `m / g` and `r / g`.
    table: Vec<(u16, u16)>,
}

impl Residues {
    /// The table for `|k|`, when it is small enough to build and `lanes`
    /// enough to pay for building it.
    fn of(k: i64, lanes: usize) -> Option<Self> {
        let m = k.unsigned_abs();
        if !(1..=GCD_TABLE_MAX).contains(&m) || m as usize > lanes {
            return None;
        }
        let table = (0..m)
            .map(|r| {
                let g = binary_gcd_u64(m, r);
                ((m / g) as u16, (r / g) as u16)
            })
            .collect();
        Some(Self {
            m: m as i64,
            inv: 1.0 / m as f64,
            table,
        })
    }

    /// `(x / g, m / g)` for `g = gcd(x, m)`.
    #[inline(always)]
    fn reduce(&self, x: i64) -> (i64, i64) {
        let (q, r) = floor_divmod(x, self.m, self.inv);
        let (m_over_g, r_over_g) = self.table[r as usize];
        let (m_over_g, r_over_g) = (i64::from(m_over_g), i64::from(r_over_g));
        // `q·(m/g)` is at most `|x| / g` in magnitude, so nothing here can
        // overflow; when `g` is 1 the lane is its own quotient.
        if m_over_g == self.m {
            (x, m_over_g)
        } else {
            (q * m_over_g + r_over_g, m_over_g)
        }
    }
}

/// `a / b` on integer lanes: each quotient reduced by one gcd, with the sign
/// carried by the numerator; a zero divisor answers the dividend's sign over
/// zero. `None` for the one quotient a machine word cannot hold,
/// `i64::MIN / -1`.
pub(super) fn integer_quotients(a: Lanes, b: Lanes, n: usize, out: &mut Out) -> Option<()> {
    // A Tensor divided by one small integer (`7 DIV`, the common case) meets
    // only `|y|` residues, so everything a lane needs is read from a table
    // built once.
    if let (Lanes::Columns { nums, .. }, Lanes::Splat(y, _)) = (a, b) {
        if let Some(residues) = Residues::of(y, n) {
            return quotients_by_small_integer(nums, y, &residues, out);
        }
    }
    // The operand shapes are matched once, not per lane.
    match (a, b) {
        (Lanes::Columns { nums: x, .. }, Lanes::Columns { nums: y, .. }) => {
            quotient_lanes(n, |i| (x[i], y[i]), out)
        }
        (Lanes::Columns { nums: x, .. }, Lanes::Splat(y, _)) => {
            quotient_lanes(n, |i| (x[i], y), out)
        }
        (Lanes::Splat(x, _), Lanes::Columns { nums: y, .. }) => {
            quotient_lanes(n, |i| (x, y[i]), out)
        }
        (Lanes::Splat(x, _), Lanes::Splat(y, _)) => quotient_lanes(n, |_| (x, y), out),
    }
}

/// [`integer_quotients`] over `n` lanes read by `lane`, four at a time: four
/// independent gcd walks interleaved keep the machine busy where one walk
/// would leave it waiting on each step, and the loop that ends when the
/// longest of the four does mispredicts once per four lanes rather than once
/// per lane.
#[inline(always)]
fn quotient_lanes(n: usize, lane: impl Fn(usize) -> (i64, i64), out: &mut Out) -> Option<()> {
    let mut i = 0;
    while i + 4 <= n {
        let pairs = [lane(i), lane(i + 1), lane(i + 2), lane(i + 3)];
        let gcds = binary_gcd_x4(
            pairs.map(|(x, _)| x.unsigned_abs()),
            pairs.map(|(_, y)| y.unsigned_abs()),
        );
        for ((x, y), g) in pairs.into_iter().zip(gcds) {
            let (q, d) = reduced_quotient(x, y, g)?;
            out.push_pair(q, d);
        }
        i += 4;
    }
    for i in i..n {
        let (x, y) = lane(i);
        let (q, d) = reduced_quotient(x, y, binary_gcd_u64(x.unsigned_abs(), y.unsigned_abs()))?;
        out.push_pair(q, d);
    }
    out.settle();
    Some(())
}

/// `x / y` reduced by `g = gcd(|x|, |y|)`, with the sign carried by the
/// numerator; a zero divisor answers `1/0`, or `0/0` for a zero dividend. `None`
/// for the one quotient a machine word cannot hold, `i64::MIN / -1` (and for
/// `i64::MIN / i64::MIN`, whose gcd is `2^63`).
#[inline(always)]
fn reduced_quotient(x: i64, y: i64, g: u64) -> Option<(i64, i64)> {
    if y == 0 {
        return Some((i64::from(x != 0), 0));
    }
    let g = i64::try_from(g).ok()?;
    // `g` divides both exactly, and below 2^53 every operand and quotient is
    // a double, so the correctly rounded floating quotient is the exact one.
    // A hardware 64-bit division costs several times a floating one and
    // pipelines worse, and dividing every lane — by 1 for most — is cheaper
    // than a branch on whether `g` is 1 that goes either way.
    const EXACT: u64 = 1 << 53;
    let (q, d) = if x.unsigned_abs() < EXACT && y.unsigned_abs() < EXACT {
        let g = g as f64;
        ((x as f64 / g) as i64, (y as f64 / g) as i64)
    } else if g == 1 {
        (x, y)
    } else {
        (x / g, y / g)
    };
    if d < 0 {
        Some((q.checked_neg()?, d.checked_neg()?))
    } else {
        Some((q, d))
    }
}

/// [`binary_gcd_u64`] of four pairs at once, its steps interleaved. A pair
/// that a zero operand or one division settles is answered before the walk
/// and parked with `a = 0`; a parked lane steps along unchanged, every update
/// a mask rather than a branch, until the last lane is done.
///
/// Each step keeps `b` odd and shifts `a` odd at its top, so their difference
/// is even; its trailing zeros are counted off the wrapping difference, which
/// has the same low bits as the absolute one, so the count need not wait for
/// the compare that picks which operand to keep.
pub(crate) fn binary_gcd_x4(a: [u64; 4], b: [u64; 4]) -> [u64; 4] {
    let (mut a, mut b) = (a, b);
    let (mut shift, mut zeros) = ([0u32; 4], [0u32; 4]);
    for k in 0..4 {
        let (mut x, y) = if a[k] >= b[k] {
            (a[k], b[k])
        } else {
            (b[k], a[k])
        };
        if y == 0 {
            (a[k], b[k]) = (0, x);
            continue;
        }
        // As in `binary_gcd_u64`: a lopsided pair is first brought within
        // the smaller operand's width.
        if x >> 16 > y {
            x %= y;
            if x == 0 {
                (a[k], b[k]) = (0, y);
                continue;
            }
        }
        shift[k] = (x | y).trailing_zeros();
        zeros[k] = x.trailing_zeros();
        (a[k], b[k]) = (x, y >> y.trailing_zeros());
    }
    while a.iter().any(|&x| x != 0) {
        for k in 0..4 {
            // A parked lane's `a` is 0, which any shift leaves 0, and its
            // mask keeps `a` at 0 and `b` at its answer.
            let x = a[k].wrapping_shr(zeros[k]);
            let live = 0u64.wrapping_sub(u64::from(x != 0));
            zeros[k] = x.wrapping_sub(b[k]).trailing_zeros();
            let (diff, low) = (x.abs_diff(b[k]), x.min(b[k]));
            a[k] = diff & live;
            b[k] = (low & live) | (b[k] & !live);
        }
    }
    [0, 1, 2, 3].map(|k| b[k] << shift[k])
}

/// `x / y` for every lane `x` and one integer `y` with `1 <= |y| <=
/// GCD_TABLE_MAX`: [`integer_quotients`]'s answer from the residue table.
/// `None` for the one quotient a machine word cannot hold, `i64::MIN / -1`.
fn quotients_by_small_integer(
    nums: &[i64],
    y: i64,
    residues: &Residues,
    out: &mut Out,
) -> Option<()> {
    let negative = y < 0;
    for &x in nums {
        let (numerator, m_over_g) = residues.reduce(x);
        if negative {
            out.push_pair(numerator.checked_neg()?, m_over_g);
        } else {
            out.push_pair(numerator, m_over_g);
        }
    }
    out.settle();
    Some(())
}

/// `MUL` by, or `DIV` by, one small integer `k` of a column of rationals — the
/// shape a chain like `2 MUL 3 DIV` leaves on every lane after the first
/// `DIV` — with the one gcd each lane needs read from `k`'s residue table.
///
/// `n/d` is in lowest terms, so `(n/d)·k` needs only `g = gcd(k, d)`, giving
/// `(n·(k/g)) / (d/g)`, and `(n/d) / k` needs only `g = gcd(n, k)`, giving
/// `(n/g) / (d·(k/g))` with the sign moved to the numerator: the cross-
/// cancelled product and quotient of `small_rational::mul`/`div` with the
/// gcd the integer side makes trivial left out. Both are in lowest terms with
/// a positive denominator, a rational's unique form, so they are the pair the
/// general law answers. A lane over zero, or one whose product outgrows a
/// machine word, is handed to that law itself (`mul_total`/`div_total`), so
/// every lane answers exactly what the general loop would.
///
/// `None` for any other shape, or when a lane has no pair answer.
pub(super) fn scaled_by_integer(
    schema: ExactArithmeticSchema,
    a: Lanes,
    b: Lanes,
    out: &mut Out,
) -> Option<()> {
    use ExactArithmeticSchema::{Div, Mul};
    let (nums, dens, k, k_first) = match (schema, a, b) {
        (
            Mul | Div,
            Lanes::Columns {
                nums,
                dens,
                integer: false,
            },
            Lanes::Splat(k, 1),
        ) => (nums, dens, k, false),
        (
            Mul,
            Lanes::Splat(k, 1),
            Lanes::Columns {
                nums,
                dens,
                integer: false,
            },
        ) => (nums, dens, k, true),
        _ => return None,
    };
    let residues = Residues::of(k, nums.len())?;
    let divide = matches!(schema, Div);
    let general = |lane: (i64, i64)| {
        let k = (k, 1);
        match (divide, k_first) {
            (true, _) => small_rational::div_total(lane, k),
            (false, false) => small_rational::mul_total(lane, k),
            (false, true) => small_rational::mul_total(k, lane),
        }
    };
    let negative = k < 0;
    for (&n, &d) in nums.iter().zip(dens) {
        let pair = if d == 0 {
            None
        } else if divide {
            // `n/g` and `k/g`, `g = gcd(n, k)`.
            let (n_over_g, m_over_g) = residues.reduce(n);
            let n_over_g = if negative {
                n_over_g.checked_neg()
            } else {
                Some(n_over_g)
            };
            n_over_g.zip(d.checked_mul(m_over_g))
        } else {
            // `d/g` and `k/g`, `g = gcd(d, k)`.
            let (d_over_g, m_over_g) = residues.reduce(d);
            let k_over_g = if negative { -m_over_g } else { m_over_g };
            n.checked_mul(k_over_g).map(|n| (n, d_over_g))
        };
        let (rn, rd) = match pair {
            Some(pair) => pair,
            None => general((n, d))?,
        };
        out.push_pair(rn, rd);
    }
    out.settle();
    Some(())
}
