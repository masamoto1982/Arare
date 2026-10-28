//! The square-free part of a radicand, so that one number has one normal form.
//!
//! `√n` is written `s·√m` with `n = s²·m` and `m` square-free. Monomials are
//! then subset products of square-free, pairwise-coprime basis elements, which
//! are themselves square-free integers — so a monomial is decided by the value
//! alone, whatever basis the value happens to carry (`√6` is keyed `6` over the
//! basis `{6}` and over `{2, 3}` alike). Without this, `8 SQRT` kept the
//! monomial `8` while `2 SQRT 2 SQRT ADD` kept `2`: one value, two displays and
//! two protocol normal forms, which LANG.VALUES.DENOTATION rules out.
//!
//! Extracting square factors needs the prime factorization, which has no known
//! cheap algorithm for a large integer. The work is therefore metered: every
//! step is charged against a budget the caller supplies (the run's remaining
//! `numericWork`), and a radicand the budget cannot factor is refused as a
//! resource limit rather than left in a non-canonical form. Trial division
//! takes the small primes, a perfect-square test and a primality test settle
//! the common large cofactors, and Pollard's rho (Brent's variant) splits the
//! rest.
//!
//! Primality is decided by Miller–Rabin over the first thirteen prime bases,
//! which is a proof below 3.3·10²⁴. Above that it is a strong probable-prime
//! test with no known counterexample; the only consequence of misjudging a
//! composite would be a non-square-free radicand — a display that differs,
//! never a value that is wrong, since equality and order do not depend on the
//! basis being square-free (`basis.rs`).

use crate::types::fraction_arithmetic::balanced_bigint_gcd;
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

/// The work budget ran out before the radicand was factored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactorBudgetExhausted;

/// Primes below which the cofactor is cleared by trial division.
const TRIAL_BOUND: u64 = 1 << 12;
const MR_BASES: [u64; 13] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41];

const fn is_prime(d: u64) -> bool {
    let mut k = 2u64;
    while k * k <= d {
        if d.is_multiple_of(k) {
            return false;
        }
        k += 1;
    }
    d >= 2
}

const fn count_primes() -> usize {
    let mut count = 0;
    let mut d = 2;
    while d < TRIAL_BOUND {
        if is_prime(d) {
            count += 1;
        }
        d += 1;
    }
    count
}

const PRIME_COUNT: usize = count_primes();

/// A prime below `TRIAL_BOUND`, with what tests a word for it without a
/// division: for odd `p`, `p | r` exactly when `r·p⁻¹ mod 2⁶⁴ ≤ ⌊(2⁶⁴−1)/p⌋`,
/// because multiplying by `p⁻¹` maps the multiples of `p` onto `0..=limit`
/// and everything else above it (Granlund and Montgomery, "Division by
/// invariant integers using multiplication", 1994, §9).
#[derive(Clone, Copy)]
struct TrialPrime {
    p: u64,
    /// `p⁻¹ mod 2⁶⁴` (unused for 2).
    inverse: u64,
    /// `⌊(2⁶⁴−1)/p⌋`.
    limit: u64,
    /// The group whose product `p` is in.
    group: u16,
}

impl TrialPrime {
    #[inline]
    fn divides(&self, r: u64) -> bool {
        if self.p == 2 {
            r & 1 == 0
        } else {
            r.wrapping_mul(self.inverse) <= self.limit
        }
    }
}

/// The primes below `TRIAL_BOUND`, packed in ascending order into groups whose
/// product fits one word — the binary counterpart of testing a decimal number
/// for 3 and 9 at once by its digit sum: one wide remainder by a group's
/// product leaves a word that every prime of the group then tests.
struct TrialTable {
    primes: [TrialPrime; PRIME_COUNT],
    /// The product of each group's primes; unused entries past the last are 0.
    products: [u64; PRIME_COUNT],
}

static TRIAL_TABLE: TrialTable = trial_table();

