use super::fraction_arithmetic::balanced_bigint_gcd;
use num_bigint::BigInt;
use num_traits::{One, ToPrimitive, Zero};
use std::str::FromStr;

/// `gcd(a, b)` by Stein's binary algorithm: shifts and subtractions only.
///
/// Euclid's algorithm spends one hardware division per step, 20 to 90 cycles
/// each on a 64-bit operand, and the normalizer runs it on every rational
/// result. Stein's needs only `trailing_zeros`, a shift and a subtraction per
/// step. The gcd is unique, so the two agree everywhere — a property test
/// below holds them to it.
#[inline]
pub(crate) fn binary_gcd_u64(a: u64, b: u64) -> u64 {
    let (mut a, mut b) = if a >= b { (a, b) } else { (b, a) };
    // A unit — an integer's denominator, the commonest operand a reduction
    // meets — divides everything, and 0 is divided by everything.
    if b <= 1 {
        return if b == 0 { a } else { 1 };
    }
    // A power of two — a literal `2`, a binary denominator — shares with the
    // other operand exactly the twos both have.
    if a & (a - 1) == 0 || b & (b - 1) == 0 {
        return 1 << a.trailing_zeros().min(b.trailing_zeros());
    }
    // Stein's halves the larger operand at least every other step, so a
    // lopsided pair — a large value against a small denominator, the common
    // case — takes as many steps as the larger has bits. One division first
    // brings it within the smaller's width: gcd(a, b) = gcd(a mod b, b).
    if a >> 16 > b {
        a %= b;
        if a == 0 {
            return b;
        }
    }
    let shift = (a | b).trailing_zeros();
    a >>= a.trailing_zeros();
    loop {
        b >>= b.trailing_zeros();
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        b -= a;
        if b == 0 {
            return a << shift;
        }
    }
}

/// [`binary_gcd_u64`] for operands up to 128 bits: the products a rational
/// sum of two `i64` pairs is formed from. When both fit 64 bits, as they do
/// once a sum has been reduced, the 64-bit form runs.
#[inline]
pub(crate) fn binary_gcd_u128(mut a: u128, mut b: u128) -> u128 {
    if let (Ok(x), Ok(y)) = (u64::try_from(a), u64::try_from(b)) {
        return u128::from(binary_gcd_u64(x, y));
    }
    // One operand wide, the other a machine word: one division brings the
    // wide one within the narrow one's width, as `binary_gcd_u64` does.
    if a > b && b != 0 && u64::try_from(b).is_ok() {
        return u128::from(binary_gcd_u64((a % b) as u64, b as u64));
    }
    if b > a && a != 0 && u64::try_from(a).is_ok() {
        return u128::from(binary_gcd_u64((b % a) as u64, a as u64));
    }
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }
    let shift = (a | b).trailing_zeros();
    a >>= a.trailing_zeros();
    loop {
        b >>= b.trailing_zeros();
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        b -= a;
        if b == 0 {
            return a << shift;
        }
    }
}

#[inline]
pub(crate) fn compute_gcd_i64(a: i64, b: i64) -> i64 {
    // Reduce in unsigned space: `i64::MIN.abs()` overflows and panics, so the
    // signed `abs()` form crashed on `i64::MIN` operands (reachable from a
    // `-9223372036854775808` literal flowing through the fraction normalizer).
    let a = binary_gcd_u64(a.unsigned_abs(), b.unsigned_abs());
    // The gcd of two i64 values always fits in i64 except gcd(i64::MIN,
    // i64::MIN) == 2^63, which wraps to i64::MIN. Every caller only ever
    // divides its operands by this result, and i64::MIN / i64::MIN == 1, so the
    // reduction stays exact even in that single wrapping case.
    a as i64
}

#[derive(Debug, Clone)]
pub(crate) enum FractionRepr {
    Small(i64, i64),
    /// Boxed, so a `Fraction` is three words rather than the eight two
    /// inline `BigInt`s would make it: every `Value` holding a number pays
    /// for its width, and a fraction past a machine word is the rare one.
    Big(Box<BigPair>),
}

#[derive(Debug, Clone)]
pub(crate) struct BigPair {
    pub(crate) numerator: BigInt,
    pub(crate) denominator: BigInt,
}

impl FractionRepr {
    #[inline]
    pub(crate) fn big(numerator: BigInt, denominator: BigInt) -> Self {
        FractionRepr::Big(Box::new(BigPair {
            numerator,
            denominator,
        }))
    }
}

