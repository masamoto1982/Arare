//! Numbers as Arrow decimals (AR-LLR-040).
//!
//! A number of width N and exponent e has the value code × 2^e. As a decimal
//! of scale s = max(0, −e) its unscaled value is code × 5^(−e) when e < 0
//! (since 2^e = 5^(−e) / 10^(−e)) and code × 2^e otherwise; both are exact.
//! An int64 code times 5^31 reaches about 2^135, past `i128`, so the unscaled
//! values are computed as a sign and a 256-bit magnitude.

use arare_kernel::Width;

/// A 256-bit magnitude, least significant word first.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct U256(pub [u64; 4]);

impl U256 {
    pub fn from_u128(x: u128) -> U256 {
        U256([x as u64, (x >> 64) as u64, 0, 0])
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 4]
    }

    /// `self × m`, or `None` past 256 bits.
    pub fn mul_small(self, m: u64) -> Option<U256> {
        let mut out = [0u64; 4];
        let mut carry = 0u128;
        for (o, w) in out.iter_mut().zip(self.0) {
            let t = w as u128 * m as u128 + carry;
            *o = t as u64;
            carry = t >> 64;
        }
        (carry == 0).then_some(U256(out))
    }

    /// `(self / d, self % d)`, `d ≠ 0`.
    pub fn divrem_small(self, d: u64) -> (U256, u64) {
        let mut out = [0u64; 4];
        let mut rem = 0u128;
        for i in (0..4).rev() {
            let t = (rem << 64) | self.0[i] as u128;
            out[i] = (t / d as u128) as u64;
            rem = t % d as u128;
        }
        (U256(out), rem as u64)
    }

    /// The value, when it is below 2^127.
    pub fn to_u128(self) -> Option<u128> {
        (self.0[2] == 0 && self.0[3] == 0 && self.0[1] >> 63 == 0)
            .then_some(self.0[0] as u128 | (self.0[1] as u128) << 64)
    }

    /// The number of decimal digits, at least 1.
    pub fn digits(self) -> u32 {
        let (mut x, mut n) = (self, 1);
        loop {
            x = x.divrem_small(10).0;
            if x.is_zero() {
                return n;
            }
            n += 1;
        }
    }
}

/// A signed 256-bit integer as sign and magnitude.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Signed {
    pub negative: bool,
    pub mag: U256,
}

impl Signed {
    /// Two's complement in `n` (16 or 32) little-endian bytes.
    pub fn to_le(self, n: usize) -> Vec<u8> {
        let mut words = self.mag.0;
        if self.negative {
            let mut carry = true;
            for w in words.iter_mut() {
                let (v, c) = (!*w).overflowing_add(carry as u64);
                *w = v;
                carry = c;
            }
        }
        words.iter().flat_map(|w| w.to_le_bytes()).take(n).collect()
    }

    /// Read two's complement from 16 or 32 little-endian bytes.
    pub fn from_le(b: &[u8]) -> Signed {
        let negative = b[b.len() - 1] & 0x80 != 0;
        let fill = if negative { 0xff } else { 0 };
        let mut words = [0u64; 4];
        for (i, w) in words.iter_mut().enumerate() {
            let mut le = [fill; 8];
            for (j, byte) in le.iter_mut().enumerate() {
                if let Some(v) = b.get(8 * i + j) {
                    *byte = *v;
                }
            }
            *w = u64::from_le_bytes(le);
        }
        if negative {
            let mut carry = true;
            for w in words.iter_mut() {
                let (v, c) = (!*w).overflowing_add(carry as u64);
                *w = v;
                carry = c;
            }
        }
        Signed {
            negative,
            mag: U256(words),
        }
    }
}

/// `m × 5^k`; at most `2^63 · 5^31 < 2^136`, so it never overflows.
fn times_five(m: U256, k: u32) -> U256 {
    (0..k).fold(m, |x, _| x.mul_small(5).expect("below 2^136"))
}

/// The decimal scale of exponent `e`.
pub fn decimal_scale(e: i32) -> i32 {
    (-e).max(0)
}

/// The unscaled decimal value of `code` at exponent `e`.
pub fn unscaled(code: i64, e: i32) -> Signed {
    let mag = U256::from_u128(code.unsigned_abs() as u128);
    let mag = if e < 0 {
        times_five(mag, e.unsigned_abs())
    } else {
        // e ≤ 31 and |code| < 2^63: below 2^94.
        U256::from_u128((code.unsigned_abs() as u128) << e)
    };
    Signed {
        negative: code < 0,
        mag,
    }
}

/// The code whose unscaled decimal value at exponent `e` is `x`, when `x` is
/// one: an exact multiple of 5^(−e) or 2^e whose quotient fits an `i64`.
pub fn code_of(x: Signed, e: i32) -> Option<i64> {
    let mut mag = x.mag;
    let (d, k) = if e < 0 {
        (5, e.unsigned_abs())
    } else {
        (2, e as u32)
    };
    for _ in 0..k {
        let (q, r) = mag.divrem_small(d);
        if r != 0 {
            return None;
        }
        mag = q;
    }
    let m = mag.to_u128()?;
    let m = i64::try_from(m).ok()?;
    Some(if x.negative { -m } else { m })
}

/// The decimal precision of width `w` at exponent `e`: the digits of the
/// largest finite magnitude's unscaled value.
pub fn precision(w: Width, e: i32) -> u32 {
    unscaled(w.max_finite(), e).mag.digits()
}

/// The Arrow decimal width in bits that holds `precision` digits.
pub fn bit_width(precision: u32) -> i32 {
    if precision <= 38 {
        128
    } else {
        256
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
pub fn base64(data: &[u8]) -> String {
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The bytes of standard base64 with padding, or `None` when it is not that.
pub fn unbase64(text: &str) -> Option<Vec<u8>> {
    let t = text.as_bytes();
    if t.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(t.len() / 4 * 3);
    for (q, quad) in t.chunks(4).enumerate() {
        let last = q + 1 == t.len() / 4;
        let pad = quad.iter().rev().take_while(|c| **c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for (i, c) in quad.iter().enumerate() {
            let v = if i >= 4 - pad {
                0
            } else {
                B64.iter().position(|x| x == c)? as u32
            };
            n = n << 6 | v;
        }
        out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
    }
    Some(out)
}