const fn trial_table() -> TrialTable {
    let blank = TrialPrime {
        p: 0,
        inverse: 0,
        limit: 0,
        group: 0,
    };
    let mut table = TrialTable {
        primes: [blank; PRIME_COUNT],
        products: [0; PRIME_COUNT],
    };
    let mut index = 0;
    let mut group = 0usize;
    let mut product: u64 = 1;
    let mut d = 2u64;
    while d < TRIAL_BOUND {
        if is_prime(d) {
            if product.checked_mul(d).is_none() {
                table.products[group] = product;
                group += 1;
                product = 1;
            }
            product *= d;
            // Newton's iteration doubles the correct low bits each step.
            let mut inverse = d;
            let mut step = 0;
            while step < 5 {
                inverse = inverse.wrapping_mul(2u64.wrapping_sub(d.wrapping_mul(inverse)));
                step += 1;
            }
            table.primes[index] = TrialPrime {
                p: d,
                inverse,
                limit: u64::MAX / d,
                group: group as u16,
            };
            index += 1;
        }
        d += 1;
    }
    table.products[group] = product;
    table
}

/// How many trial candidates — 2, then every odd number — are at most `x`.
fn candidates_through(x: u64) -> u64 {
    if x < 2 {
        0
    } else {
        1 + (x - 1) / 2
    }
}

/// `|n| mod m`: on `u128` when `n` fits, else by reciprocal.
fn residue_mod(n: &BigInt, m: u64) -> u64 {
    match n.magnitude().to_u128() {
        Some(v) => (v % u128::from(m)) as u64,
        None => crate::types::small_divisor::rem_word(n, m),
    }
}

/// What one factoring step costs, in the `numericWork` limb-multiply unit, on
/// top of its limb count squared. A step is a bignum multiply, a remainder, a
/// subtraction and the allocations between them — about as much as the
/// rational addition the unit is calibrated on, several times over — so it is
/// priced by measurement (`squarefree` timing in `docs/dev`), to within the
/// order of magnitude that keeps the ceiling meaningful.
const STEP_UNITS: u64 = 8;

/// A work meter over `budget`: each modular step costs `STEP_UNITS` times its
/// limb count squared.
struct Meter<'a> {
    budget: &'a mut u64,
}

impl Meter<'_> {
    fn charge(&mut self, n: &BigInt) -> Result<(), FactorBudgetExhausted> {
        self.charge_units(Self::units(n))
    }

    /// What one step on `n` costs.
    fn units(n: &BigInt) -> u64 {
        let limbs = n.bits().div_ceil(64).max(1);
        limbs.saturating_mul(limbs).saturating_mul(STEP_UNITS)
    }

    fn charge_units(&mut self, units: u64) -> Result<(), FactorBudgetExhausted> {
        if *self.budget < units {
            *self.budget = 0;
            return Err(FactorBudgetExhausted);
        }
        *self.budget -= units;
        Ok(())
    }
}