#[derive(Debug, Clone)]
pub struct Fraction {
    pub(crate) repr: FractionRepr,
}

impl Fraction {
    #[inline]
    pub(crate) fn from_repr(repr: FractionRepr) -> Self {
        Fraction { repr }
    }
}

/// Every `Fraction` is held canonically: in lowest terms, the denominator
/// positive or one of the two points over zero (`fraction_extended`), and
/// `Small` exactly when both halves fit a machine word. Every constructor
/// keeps it — `new` divides out the gcd, the arithmetic cancels before it
/// multiplies, `from_bigint_pair` narrows whatever fits — so two fractions
/// are equal exactly when their pairs are, and hash as those pairs.
impl PartialEq for Fraction {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (&self.repr, &other.repr) {
            (FractionRepr::Small(a, b), FractionRepr::Small(c, d)) => a == c && b == d,
            (FractionRepr::Big(x), FractionRepr::Big(y)) => {
                x.numerator == y.numerator && x.denominator == y.denominator
            }
            _ => false,
        }
    }
}

impl Eq for Fraction {}

impl std::hash::Hash for Fraction {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.repr {
            FractionRepr::Small(n, d) => {
                n.hash(state);
                d.hash(state);
            }
            FractionRepr::Big(big) => {
                big.numerator.hash(state);
                big.denominator.hash(state);
            }
        }
    }
}

impl Fraction {
    #[inline]
    pub fn is_small(&self) -> bool {
        matches!(self.repr, FractionRepr::Small(..))
    }

    /// A `Fraction` from a pair the caller already knows is in lowest terms
    /// with a non-negative denominator — one of the two pairs over zero
    /// included.
    ///
    /// `Fraction::new` reaches the same answer by widening both halves to
    /// `BigInt`, narrowing them straight back, and running a Euclidean gcd —
    /// two heap allocations and a `__divti3` loop. That is the right price for
    /// a pair of unknown provenance and pure waste for one that has already
    /// paid it. A `DenseTensor`'s columns are such a pair: every lane in one
    /// was written from an already-normalized `Fraction` (see
    /// `DenseTensor::from_columns`), so reading a lane back re-derived a
    /// normal form it was stored in.
    ///
    /// The precondition is checked in debug builds rather than trusted, so any
    /// path that ever stores an unnormalized column fails the test suite here
    /// instead of returning a quietly malformed value.
    #[inline]
    pub(crate) fn from_normalized_pair(numerator: i64, denominator: i64) -> Self {
        debug_assert!(
            denominator > 0 || (denominator == 0 && (0..=1).contains(&numerator)),
            "from_normalized_pair needs a reduced pair with a non-negative denominator, got {numerator}/{denominator}"
        );
        debug_assert!(
            {
                fn gcd(mut a: i128, mut b: i128) -> i128 {
                    a = a.abs();
                    b = b.abs();
                    while b != 0 {
                        let t = b;
                        b = a % b;
                        a = t;
                    }
                    a
                }
                gcd(numerator as i128, denominator as i128) == 1
                    || numerator == 0
                    || denominator == 0
            },
            "from_normalized_pair needs lowest terms, got {numerator}/{denominator}"
        );
        debug_assert!(
            numerator != 0 || denominator <= 1,
            "zero normalizes to 0/1, got {numerator}/{denominator}"
        );
        Fraction {
            repr: FractionRepr::Small(numerator, denominator),
        }
    }

    /// The reduced pair `numerator / denominator`. A zero denominator is a
    /// pair like any other: it reduces to one of the two points over zero
    /// (`fraction_extended`), so `100/0` and `-100/0` are `1/0`.
    pub fn new(numerator: BigInt, denominator: BigInt) -> Self {
        if denominator.is_zero() {
            return Self::over_zero(numerator.sign().cmp(&num_bigint::Sign::NoSign));
        }

        if numerator.is_zero() {
            return Fraction {
                repr: FractionRepr::Small(0, 1),
            };
        }

        if let (Some(n), Some(d)) = (numerator.to_i64(), denominator.to_i64()) {
            // Normalize through the i128 path: the previous i64 reduction
            // panicked on `i64::MIN` operands because sign normalization
            // (`num = -num`) overflows for `i64::MIN` (e.g. `Fraction::new(
            // i64::MIN, -1)`). Widening to i128 makes every abs/negation total
            // while producing an identical Small/Big result for i64 inputs.
            return Self::create_from_i128(n as i128, d as i128);
        }

        let common: BigInt = balanced_bigint_gcd(&numerator, &denominator);
        let mut num: BigInt = &numerator / &common;
        let mut den: BigInt = &denominator / &common;
        if den < BigInt::zero() {
            num = -num;
            den = -den;
        }
        Self::from_bigint_pair(num, den)
    }

