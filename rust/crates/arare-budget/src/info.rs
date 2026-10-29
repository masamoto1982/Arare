//! What the checker knows of one number, and how each numeric Word carries it
//! (AR-LLR-019, refining AR-HLR-027 and AR-HLR-028).
//!
//! Each function here is the Lean definition of the same name in
//! `formal/Arare/Budget.lean`, which `Arare.Budget.budget_sound` proves sound,
//! computed with the checked rationals of [`crate::rat`]. Where an exact
//! rational would not fit in 128 bits, or a scale would leave ±31 (where the
//! kernel refuses the program), the function answers [`Info::Top`]: knowing
//! nothing is always sound. The Lean–Rust table (AR-VER-215) checks the two
//! agree on a grid where nothing overflows.

use crate::rat::Q;
use arare_kernel::{Num, Val, Width, MAX_SCALE};

/// The scale range of one number: `[smin, smax]`. A scalar has
/// `smin = smax`; a vector's lanes, or a loop's iterations, may differ.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Range {
    /// The exact value lies in `[lo, hi]`.
    pub lo: Q,
    pub hi: Q,
    /// The machine's value is within `err` of the exact one.
    pub err: Q,
    pub smin: i32,
    pub smax: i32,
}

/// Lean's `Info`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Info {
    /// Nothing is known.
    Top,
    /// Both meanings are `1/0`.
    Inf,
    /// Both meanings are `0/0`.
    Bot,
    /// Both meanings are finite, related as the range says.
    Num(Range),
}

/// The widest width's half-range, `2^63`: where arithmetic results are fitted.
pub const MW: i128 = 1i128 << 63;

fn rmax(a: Q, b: Q) -> Q {
    if a.le(b) {
        b
    } else {
        a
    }
}

fn rmin(a: Q, b: Q) -> Q {
    if a.le(b) {
        a
    } else {
        b
    }
}

fn mag(lo: Q, hi: Q) -> Q {
    rmax(lo.neg(), hi)
}

fn mmag(lo: Q, hi: Q, err: Q) -> Option<Q> {
    Some(rmax(lo.sub(err)?.neg(), hi.add(err)?))
}

fn nonzero(lo: Q, hi: Q, err: Q) -> Option<bool> {
    Some(Q::ZERO.lt(lo.sub(err)?) || hi.add(err)?.lt(Q::ZERO))
}

fn is_zero(lo: Q, hi: Q, err: Q) -> bool {
    lo.is_zero() && hi.is_zero() && err.is_zero()
}

fn low(lo: Q, hi: Q, err: Q) -> Option<Q> {
    let l = lo.sub(err)?;
    if Q::ZERO.lt(l) {
        Some(l)
    } else {
        Some(hi.add(err)?.neg())
    }
}

fn imul(l1: Q, h1: Q, l2: Q, h2: Q) -> Option<(Q, Q)> {
    let (a, b, c, d) = (l1.mul(l2)?, l1.mul(h2)?, h1.mul(l2)?, h1.mul(h2)?);
    Some((rmin(rmin(a, b), rmin(c, d)), rmax(rmax(a, b), rmax(c, d))))
}

/// A scale the kernel accepts; outside ±31 it refuses the program, so a
/// range that reaches there knows nothing useful.
fn scale(s: i64) -> Option<i32> {
    (s.unsigned_abs() <= MAX_SCALE as u64).then_some(s as i32)
}

/// Half a unit at scale `s`, Lean's `pow2 s / 2`, for a scale the kernel
/// accepts.
fn half_unit(s: i32) -> Q {
    Q::pow2(s - 1).expect("a scale within ±31 has a power of two in 128 bits")
}

fn num(lo: Q, hi: Q, err: Q, smin: i64, smax: i64) -> Option<Info> {
    Some(Info::Num(Range {
        lo,
        hi,
        err,
        smin: scale(smin)?,
        smax: scale(smax)?,
    }))
}

impl Info {
    /// Whether the checker bounds this number (Lean's `isNum`).
    pub fn is_num(self) -> bool {
        matches!(self, Info::Num(_))
    }
}

/// Lean's `guard`: fitting into a width of half-range `m`.
pub fn guard(m: i128, i: Info) -> Info {
    let Info::Num(r) = i else {
        return i;
    };
    let fits = || -> Option<bool> {
        let bound = Q::int(m - 2)?.mul(Q::pow2(r.smin)?)?;
        Some(mmag(r.lo, r.hi, r.err)?.le(bound))
    };
    if fits() == Some(true) {
        i
    } else {
        Info::Top
    }
}

