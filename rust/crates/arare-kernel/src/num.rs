//! Numbers with widths and scales (AR-LLR-004 to AR-LLR-007, refining
//! AR-HLR-003 to AR-HLR-011).
//!
//! A [`Num`] is a value, its width and its scale exponent `e`: its value is
//! code × 2^e. Exponents stay within ±[`MAX_SCALE`], so aligning two numbers
//! shifts by at most 62 bits and every intermediate fits an `i128` exactly.

use crate::codes::{fit, fit_val, Val};
use crate::truth::Truth;
use crate::width::Width;
use core::cmp::Ordering;

/// The largest scale exponent magnitude (AR-LLR-004).
pub const MAX_SCALE: i32 = 31;

const SCALES: core::ops::RangeInclusive<i32> = -MAX_SCALE..=MAX_SCALE;

/// An operation whose result scale would leave ±[`MAX_SCALE`]: an error in
/// the program, not a value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScaleOverflow;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Num {
    val: Val,
    width: Width,
    scale: i32,
}

/// `max · 2^d` as a bound on magnitudes; `d ≤ 62` keeps it below `2^125`.
fn shifted_bound(w: Width, d: u32) -> u128 {
    (w.max_finite() as u128) << d
}

/// Round half to even of `n / d`, `d ≠ 0` (AR-LLR-005). Written on the
/// truncating quotient, a second formulation beside the specification's
/// Euclidean one.
fn round_div(n: i128, d: i128) -> i128 {
    let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
    let q = n / d;
    let twice = 2 * (n % d).abs();
    let away = q + n.signum();
    if twice < d {
        q
    } else if twice > d || q % 2 != 0 {
        away
    } else {
        q
    }
}

/// `k / 2^s` rounded half to even, by shifting (AR-LLR-006).
pub(crate) fn rescale_code(k: i64, s: u32) -> i64 {
    if s == 0 {
        return k;
    }
    if s >= 64 {
        // |k| < 2^63 ≤ 2^s / 2: below half a unit.
        return 0;
    }
    let q = k >> s;
    let r = k & ((1i64 << s) - 1);
    let half = 1i64 << (s - 1);
    if r < half || (r == half && q % 2 == 0) {
        q
    } else {
        q + 1
    }
}

impl Num {
    /// A number of the given value, width and scale, when the value is one
    /// the width holds and the scale is in range.
    pub fn new(val: Val, width: Width, scale: i32) -> Option<Num> {
        let fits = match val {
            Val::Fin(k) => k.unsigned_abs() <= width.max_finite() as u64,
            _ => true,
        };
        // A range check, not `scale.abs()`: `i32::MIN.abs()` overflows, and in a
        // release build wraps to a negative number that would pass (found by
        // the Kani harness `add_w64_any_scales_no_overflow`).
        (fits && SCALES.contains(&scale)).then_some(Num { val, width, scale })
    }

    /// An integer literal: scale 0 and the narrowest width that holds it
    /// (AR-LLR-002). `None` beyond the widest finite range.
    pub fn int(k: i64) -> Option<Num> {
        let mag = k.unsigned_abs() as u128;
        if mag > Width::W64.max_finite() as u128 {
            return None;
        }
        Some(Num {
            val: Val::Fin(k),
            width: Width::narrowest_for(mag),
            scale: 0,
        })
    }

    pub fn val(self) -> Val {
        self.val
    }

    pub fn width(self) -> Width {
        self.width
    }

    pub fn scale(self) -> i32 {
        self.scale
    }

    /// ADD (AR-LLR-004): align to the smaller exponent, which is exact, then
    /// add; the result width holds the sum of the aligned operand ranges.
    pub fn add(self, o: Num) -> Num {
        let e = self.scale.min(o.scale);
        let (da, db) = ((self.scale - e) as u32, (o.scale - e) as u32);
        let width =
            Width::narrowest_for(shifted_bound(self.width, da) + shifted_bound(o.width, db));
        let val = match (self.val, o.val) {
            (Val::Bot, _) | (_, Val::Bot) | (Val::Inf, Val::Inf) => Val::Bot,
            (Val::Inf, Val::Fin(_)) | (Val::Fin(_), Val::Inf) => Val::Inf,
            (Val::Fin(a), Val::Fin(b)) => fit(width, ((a as i128) << da) + ((b as i128) << db)),
        };
        Num {
            val,
            width,
            scale: e,
        }
    }

