//! What compiled code runs with (AR-LLR-023, refining AR-HLR-030 and
//! AR-HLR-031): the columns and scalars it takes, the values it answers, and
//! the reference interpreter it falls back to.
//!
//! A [`Column`] records, when it is made, whether any lane holds a wheel point
//! and whether any lane is NIL, as Arrow records a null count. Compiled code
//! reads those two facts once, and only when both are false runs its fast
//! path, which computes on plain integer codes; anything else — a point, a
//! NIL, a divisor that turns out zero, a result that turns out not to fit —
//! hands the whole program to the reference interpreter ([`fallback`]), so the
//! compiled route answers what the interpreter answers (AR-HLR-030).

use arare_front::parse;
use arare_interp::{run_on, Reason, Value};
use arare_kernel::{decode, encode, is_code, Num, Val};

/// `arare_kernel::rescale_code` on a code that fits `i32`: the same shift
/// and the same tie to even, in a type that vectorizes twice as wide
/// (AR-LLR-026).
#[inline]
pub fn rescale_i32(k: i32, s: u32) -> i32 {
    if s == 0 {
        return k;
    }
    if s >= 32 {
        // |k| < 2^31 ≤ 2^s / 2: below half a unit.
        return 0;
    }
    let q = k >> s;
    // The mask in u32: for s = 31 it is 2^31 − 1, which i32 arithmetic would overflow to reach.
    let r = k & (((1u32 << s) - 1) as i32);
    let half = 1i32 << (s - 1);
    if r < half || (r == half && q % 2 == 0) {
        q
    } else {
        q + 1
    }
}

// What generated code names, so that it needs no other crate.
pub use arare_interp::RunError;
pub use arare_kernel::{rescale_code, round_div, Width};

/// A column's codes, in the integer type of its width.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Codes {
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
}

impl Codes {
    pub fn len(&self) -> usize {
        match self {
            Codes::I8(v) => v.len(),
            Codes::I16(v) => v.len(),
            Codes::I32(v) => v.len(),
            Codes::I64(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, i: usize) -> i64 {
        match self {
            Codes::I8(v) => v[i] as i64,
            Codes::I16(v) => v[i] as i64,
            Codes::I32(v) => v[i] as i64,
            Codes::I64(v) => v[i],
        }
    }

    fn width(&self) -> Width {
        match self {
            Codes::I8(_) => Width::W8,
            Codes::I16(_) => Width::W16,
            Codes::I32(_) => Width::W32,
            Codes::I64(_) => Width::W64,
        }
    }

    /// Codes of `width` for `vals`, a NIL lane taking code 0.
    fn of(width: Width, vals: impl Iterator<Item = i64>) -> Codes {
        match width {
            Width::W8 => Codes::I8(vals.map(|c| c as i8).collect()),
            Width::W16 => Codes::I16(vals.map(|c| c as i16).collect()),
            Width::W32 => Codes::I32(vals.map(|c| c as i32).collect()),
            Width::W64 => Codes::I64(vals.collect()),
        }
    }
}

/// Why a column cannot be made.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColumnError {
    /// The codes are not of the column's width.
    WidthMismatch,
    /// A code no width produces: the unused code −(M−1).
    NotACode(usize),
    /// The validity bitmap does not cover the lanes.
    ValidityLength,
    /// The scale exponent is outside ±31.
    Scale,
}

/// A vector of numbers of one width and scale, in Arrow's layout: codes and
/// an optional validity bitmap, lane `i` at bit `i % 64` of word `i / 64`, a
/// cleared bit for a NIL lane.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Column {
    width: Width,
    scale: i32,
    codes: Codes,
    valid: Option<Vec<u64>>,
    points: bool,
    nils: bool,
}