/// Lean's `alit`: a literal written as the exact rational `p/q` (`q ≥ 0`),
/// quantized at `width` and exponent `e`. `code` is what the kernel
/// quantized it to.
pub fn lit(p: i64, q: i64, width: Width, e: i32, code: Val) -> Info {
    if q == 0 {
        return if p == 0 { Info::Bot } else { Info::Inf };
    }
    let known = || -> Option<Info> {
        let x = Q::new(p as i128, q as i128)?;
        // The kernel fits into the width; a code it could not fit is one
        // Lean's guard refuses.
        let Val::Fin(c) = code else {
            return Some(Info::Top);
        };
        let d = Q::int(c as i128)?.mul(Q::pow2(e)?)?.sub(x)?;
        let err = rmax(d.neg(), d);
        Some(guard(
            width.half_range(),
            num(x, x, err, e as i64, e as i64)?,
        ))
    };
    known().unwrap_or(Info::Top)
}

/// The literal a kernel number stands for when it was written as an integer.
pub fn int(n: Num) -> Info {
    match n.val() {
        Val::Fin(k) => lit(k, 1, n.width(), 0, n.val()),
        _ => Info::Top,
    }
}

/// Lean's `aneg`.
pub fn neg(i: Info) -> Info {
    match i {
        Info::Num(r) => Info::Num(Range {
            lo: r.hi.neg(),
            hi: r.lo.neg(),
            ..r
        }),
        i => i,
    }
}

/// Lean's `padd`.
fn padd(a: Info, b: Info) -> Option<Info> {
    use Info::*;
    Some(match (a, b) {
        (Bot, _) | (_, Bot) => Bot,
        (Inf, Inf) => Bot,
        (Inf, Num(_)) | (Num(_), Inf) => Inf,
        // The smaller of two scales the kernel accepts is one it accepts.
        (Num(x), Num(y)) => Num(Range {
            lo: x.lo.add(y.lo)?,
            hi: x.hi.add(y.hi)?,
            err: x.err.add(y.err)?,
            smin: x.smin.min(y.smin),
            smax: x.smax.min(y.smax),
        }),
        _ => Top,
    })
}

/// What is known when one operand of MUL is `1/0` and the other `r`.
fn inf_times(r: Range) -> Option<Info> {
    Some(if nonzero(r.lo, r.hi, r.err)? {
        Info::Inf
    } else if is_zero(r.lo, r.hi, r.err) {
        Info::Bot
    } else {
        Info::Top
    })
}

/// Lean's `pmul`.
fn pmul(a: Info, b: Info) -> Option<Info> {
    use Info::*;
    Some(match (a, b) {
        (Bot, _) | (_, Bot) => Bot,
        (Inf, Inf) => Inf,
        (Inf, Num(r)) | (Num(r), Inf) => inf_times(r)?,
        (Num(x), Num(y)) => {
            let (lo, hi) = imul(x.lo, x.hi, y.lo, y.hi)?;
            let err = mag(x.lo, x.hi)
                .add(x.err)?
                .mul(y.err)?
                .add(mag(y.lo, y.hi).mul(x.err)?)?;
            num(
                lo,
                hi,
                err,
                x.smin as i64 + y.smin as i64,
                x.smax as i64 + y.smax as i64,
            )?
        }
        _ => Top,
    })
}

/// Lean's `pdiv`.
fn pdiv(a: Info, b: Info) -> Option<Info> {
    use Info::*;
    Some(match (a, b) {
        (Bot, _) | (_, Bot) => Bot,
        (Inf, Inf) => Bot,
        (Inf, Num(_)) => Inf,
        (Num(x), Inf) => num(Q::ZERO, Q::ZERO, Q::ZERO, x.smin as i64, x.smax as i64)?,
        (Num(x), Num(y)) => {
            if is_zero(y.lo, y.hi, y.err) {
                inf_times(x)?
            } else if nonzero(y.lo, y.hi, y.err)? {
                let (lo, hi) = imul(x.lo, x.hi, y.hi.inv(), y.lo.inv())?;
                let li = low(y.lo, y.hi, y.err)?.inv();
                let err = half_unit(x.smax)
                    .add(x.err.mul(li)?)?
                    .add(mag(x.lo, x.hi).mul(y.err)?.mul(li.mul(li)?)?)?;
                num(lo, hi, err, x.smin as i64, x.smax as i64)?
            } else {
                Top
            }
        }
        _ => Top,
    })
}

/// ADD: Lean's `guard Mw (padd a b)`.
pub fn add(a: Info, b: Info) -> Info {
    guard(MW, padd(a, b).unwrap_or(Info::Top))
}

/// SUB: Lean's `guard Mw (padd a (aneg b))`.
pub fn sub(a: Info, b: Info) -> Info {
    add(a, neg(b))
}

/// MUL: Lean's `guard Mw (pmul a b)`.
pub fn mul(a: Info, b: Info) -> Info {
    guard(MW, pmul(a, b).unwrap_or(Info::Top))
}

/// DIV: Lean's `guard Mw (pdiv a b)`.
pub fn div(a: Info, b: Info) -> Info {
    guard(MW, pdiv(a, b).unwrap_or(Info::Top))
}

