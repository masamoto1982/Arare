//! Quantization and exact values (AR-LLR-010, refining AR-HLR-018 and
//! AR-HLR-026): how an exact rational literal, or a number at another width
//! and scale, becomes a number at a chosen width and scale, and how a number
//! reads back as the exact rational it denotes.

use crate::codes::{fit, fit_val, Val};
use crate::num::{rescale_code, round_div, Num, ScaleOverflow, SCALES};
use crate::width::Width;

impl Num {
    /// The exact rational `p/q` (`q ≥ 0`) quantized at `width` and exponent
    /// `scale`: the code is `p·2^(−scale) / q` rounded half to even, fitted
    /// into the width. `q = 0` is `1/0`, or `0/0` for `p = 0`, as in Ajisai.
    pub fn from_ratio(p: i64, q: i64, width: Width, scale: i32) -> Result<Num, ScaleOverflow> {
        assert!(q >= 0, "the sign of a ratio is carried by its numerator");
        if !SCALES.contains(&scale) {
            return Err(ScaleOverflow);
        }
        let val = match (p, q) {
            (0, 0) => Val::Bot,
            (_, 0) => Val::Inf,
            // |p|, q < 2^63 and the shift is at most 31: exact in i128.
            _ if scale <= 0 => fit(width, round_div((p as i128) << -scale, q as i128)),
            _ => fit(width, round_div(p as i128, (q as i128) << scale)),
        };
        Ok(Num { val, width, scale })
    }

    /// QUANTIZE: this number at `width` and exponent `scale`. Raising the
    /// exponent rounds half to even, as RESCALE does; lowering it shifts
    /// exactly. The result is fitted into `width`; the points pass through.
    pub fn quantize(self, width: Width, scale: i32) -> Result<Num, ScaleOverflow> {
        if !SCALES.contains(&scale) {
            return Err(ScaleOverflow);
        }
        let val = match self.val {
            Val::Fin(k) if scale >= self.scale => {
                fit(width, rescale_code(k, (scale - self.scale) as u32) as i128)
            }
            // At most a 62-bit shift of a code below 2^63: exact in i128.
            Val::Fin(k) => fit(width, (k as i128) << (self.scale - scale)),
            point => fit_val(width, point),
        };
        Ok(Num { val, width, scale })
    }

    /// The exact rational this number denotes, as `(numerator, denominator)`
    /// in lowest terms with a positive denominator: `1/0` is `(1, 0)` and
    /// `0/0` is `(0, 0)`, as Ajisai writes them.
    pub fn exact(self) -> (i128, i128) {
        match self.val {
            Val::Inf => (1, 0),
            Val::Bot => (0, 0),
            Val::Fin(k) if self.scale >= 0 => ((k as i128) << self.scale, 1),
            Val::Fin(k) => {
                let (mut n, mut e) = (k as i128, -self.scale);
                while e > 0 && n != 0 && n % 2 == 0 {
                    n /= 2;
                    e -= 1;
                }
                if n == 0 {
                    (0, 1)
                } else {
                    (n, 1i128 << e)
                }
            }
        }
    }
}