impl Column {
    /// A column from its codes, checked once: every code must be one its
    /// width produces. Whether it holds a wheel point or a NIL lane is
    /// recorded here, so that compiled code need not look at a lane to know.
    pub fn new(
        width: Width,
        scale: i32,
        codes: Codes,
        valid: Option<Vec<u64>>,
    ) -> Result<Column, ColumnError> {
        if codes.width() != width {
            return Err(ColumnError::WidthMismatch);
        }
        if Num::new(Val::Fin(0), width, scale).is_none() {
            return Err(ColumnError::Scale);
        }
        let n = codes.len();
        if valid.as_ref().is_some_and(|v| v.len() != n.div_ceil(64)) {
            return Err(ColumnError::ValidityLength);
        }
        let is_valid = |i: usize| {
            valid
                .as_ref()
                .is_none_or(|v| v[i / 64] >> (i % 64) & 1 == 1)
        };
        let (mut points, mut nils) = (false, false);
        for i in 0..n {
            let c = codes.get(i);
            if !is_valid(i) {
                nils = true;
                continue;
            }
            if !is_code(width, c) {
                return Err(ColumnError::NotACode(i));
            }
            points |= c.unsigned_abs() > width.max_finite() as u64;
        }
        Ok(Column {
            width,
            scale,
            codes,
            valid: if nils { valid } else { None },
            points,
            nils,
        })
    }

    /// A column of values, `None` for a NIL lane.
    pub fn of_values(
        width: Width,
        scale: i32,
        vals: &[Option<Val>],
    ) -> Result<Column, ColumnError> {
        let codes = Codes::of(
            width,
            vals.iter().map(|v| v.map_or(0, |v| encode(width, v))),
        );
        let valid = vals.iter().any(Option::is_none).then(|| {
            let mut words = vec![0u64; vals.len().div_ceil(64)];
            for (i, v) in vals.iter().enumerate() {
                if v.is_some() {
                    words[i / 64] |= 1 << (i % 64);
                }
            }
            words
        });
        Column::new(width, scale, codes, valid)
    }

    pub fn width(&self) -> Width {
        self.width
    }

    pub fn scale(&self) -> i32 {
        self.scale
    }

    pub fn len(&self) -> usize {
        self.codes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }

    /// Whether some lane holds `1/0` or `0/0`.
    pub fn has_points(&self) -> bool {
        self.points
    }

    /// Whether some lane is NIL.
    pub fn has_nils(&self) -> bool {
        self.nils
    }

    /// Lane `i`: its value, or `None` when it is NIL.
    pub fn lane(&self, i: usize) -> Option<Val> {
        let valid = self
            .valid
            .as_ref()
            .is_none_or(|v| v[i / 64] >> (i % 64) & 1 == 1);
        valid.then(|| decode(self.width, self.codes.get(i)))
    }

    /// The codes, when every lane is a finite number of `width` at `scale`:
    /// what the fast path computes on.
    pub fn plain_codes(&self, width: Width, scale: i32) -> Option<&Codes> {
        (self.width == width && self.scale == scale && !self.points && !self.nils)
            .then_some(&self.codes)
    }

    /// A column the fast path computed: finite codes, no NIL.
    pub fn computed(width: Width, scale: i32, codes: Codes) -> Column {
        debug_assert_eq!(codes.width(), width);
        Column {
            width,
            scale,
            codes,
            valid: None,
            points: false,
            nils: false,
        }
    }

    /// The column as the interpreter's vector value.
    pub fn to_value(&self) -> Value {
        Value::Vector(
            (0..self.len())
                .map(|i| match self.lane(i) {
                    Some(v) => Value::Num(
                        Num::new(v, self.width, self.scale).expect("a column holds valid numbers"),
                    ),
                    None => Value::Nil(Reason::Literal),
                })
                .collect(),
        )
    }
}

/// An input of a compiled program: a scalar number or a column.
#[derive(Clone, PartialEq, Debug)]
pub enum Arg {
    Num(Num),
    Column(Column),
}

impl Arg {
    pub fn to_value(&self) -> Value {
        match self {
            Arg::Num(n) => Value::Num(*n),
            Arg::Column(c) => c.to_value(),
        }
    }