/// RESCALE by `s` bits: Lean's `arescale`.
pub fn rescale(i: Info, s: u32) -> Info {
    let Info::Num(r) = i else {
        return i;
    };
    let known = || -> Option<Info> {
        let smax = r.smax as i64 + s as i64;
        let err = r.err.add(half_unit(scale(smax)?))?;
        num(r.lo, r.hi, err, r.smin as i64 + s as i64, smax)
    };
    known().unwrap_or(Info::Top)
}

/// NARROW to `width`: Lean's `guard M`.
pub fn narrow(i: Info, width: Width) -> Info {
    guard(width.half_range(), i)
}

/// QUANTIZE at `width` and exponent `e`: Lean's `aquantize`.
pub fn quantize(i: Info, width: Width, e: i32) -> Info {
    let Info::Num(r) = i else {
        return i;
    };
    let known = || -> Option<Info> {
        let extra = if r.smin < e { half_unit(e) } else { Q::ZERO };
        num(r.lo, r.hi, r.err.add(extra)?, e as i64, e as i64)
    };
    guard(width.half_range(), known().unwrap_or(Info::Top))
}

/// What is known of a value that is one of two: Lean's `join`.
pub fn join(a: Info, b: Info) -> Info {
    use Info::*;
    match (a, b) {
        (Inf, Inf) => Inf,
        (Bot, Bot) => Bot,
        (Num(x), Num(y)) => Info::Num(Range {
            lo: rmin(x.lo, y.lo),
            hi: rmax(x.hi, y.hi),
            err: rmax(x.err, y.err),
            smin: x.smin.min(y.smin),
            smax: x.smax.max(y.smax),
        }),
        _ => Top,
    }
}

/// What is known of a value when the two meanings may each come from either
/// of two values: a SELECT whose condition may differ between them. Within
/// the hull of the two ranges, the machine's value from one side is at most
/// its own error plus the hull's width from the exact value of the other
/// (AR-HLR-029; argued, not proved in Lean — docs/quality/FORMAL_VERIFICATION.md).
pub fn cross(a: Info, b: Info) -> Info {
    use Info::*;
    match (a, b) {
        (Inf, Inf) => Inf,
        (Bot, Bot) => Bot,
        (Num(x), Num(y)) => {
            let known = || -> Option<Info> {
                let (lo, hi) = (rmin(x.lo, y.lo), rmax(x.hi, y.hi));
                let err = rmax(x.err, y.err).add(hi.sub(lo)?)?;
                num(
                    lo,
                    hi,
                    err,
                    x.smin.min(y.smin) as i64,
                    x.smax.max(y.smax) as i64,
                )
            };
            known().unwrap_or(Top)
        }
        _ => Top,
    }
}

/// The order of two numbers is the same in both meanings: certainly below,
/// certainly not below, or both exact (AR-HLR-029).
pub fn lt_agrees(a: Info, b: Info) -> Option<bool> {
    let (Info::Num(x), Info::Num(y)) = (a, b) else {
        return None;
    };
    if x.err.is_zero() && y.err.is_zero() {
        return Some(true);
    }
    let below = x.hi.add(x.err)?.lt(y.lo.sub(y.err)?);
    let above = y.hi.add(y.err)?.le(x.lo.sub(x.err)?);
    Some(below || above)
}

/// Equality of two numbers is the same in both meanings: two points, a point
/// and a finite number, two exact numbers, or two ranges apart (AR-HLR-029).
pub fn eq_agrees(a: Info, b: Info) -> bool {
    use Info::*;
    match (a, b) {
        (Inf | Bot, Inf | Bot) | (Inf | Bot, Num(_)) | (Num(_), Inf | Bot) => true,
        (Num(x), Num(y)) => {
            if x.err.is_zero() && y.err.is_zero() {
                return true;
            }
            let apart = || -> Option<bool> {
                Some(
                    x.hi.add(x.err)?.lt(y.lo.sub(y.err)?)
                        || y.hi.add(y.err)?.lt(x.lo.sub(x.err)?),
                )
            };
            apart() == Some(true)
        }
        _ => false,
    }
}

/// The error budget of a number: its `err`, `0` for a point both meanings
/// share, nothing when nothing is known.
pub fn budget(i: Info) -> Option<Q> {
    match i {
        Info::Num(r) => Some(r.err),
        Info::Inf | Info::Bot => Some(Q::ZERO),
        Info::Top => None,
    }
}

/// The exact integer a number certainly is in both meanings, if it is one:
/// an operand such as a RESCALE shift or a NARROW width must be the same in
/// both meanings for the checker to follow the Word.
pub fn known_int(i: Info) -> Option<i64> {
    match i {
        Info::Num(r) if r.lo == r.hi && r.err.is_zero() && r.lo.den() == 1 => {
            i64::try_from(r.lo.num()).ok()
        }
        _ => None,
    }
}
