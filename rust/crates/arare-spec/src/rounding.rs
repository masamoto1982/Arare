//! AR-LLR-001. Mirrors `formal/Arare/Rounding.lean` (AR-HLR-007, AR-HLR-008).

/// `roundDivPos`: the quotient of `n` by a positive `d`, rounded half to even.
/// `div_euclid` and `rem_euclid` are Lean's `/` and `%` on `Int`.
pub fn round_div_pos(n: i128, d: i128) -> i128 {
    assert!(d > 0, "round_div_pos needs a positive divisor");
    let q = n.div_euclid(d);
    let r = n.rem_euclid(d);
    if 2 * r < d {
        q
    } else if 2 * r > d {
        q + 1
    } else if q.rem_euclid(2) == 0 {
        q
    } else {
        q + 1
    }
}

/// `roundDiv`: a negative divisor is reduced to a positive one; the value at
/// `d = 0` is never used (the wheel decides division by zero).
pub fn round_div(n: i128, d: i128) -> i128 {
    if d > 0 {
        round_div_pos(n, d)
    } else if d < 0 {
        round_div_pos(-n, -d)
    } else {
        0
    }
}

/// `rescale`: the code `k` raised by `s` scale bits, rounded half to even.
pub fn rescale(k: i128, s: u32) -> i128 {
    round_div_pos(k, 1i128 << s)
}
