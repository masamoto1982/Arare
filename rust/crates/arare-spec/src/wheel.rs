//! AR-LLR-001. Mirrors `formal/Arare/Wheel.lean` and `formal/Arare/Codes.lean`
//! (AR-HLR-002 to AR-HLR-007, AR-HLR-009).

use crate::rounding::round_div;
use core::fmt;

/// `W`: an integer code at a common scale, `1/0` or `0/0`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum W {
    Fin(i128),
    Inf,
    Bot,
}

impl W {
    pub fn add(self, other: W) -> W {
        match (self, other) {
            (W::Bot, _) | (_, W::Bot) => W::Bot,
            (W::Inf, W::Inf) => W::Bot,
            (W::Inf, W::Fin(_)) | (W::Fin(_), W::Inf) => W::Inf,
            (W::Fin(a), W::Fin(b)) => W::Fin(a + b),
        }
    }

    pub fn neg(self) -> W {
        match self {
            W::Fin(a) => W::Fin(-a),
            other => other,
        }
    }

    pub fn sub(self, other: W) -> W {
        self.add(other.neg())
    }

    pub fn mul(self, other: W) -> W {
        match (self, other) {
            (W::Bot, _) | (_, W::Bot) => W::Bot,
            (W::Inf, W::Inf) => W::Inf,
            (W::Inf, W::Fin(b)) => {
                if b == 0 {
                    W::Bot
                } else {
                    W::Inf
                }
            }
            (W::Fin(a), W::Inf) => {
                if a == 0 {
                    W::Bot
                } else {
                    W::Inf
                }
            }
            (W::Fin(a), W::Fin(b)) => W::Fin(a * b),
        }
    }

    pub fn div(self, other: W) -> W {
        match (self, other) {
            (W::Bot, _) | (_, W::Bot) => W::Bot,
            (W::Inf, W::Inf) => W::Bot,
            (W::Inf, W::Fin(_)) => W::Inf,
            (W::Fin(_), W::Inf) => W::Fin(0),
            (W::Fin(a), W::Fin(b)) => {
                if b == 0 {
                    if a == 0 {
                        W::Bot
                    } else {
                        W::Inf
                    }
                } else {
                    W::Fin(round_div(a, b))
                }
            }
        }
    }

    pub fn is_point(self) -> bool {
        !matches!(self, W::Fin(_))
    }
}

/// The table format `formal/Table.lean` prints: `F<k>`, `I` or `B`.
impl fmt::Display for W {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            W::Fin(k) => write!(f, "F{k}"),
            W::Inf => f.write_str("I"),
            W::Bot => f.write_str("B"),
        }
    }
}

/// `inRange`: the finite range of a width with half-range `m`.
pub fn in_range(m: i128, k: i128) -> bool {
    -(m - 2) <= k && k <= m - 2
}

/// `fit`: a finite number outside the range becomes `1/0`.
pub fn fit(m: i128, x: W) -> W {
    match x {
        W::Fin(k) if !in_range(m, k) => W::Inf,
        other => other,
    }
}

/// `encode`: the code of a value.
pub fn encode(m: i128, x: W) -> i128 {
    match x {
        W::Fin(k) => k,
        W::Inf => m - 1,
        W::Bot => -m,
    }
}

/// `decode`: the value of a code.
pub fn decode(m: i128, c: i128) -> W {
    if c == -m {
        W::Bot
    } else if c == m - 1 {
        W::Inf
    } else {
        W::Fin(c)
    }
}