    #[inline]
    pub(crate) fn create_already_reduced(mut numerator: BigInt, mut denominator: BigInt) -> Self {
        debug_assert!(!denominator.is_zero());
        if denominator < BigInt::zero() {
            numerator = -numerator;
            denominator = -denominator;
        }
        Self::from_bigint_pair(numerator, denominator)
    }

    #[inline]
    pub(crate) fn from_bigint_pair(numerator: BigInt, denominator: BigInt) -> Self {
        if let (Some(n), Some(d)) = (numerator.to_i64(), denominator.to_i64()) {
            return Fraction {
                repr: FractionRepr::Small(n, d),
            };
        }
        Fraction {
            repr: FractionRepr::big(numerator, denominator),
        }
    }

    #[inline]
    pub fn numerator(&self) -> BigInt {
        match &self.repr {
            FractionRepr::Small(n, _) => BigInt::from(*n),
            FractionRepr::Big(big) => big.numerator.clone(),
        }
    }

    #[inline]
    pub fn denominator(&self) -> BigInt {
        match &self.repr {
            FractionRepr::Small(_, d) => BigInt::from(*d),
            FractionRepr::Big(big) => big.denominator.clone(),
        }
    }

    #[inline]
    pub fn to_bigint_pair(&self) -> (BigInt, BigInt) {
        match &self.repr {
            FractionRepr::Small(n, d) => (BigInt::from(*n), BigInt::from(*d)),
            FractionRepr::Big(big) => (big.numerator.clone(), big.denominator.clone()),
        }
    }

    #[inline]
    pub fn is_integer(&self) -> bool {
        match &self.repr {
            FractionRepr::Small(_, d) => *d == 1,
            FractionRepr::Big(big) => big.denominator.is_one(),
        }
    }

    /// True for the number zero. `0/0` is not zero: its denominator is 0, and
    /// it is one of the two points of `fraction_extended`.
    #[inline]
    pub fn is_zero(&self) -> bool {
        matches!(self.repr, FractionRepr::Small(0, 1))
    }

    /// True for values strictly greater than zero. The denominator is always
    /// normalised positive, so the sign is carried entirely by the numerator.
    #[inline]
    pub fn is_positive(&self) -> bool {
        match &self.repr {
            FractionRepr::Small(n, _) => *n > 0,
            FractionRepr::Big(big) => big.numerator > BigInt::zero(),
        }
    }

    #[inline]
    pub(crate) fn extract_i64_pair(&self) -> Option<(i64, i64)> {
        match self.repr {
            FractionRepr::Small(n, d) => Some((n, d)),
            FractionRepr::Big(_) => None,
        }
    }

    #[inline]
    pub fn to_i64(&self) -> Option<i64> {
        match self.repr {
            FractionRepr::Small(n, 1) => Some(n),
            _ => None,
        }
    }

    #[inline]
    pub fn as_usize(&self) -> Option<usize> {
        usize::try_from(self.as_u64()?).ok()
    }

    /// The value as a `u64` when it is a non-negative integer that fits one.
    ///
    /// Unlike [`as_usize`](Self::as_usize), the answer is the same on every
    /// target. A count or an axis length that a 32-bit `usize` cannot hold is
    /// still a well-formed count, so a Word reads it here and decides what it
    /// means (with [`saturating_narrow`]) rather than declining it as
    /// malformed on wasm32 alone.
    pub fn as_u64(&self) -> Option<u64> {
        match &self.repr {
            FractionRepr::Small(n, d) => {
                if *d != 1 {
                    return None;
                }
                u64::try_from(*n).ok()
            }
            FractionRepr::Big(big) => {
                if !big.denominator.is_one() {
                    return None;
                }
                big.numerator.to_u64()
            }
        }
    }