/// `n = s²·m` with `m` square-free, for `n ≥ 1`: returns `(s, m)`.
pub fn squarefree_split(
    n: &BigInt,
    budget: &mut u64,
) -> Result<(BigInt, BigInt), FactorBudgetExhausted> {
    let mut meter = Meter { budget };
    let mut primes: Vec<(BigInt, u32)> = Vec::new();
    let mut rest = n.clone();

    // Each candidate below `TRIAL_BOUND` — 2, then every odd number, until
    // its square passes `rest` — is charged one step on `rest`, as when each
    // was divided in turn. Only the primes are tested; the composites between
    // them are charged together, which runs out of budget exactly when
    // charging them one by one would.
    let mut charged: u64 = 0;
    // The residue of `rest` modulo the current group's product, once per
    // group rather than one wide remainder per candidate; dropped whenever a
    // factor comes out of `rest`.
    let mut residue: Option<(u16, u64)> = None;
    // What a step costs, `rest` as a word and the last candidate its square
    // root admits, all fixed until a factor comes out of `rest`.
    let mut units = Meter::units(&rest);
    let mut rest_word = rest.to_u64();
    let mut last = rest_word.map_or(u64::MAX, u64::isqrt);
    for prime in TRIAL_TABLE
        .primes
        .iter()
        .map(Some)
        .chain(std::iter::once(None))
    {
        let through = prime.map_or(TRIAL_BOUND - 1, |prime| prime.p);
        let reach = through.min(last);
        let due = candidates_through(reach).saturating_sub(charged);
        meter.charge_units(units.saturating_mul(due))?;
        charged += due;
        let Some(prime) = prime.filter(|prime| prime.p == reach) else {
            break;
        };
        let r = match (rest_word, residue) {
            (Some(word), _) => word,
            (None, Some((group, r))) if group == prime.group => r,
            (None, _) => {
                let r = residue_mod(&rest, TRIAL_TABLE.products[usize::from(prime.group)]);
                residue = Some((prime.group, r));
                r
            }
        };
        if prime.divides(r) {
            let big_d = BigInt::from(prime.p);
            let mut exponent = 0;
            while (&rest % &big_d).is_zero() {
                rest /= &big_d;
                exponent += 1;
            }
            primes.push((big_d, exponent));
            residue = None;
            units = Meter::units(&rest);
            rest_word = rest.to_u64();
            last = rest_word.map_or(u64::MAX, u64::isqrt);
        }
    }
    factor_into(&rest, 1, &mut primes, &mut meter)?;

    let mut s = BigInt::one();
    let mut m = BigInt::one();
    primes.sort();
    let mut i = 0;
    while i < primes.len() {
        let p = primes[i].0.clone();
        let mut exponent = 0;
        while i < primes.len() && primes[i].0 == p {
            exponent += primes[i].1;
            i += 1;
        }
        s *= num_traits::pow(p.clone(), (exponent / 2) as usize);
        if exponent % 2 == 1 {
            m *= p;
        }
    }
    Ok((s, m))
}

/// Push the prime factors of `n` (every one ≥ `TRIAL_BOUND`, or `n` is 1),
/// each with `multiplicity`.
fn factor_into(
    n: &BigInt,
    multiplicity: u32,
    primes: &mut Vec<(BigInt, u32)>,
    meter: &mut Meter,
) -> Result<(), FactorBudgetExhausted> {
    if n.is_one() {
        return Ok(());
    }
    let bound = BigInt::from(TRIAL_BOUND);
    if n < &(&bound * &bound) {
        // No factor below the trial bound and below its square: prime.
        primes.push((n.clone(), multiplicity));
        return Ok(());
    }
    meter.charge(n)?;
    if let Some(root) = exact_sqrt(n) {
        return factor_into(&root, multiplicity * 2, primes, meter);
    }
    if is_probable_prime(n, meter)? {
        primes.push((n.clone(), multiplicity));
        return Ok(());
    }
    let d = pollard_brent(n, meter)?;
    factor_into(&d, multiplicity, primes, meter)?;
    factor_into(&(n / &d), multiplicity, primes, meter)
}

fn is_probable_prime(n: &BigInt, meter: &mut Meter) -> Result<bool, FactorBudgetExhausted> {
    is_probable_prime_word(n, meter).unwrap_or_else(|| is_probable_prime_wide(n, meter))
}

fn is_probable_prime_wide(n: &BigInt, meter: &mut Meter) -> Result<bool, FactorBudgetExhausted> {
    let one = BigInt::one();
    let n_minus_1 = n - &one;
    let mut d = n_minus_1.clone();
    let mut r = 0u32;
    while d.is_even() {
        d >>= 1;
        r += 1;
    }
    'bases: for &a in &MR_BASES {
        let a = BigInt::from(a);
        if (&a % n).is_zero() {
            continue;
        }
        for _ in 0..n.bits() {
            meter.charge(n)?;
        }
        let mut x = a.modpow(&d, n);
        if x.is_one() || x == n_minus_1 {
            continue;
        }
        for _ in 1..r {
            meter.charge(n)?;
            x = (&x * &x) % n;
            if x == n_minus_1 {
                continue 'bases;
            }
        }
        return Ok(false);
    }
    Ok(true)
}

