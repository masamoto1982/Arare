//! Exact rationals in 128 bits (AR-LLR-018, refining AR-HLR-027).
//!
//! The checker's bounds are the Lean model's rationals (`formal/Arare/Budget.lean`)
//! computed exactly. Every operation is checked: one whose exact result does
//! not fit answers `None`, and the checker then knows nothing of the value it
//! was bounding. A bound is never rounded, so the Rust checker answers what
//! the Lean `analyze` answers or nothing at all (ADR-0008).

use core::cmp::Ordering;
use core::fmt;

/// A rational `n/d` in lowest terms with `d > 0`. `n` is never `i128::MIN`,
/// so negation is total.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Q {
    n: i128,
    d: i128,
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Q {
    pub const ZERO: Q = Q { n: 0, d: 1 };
    pub const ONE: Q = Q { n: 1, d: 1 };

    /// `n/d` in lowest terms; `None` for `d = 0` or when `i128::MIN` would
    /// have to be negated.
    pub fn new(n: i128, d: i128) -> Option<Q> {
        if d == 0 || n == i128::MIN || d == i128::MIN {
            return None;
        }
        let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
        let g = gcd(n.unsigned_abs(), d as u128).max(1) as i128;
        Some(Q { n: n / g, d: d / g })
    }

    pub fn int(k: i128) -> Option<Q> {
        Q::new(k, 1)
    }

    pub fn num(self) -> i128 {
        self.n
    }

    pub fn den(self) -> i128 {
        self.d
    }

    pub fn is_zero(self) -> bool {
        self.n == 0
    }

    pub fn neg(self) -> Q {
        Q {
            n: -self.n,
            d: self.d,
        }
    }

    pub fn add(self, o: Q) -> Option<Q> {
        let g = gcd(self.d as u128, o.d as u128) as i128;
        let (da, db) = (self.d / g, o.d / g);
        let n = self.n.checked_mul(db)?.checked_add(o.n.checked_mul(da)?)?;
        Q::new(n, self.d.checked_mul(db)?)
    }

    pub fn sub(self, o: Q) -> Option<Q> {
        self.add(o.neg())
    }

    pub fn mul(self, o: Q) -> Option<Q> {
        let g1 = gcd(self.n.unsigned_abs(), o.d as u128).max(1) as i128;
        let g2 = gcd(o.n.unsigned_abs(), self.d as u128).max(1) as i128;
        let n = (self.n / g1).checked_mul(o.n / g2)?;
        let d = (self.d / g2).checked_mul(o.d / g1)?;
        Q::new(n, d)
    }

    /// The reciprocal; `0⁻¹ = 0`, as in Lean.
    pub fn inv(self) -> Q {
        match self.n.cmp(&0) {
            Ordering::Equal => Q::ZERO,
            Ordering::Greater => Q {
                n: self.d,
                d: self.n,
            },
            Ordering::Less => Q {
                n: -self.d,
                d: -self.n,
            },
        }
    }

    /// `self / 2`, `None` when it does not fit.
    pub fn half(self) -> Option<Q> {
        if self.n % 2 == 0 {
            Some(Q {
                n: self.n / 2,
                d: self.d,
            })
        } else {
            Some(Q {
                n: self.n,
                d: self.d.checked_mul(2)?,
            })
        }
    }

    /// `2^s`; `None` beyond `|s| ≤ 126`.
    pub fn pow2(s: i32) -> Option<Q> {
        if s.unsigned_abs() > 126 {
            return None;
        }
        let p = 1i128 << s.unsigned_abs();
        Some(if s >= 0 {
            Q { n: p, d: 1 }
        } else {
            Q { n: 1, d: p }
        })
    }

    /// The exact order, by comparing integer parts and then the reciprocals
    /// of the remainders, so it never overflows.
    pub fn compare(self, o: Q) -> Ordering {
        let (q1, r1) = (self.n.div_euclid(self.d), self.n.rem_euclid(self.d));
        let (q2, r2) = (o.n.div_euclid(o.d), o.n.rem_euclid(o.d));
        if q1 != q2 {
            return q1.cmp(&q2);
        }
        match (r1, r2) {
            (0, 0) => Ordering::Equal,
            (0, _) => Ordering::Less,
            (_, 0) => Ordering::Greater,
            // r1/d1 vs r2/d2, both in (0, 1): the order of d2/r2 vs d1/r1.
            _ => Q { n: o.d, d: r2 }.compare(Q { n: self.d, d: r1 }),
        }
    }

    pub fn le(self, o: Q) -> bool {
        self.compare(o) != Ordering::Greater
    }

    pub fn lt(self, o: Q) -> bool {
        self.compare(o) == Ordering::Less
    }
}

impl fmt::Display for Q {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.n, self.d)
    }
}