    #[inline]
    pub(crate) fn create_from_i128(num: i128, den: i128) -> Self {
        if den == 0 {
            return Self::over_zero(num.cmp(&0));
        }
        let g = binary_gcd_u128(num.unsigned_abs(), den.unsigned_abs()) as i128;
        let (n, d) = (num / g, den / g);
        let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
        match (i64::try_from(n), i64::try_from(d)) {
            (Ok(n), Ok(d)) => Fraction {
                repr: FractionRepr::Small(n, d),
            },
            _ => Fraction {
                repr: FractionRepr::big(BigInt::from(n), BigInt::from(d)),
            },
        }
    }

    // Inherent `from_str` (not `std::str::FromStr`): the `String` error type
    // and the parse call sites predate the trait, and switching would churn
    // every caller for no behavioral gain.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> std::result::Result<Self, String> {
        if s.is_empty() {
            return Err("Empty string".to_string());
        }

        if let Some(e_pos) = s.find(|c| char::is_ascii(&c) && (c == 'e' || c == 'E')) {
            let mantissa_str = &s[..e_pos];
            let exponent_str = &s[e_pos + 1..];

            let mantissa: Fraction = Self::from_str(mantissa_str)?;
            // The exponent's *form* is checked before its *range*: a zero
            // mantissa is zero at any scale, so `0e2147483648` denotes 0 and
            // must not fail the i32 parse below — which is what it did while
            // the range check came first, making `0e<beyond i32>` a mid-run
            // source error here, `invalidEncoding` through NUM and 0 through
            // JSON-DECODE, three answers for one lexeme.
            let exponent_digits = exponent_str
                .strip_prefix(['+', '-'])
                .unwrap_or(exponent_str);
            if exponent_digits.is_empty() || !exponent_digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(format!("invalid exponent: {exponent_str}"));
            }
            let (mn, md): (BigInt, BigInt) = mantissa.to_bigint_pair();
            // Skip the (possibly enormous) power, so `0e<huge>` cannot drive
            // an unbounded `pow` allocation.
            if mn.is_zero() {
                return Ok(Fraction::new(BigInt::zero(), BigInt::one()));
            }
            let exponent: i32 = exponent_str.parse::<i32>().map_err(|e| e.to_string())?;
            // Take the magnitude with `unsigned_abs`: negating an i32::MIN
            // exponent (`1e-2147483648`) overflows and panicked here, a crash
            // reachable from both a numeric literal and `NUM` on a string.
            let magnitude: u32 = exponent.unsigned_abs();
            let power: BigInt = BigInt::from(10).pow(magnitude);
            if exponent >= 0 {
                return Ok(Fraction::new(mn * power, md));
            } else {
                return Ok(Fraction::new(mn, md * power));
            }
        }
        if let Some(pos) = s.find('/') {
            let num: BigInt = BigInt::from_str(&s[..pos]).map_err(|e| e.to_string())?;
            let den: BigInt = BigInt::from_str(&s[pos + 1..]).map_err(|e| e.to_string())?;
            // `n/0` denotes a number like any other pair (LANG.SOURCE.TEXT):
            // `Fraction::new` reduces it to the sign of `n` over zero.
            Ok(Fraction::new(num, den))
        } else if let Some(dot_pos) = s.find('.') {
            // Strip an optional leading sign up front so that signed
            // leading-dot decimals (`-.5`, `+.5`) parse exactly like `.5`.
            // (Numeric literals are ASCII, so byte and char indices coincide.)
            let (neg, body, dot_pos) = if let Some(rest) = s.strip_prefix('-') {
                (true, rest, dot_pos - 1)
            } else if let Some(rest) = s.strip_prefix('+') {
                (false, rest, dot_pos - 1)
            } else {
                (false, s, dot_pos)
            };
            let int_part_str = if body.starts_with('.') {
                "0"
            } else {
                &body[..dot_pos]
            };
            let frac_part_str = &body[dot_pos + 1..];
            let int_part: BigInt = BigInt::from_str(int_part_str).map_err(|e| e.to_string())?;
            if frac_part_str.is_empty() {
                // Trailing-dot decimal such as `5.` denotes the integer part.
                return Ok(Fraction::new(
                    if neg { -int_part } else { int_part },
                    BigInt::one(),
                ));
            }
            let frac_num: BigInt = BigInt::from_str(frac_part_str).map_err(|e| e.to_string())?;
            let frac_den: BigInt = BigInt::from(10).pow(frac_part_str.len() as u32);
            let total_num: BigInt = int_part * &frac_den + frac_num;
            Ok(Fraction::new(
                if neg { -total_num } else { total_num },
                frac_den,
            ))
        } else {
            let num: BigInt = BigInt::from_str(s).map_err(|e| e.to_string())?;

            if let Some(n) = num.to_i64() {
                return Ok(Fraction {
                    repr: FractionRepr::Small(n, 1),
                });
            }
            Ok(Fraction {
                repr: FractionRepr::big(num, BigInt::one()),
            })
        }
    }
}