/// A non-trivial factor of the composite `n`, by Pollard's rho with Brent's
/// cycle detection, trying successive polynomial constants.
fn pollard_brent(n: &BigInt, meter: &mut Meter) -> Result<BigInt, FactorBudgetExhausted> {
    pollard_brent_word(n, meter).unwrap_or_else(|| pollard_brent_wide(n, meter))
}

fn pollard_brent_wide(n: &BigInt, meter: &mut Meter) -> Result<BigInt, FactorBudgetExhausted> {
    let one = BigInt::one();
    for c in 1u64.. {
        let c = BigInt::from(c);
        let f = |x: &BigInt| (x * x + &c) % n;
        let mut y = BigInt::from(2);
        let mut r: u64 = 1;
        let mut q = BigInt::one();
        let mut g = BigInt::one();
        let mut x = y.clone();
        let mut ys = y.clone();
        const M: u64 = 128;
        while g.is_one() {
            x = y.clone();
            for _ in 0..r {
                meter.charge(n)?;
                y = f(&y);
            }
            let mut k = 0;
            while k < r && g.is_one() {
                ys = y.clone();
                for _ in 0..M.min(r - k) {
                    meter.charge(n)?;
                    y = f(&y);
                    q = (q * (&x - &y).abs()) % n;
                }
                g = balanced_bigint_gcd(&q, n);
                k += M;
            }
            r *= 2;
        }
        if g == *n {
            loop {
                meter.charge(n)?;
                ys = f(&ys);
                g = balanced_bigint_gcd(&(&x - &ys).abs(), n);
                if !g.is_one() {
                    break;
                }
            }
        }
        if g != *n && g != one {
            return Ok(g);
        }
        // This constant cycled without splitting; try the next one.
        if c.to_u64().unwrap_or(u64::MAX) > 64 {
            return Err(FactorBudgetExhausted);
        }
    }
    unreachable!("the constant loop only exits by returning")
}

/// Which residues modulo `m` are squares.
const fn squares_mod<const M: usize>() -> [bool; M] {
    let mut table = [false; M];
    let mut i = 0;
    while i < M {
        table[(i * i) % M] = true;
        i += 1;
    }
    table
}

static SQUARES_MOD_64: [bool; 64] = squares_mod::<64>();
static SQUARES_MOD_63: [bool; 63] = squares_mod::<63>();
static SQUARES_MOD_65: [bool; 65] = squares_mod::<65>();
static SQUARES_MOD_11: [bool; 11] = squares_mod::<11>();

/// `√n` when the non-negative `n` is a perfect square.
///
/// Like ruling out a multiple of 3 by its digit sum before dividing, a number
/// whose last six bits, or whose residue modulo 63, 65 or 11, no square can
/// have is turned away by table lookups — all but about one non-square in 120
/// — before the square root is taken (the screen GMP's `mpz_perfect_square_p`
/// uses).
pub(crate) fn exact_sqrt(n: &BigInt) -> Option<BigInt> {
    let low = n.magnitude().iter_u64_digits().next().unwrap_or(0);
    if !SQUARES_MOD_64[(low & 63) as usize] {
        return None;
    }
    let r = residue_mod(n, 63 * 65 * 11);
    if !SQUARES_MOD_63[(r % 63) as usize]
        || !SQUARES_MOD_65[(r % 65) as usize]
        || !SQUARES_MOD_11[(r % 11) as usize]
    {
        return None;
    }
    let root = n.sqrt();
    (&root * &root == *n).then_some(root)
}

#[path = "squarefree_word.rs"]
mod word;
use word::{is_probable_prime_word, pollard_brent_word};

#[cfg(test)]
#[path = "squarefree_tests.rs"]
mod tests;
