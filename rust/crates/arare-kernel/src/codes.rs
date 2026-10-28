//! Values and their codes (AR-LLR-003, refining AR-HLR-002, AR-HLR-004 and
//! AR-HLR-009).

use crate::width::Width;

/// A value at some width: a finite code, `1/0` or `0/0`. A finite code of a
/// valid value lies in `[−(M−2), M−2]` for its width.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Val {
    Fin(i64),
    Inf,
    Bot,
}

/// The code of `v` at width `w`.
pub fn encode(w: Width, v: Val) -> i64 {
    match v {
        Val::Fin(k) => k,
        Val::Inf => w.inf_code(),
        Val::Bot => w.bot_code(),
    }
}

/// Whether `c` is a code width `w` produces: inside `[−M, M−1]` and not the
/// unused code `−(M−1)`.
pub fn is_code(w: Width, c: i64) -> bool {
    let c = c as i128;
    let m = w.half_range();
    -m <= c && c < m && c != -(m - 1)
}

/// The value of the code `c` at width `w`. `c` must satisfy [`is_code`].
pub fn decode(w: Width, c: i64) -> Val {
    if c == w.bot_code() {
        Val::Bot
    } else if c == w.inf_code() {
        Val::Inf
    } else {
        Val::Fin(c)
    }
}

/// The exact integer `x` at width `w`: itself when it lies in the finite
/// range, otherwise `1/0`. Nothing wraps around.
pub fn fit(w: Width, x: i128) -> Val {
    let max = w.max_finite() as i128;
    if -max <= x && x <= max {
        Val::Fin(x as i64)
    } else {
        Val::Inf
    }
}

/// [`fit`] lifted to values: the wheel points pass through.
pub fn fit_val(w: Width, v: Val) -> Val {
    match v {
        Val::Fin(k) => fit(w, k as i128),
        point => point,
    }
}