impl From<i64> for Fraction {
    #[inline]
    fn from(n: i64) -> Self {
        Fraction {
            repr: FractionRepr::Small(n, 1),
        }
    }
}

impl From<i32> for Fraction {
    #[inline]
    fn from(n: i32) -> Self {
        Fraction {
            repr: FractionRepr::Small(n as i64, 1),
        }
    }
}

impl std::fmt::Display for Fraction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.repr {
            FractionRepr::Small(n, d) => {
                if *d == 1 {
                    write!(f, "{}", n)
                } else {
                    write!(f, "{}/{}", n, d)
                }
            }
            FractionRepr::Big(big) => {
                let (numerator, denominator) = (&big.numerator, &big.denominator);
                if denominator.is_one() {
                    write!(f, "{}", numerator)
                } else {
                    write!(f, "{}/{}", numerator, denominator)
                }
            }
        }
    }
}

/// `n` narrowed to `T`, saturating at `T::MAX`, for a count read with
/// [`Fraction::as_u64`]. A count past `T::MAX` exceeds anything the host can
/// hold, so the saturated count fails the same size or stack check the exact
/// one would. Generic in the width so the 32-bit narrowing wasm32 performs
/// can be exercised on a 64-bit host.
pub(crate) fn saturating_narrow<T: TryFrom<u64> + num_traits::Bounded>(n: u64) -> T {
    T::try_from(n).unwrap_or_else(|_| T::max_value())
}

#[cfg(test)]
mod literal_parsing_tests {
    use super::Fraction;

    #[test]
    fn literals_parse_to_reduced_canonical_fractions() {
        // Surface forms are convenience syntax; all reduce to the same
        // canonical exact-real value. No surface style is retained.
        assert_eq!(
            Fraction::from_str("0.5").unwrap(),
            Fraction::from_str("1/2").unwrap()
        );
        assert_eq!(
            Fraction::from_str("2/1").unwrap(),
            Fraction::from_str("2").unwrap()
        );
        assert_eq!(
            Fraction::from_str("4/2").unwrap(),
            Fraction::from_str("2").unwrap()
        );
        assert!(Fraction::from_str("2/1").unwrap().is_integer());
    }

    #[test]
    fn arithmetic_results_are_canonical() {
        let left = Fraction::from_str("0.5").expect("literal parses");
        let right = Fraction::from_str("0.5").expect("literal parses");
        let sum = left.add(&right);
        assert_eq!(sum.to_string(), "1");
    }

    #[test]
    fn zero_denominator_fraction_literal_reduces_to_its_point_over_zero() {
        // `n/0` denotes one of the two points (LANG.SOURCE.TEXT), reduced
        // like every other pair.
        assert_eq!(Fraction::from_str("3/0").unwrap(), Fraction::infinity());
        assert_eq!(Fraction::from_str("0/0").unwrap(), Fraction::nullity());
        assert_eq!(Fraction::from_str("-5/0").unwrap(), Fraction::infinity());
    }

    #[test]
    fn signed_leading_dot_decimals_parse() {
        // The tokenizer accepts `-.5` / `+.5`; the converter must agree.
        assert_eq!(
            Fraction::from_str("-.5").unwrap(),
            Fraction::from_str("-1/2").unwrap()
        );
        assert_eq!(
            Fraction::from_str("+.5").unwrap(),
            Fraction::from_str("1/2").unwrap()
        );
    }

    #[test]
    fn documented_surface_variants_are_accepted() {
        // Forms now documented in LANG.SOURCE.TEXT beyond the canonical examples:
        // leading `+`, uppercase `E`, explicit `e+`, leading zeros, and
        // trailing-dot decimals. All reduce to their canonical value.
        let cases = [
            ("+7", "7"),
            ("+3/4", "3/4"),
            ("007", "7"),
            ("5.", "5"),
            ("1.E5", "100000"),
            ("1.5E2", "150"),
            ("1e+5", "100000"),
        ];
        for (src, expected) in cases {
            assert_eq!(
                Fraction::from_str(src).unwrap(),
                Fraction::from_str(expected).unwrap(),
                "`{src}` should equal `{expected}`"
            );
        }
    }
}