    /// NEG: the finite range is symmetric, so the width does not change;
    /// `1/0` is its own negation.
    pub fn neg(self) -> Num {
        let val = match self.val {
            Val::Fin(k) => Val::Fin(-k),
            point => point,
        };
        Num { val, ..self }
    }

    /// SUB is ADD of the negation.
    pub fn sub(self, o: Num) -> Num {
        self.add(o.neg())
    }

    /// MUL (AR-LLR-004): exponents add; the result width holds the product
    /// of the operand ranges.
    pub fn mul(self, o: Num) -> Result<Num, ScaleOverflow> {
        let scale = self.scale + o.scale;
        if !SCALES.contains(&scale) {
            return Err(ScaleOverflow);
        }
        let width =
            Width::narrowest_for(self.width.max_finite() as u128 * o.width.max_finite() as u128);
        let zero_times_inf = |k: i64| if k == 0 { Val::Bot } else { Val::Inf };
        let val = match (self.val, o.val) {
            (Val::Bot, _) | (_, Val::Bot) => Val::Bot,
            (Val::Inf, Val::Inf) => Val::Inf,
            (Val::Inf, Val::Fin(k)) | (Val::Fin(k), Val::Inf) => zero_times_inf(k),
            (Val::Fin(a), Val::Fin(b)) => fit(width, a as i128 * b as i128),
        };
        Ok(Num { val, width, scale })
    }

    /// DIV (AR-LLR-005): the quotient at the dividend's scale, rounded half to
    /// even; division by zero and by `1/0` follow the wheel.
    pub fn div(self, o: Num) -> Num {
        // value = (a·2^ea) / (b·2^eb), at scale ea: code = round(a·2^(−eb) / b).
        let (p, q) = if o.scale < 0 {
            ((-o.scale) as u32, 0)
        } else {
            (0, o.scale as u32)
        };
        let width = Width::narrowest_for(shifted_bound(self.width, p));
        let val = match (self.val, o.val) {
            (Val::Bot, _) | (_, Val::Bot) | (Val::Inf, Val::Inf) => Val::Bot,
            (Val::Inf, Val::Fin(_)) => Val::Inf,
            (Val::Fin(_), Val::Inf) => Val::Fin(0),
            (Val::Fin(0), Val::Fin(0)) => Val::Bot,
            (Val::Fin(_), Val::Fin(0)) => Val::Inf,
            (Val::Fin(a), Val::Fin(b)) => fit(width, round_div((a as i128) << p, (b as i128) << q)),
        };
        Num {
            val,
            width,
            scale: self.scale,
        }
    }

    /// RESCALE by `s` bits (AR-LLR-006): the exponent rises by `s` and the
    /// code is rounded half to even; the width stays, since the magnitude
    /// cannot grow.
    pub fn rescale(self, s: u32) -> Result<Num, ScaleOverflow> {
        let scale = self.scale as i64 + s as i64;
        if scale > MAX_SCALE as i64 {
            return Err(ScaleOverflow);
        }
        let val = match self.val {
            Val::Fin(k) => Val::Fin(rescale_code(k, s)),
            point => point,
        };
        Ok(Num {
            val,
            width: self.width,
            scale: scale as i32,
        })
    }

    /// NARROW to `width` (AR-LLR-006): a finite number outside its range
    /// becomes `1/0`.
    pub fn narrow(self, width: Width) -> Num {
        Num {
            val: fit_val(width, self.val),
            width,
            scale: self.scale,
        }
    }

    /// The order of two finite numbers by value, across scales (AR-LLR-007).
    fn cmp_fin(a: i64, ea: i32, b: i64, eb: i32) -> Ordering {
        let e = ea.min(eb);
        ((a as i128) << (ea - e)).cmp(&((b as i128) << (eb - e)))
    }

    /// EQ (AR-LLR-007): by denotation, always decided.
    pub fn eq(self, o: Num) -> Truth {
        let equal = match (self.val, o.val) {
            (Val::Fin(a), Val::Fin(b)) => {
                Num::cmp_fin(a, self.scale, b, o.scale) == Ordering::Equal
            }
            (x, y) => x == y,
        };
        if equal {
            Truth::True
        } else {
            Truth::False
        }
    }

    /// LT (AR-LLR-007): over finite numbers by value; UNKNOWN at a point.
    pub fn lt(self, o: Num) -> Truth {
        match (self.val, o.val) {
            (Val::Fin(a), Val::Fin(b)) => {
                if Num::cmp_fin(a, self.scale, b, o.scale) == Ordering::Less {
                    Truth::True
                } else {
                    Truth::False
                }
            }
            _ => Truth::Unknown,
        }
    }
}