    /// The scalar, when it is a finite number of `width` at `scale`.
    pub fn plain_num(&self, width: Width, scale: i32) -> Option<i64> {
        match self {
            Arg::Num(n) if n.width() == width && n.scale() == scale => match n.val() {
                Val::Fin(k) => Some(k),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn column(&self) -> Option<&Column> {
        match self {
            Arg::Column(c) => Some(c),
            Arg::Num(_) => None,
        }
    }
}

/// A value a compiled program leaves: a column the fast path computed, or
/// any value of the interpreter's.
#[derive(Clone, PartialEq, Debug)]
pub enum Out {
    Column(Column),
    Bools(Vec<bool>),
    Value(Value),
}

impl Out {
    /// The value as the interpreter would leave it.
    pub fn to_value(&self) -> Value {
        match self {
            Out::Column(c) => c.to_value(),
            Out::Bools(bs) => Value::Vector(bs.iter().map(|&b| Value::Bool(b)).collect()),
            Out::Value(v) => v.clone(),
        }
    }
}

/// Run `source` in the reference interpreter on `args`: what compiled code
/// does whenever its fast path does not apply.
pub fn fallback(source: &str, args: &[Arg]) -> Result<Vec<Out>, RunError> {
    debug_assert!(parse(source).is_ok(), "compiled source parses");
    let stack = args.iter().map(Arg::to_value).collect();
    Ok(run_on(source, stack)?.into_iter().map(Out::Value).collect())
}

/// A finite number the fast path computed.
pub fn num(code: i64, width: Width, scale: i32) -> Out {
    Out::Value(Value::Num(
        Num::new(Val::Fin(code), width, scale).expect("the fast path keeps codes in their width"),
    ))
}

pub fn truth(b: bool) -> Out {
    Out::Value(Value::Bool(b))
}

/// SUM of `count` finite lanes totalling `total` (AR-HLR-016): `widths[k]` is
/// the width the interpreter's running sum has after `k + 1` lanes, the last
/// entry holding from there on; no lane is NIL on the fast path, so an empty
/// sum is the only NIL.
pub fn sum(count: usize, total: i128, widths: &[Width], scale: i32) -> Option<Out> {
    if count == 0 {
        return Some(Out::Value(Value::Nil(Reason::EmptyAggregate)));
    }
    let width = widths[(count - 1).min(widths.len() - 1)];
    let code = i64::try_from(total).ok()?;
    (code.unsigned_abs() <= width.max_finite() as u64).then(|| num(code, width, scale))
}

/// Whether `count` lanes each of magnitude at most `bound` sum without
/// leaving `limit`, whatever their order.
pub fn sum_fits(count: usize, bound: u128, limit: u128) -> bool {
    (count as u128)
        .checked_mul(bound)
        .is_some_and(|b| b <= limit)
}

/// DIV's rounded quotient `round(n / d)`, half to even, for integers
/// `|n| < 2^52` and `0 < |d| < 2^53`, computed in `f64` so that it
/// vectorizes (AR-LLR-026). Both operands are exact in `f64`, and the
/// correctly rounded quotient `q` never lands on the wrong side of a
/// half-integer: a quotient that is not a tie is at least `1/(2|d|)` from
/// one, while `|q − n/d| ≤ |n/d|·2^-53 < 1/(2|d|)` because `|n| < 2^52`; a
/// tie is exact in `f64` and divides exactly. So rounding `q` half to even
/// rounds `n/d` half to even. `arare_kernel::round_div` is the reference it is
/// tested against.
#[inline]
pub fn div_f64(n: f64, d: f64) -> f64 {
    (n / d).round_ties_even()
}

/// The largest dividend [`div_f64`] takes, exclusive.
pub const DIV_F64_DIVIDEND: u128 = 1 << 52;

/// The largest divisor [`div_f64`] takes, exclusive.
pub const DIV_F64_DIVISOR: u128 = 1 << 53;
