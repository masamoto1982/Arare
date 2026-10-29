//! What the checker knows of a stack value (AR-LLR-020, refining AR-HLR-027
//! and AR-HLR-029).
//!
//! Every claim relates two meanings of the same program: what Arare's machine
//! computes and the exact value Ajisai computes. A value both meanings agree
//! on the kind of — NIL, a number, a truth value, a vector of the same length,
//! the same quote — is described kind by kind; anything else is [`Abs::Top`].

use crate::info::{self, Info};
use crate::rat::Q;
use arare_front::Item;
use std::rc::Rc;

#[derive(Clone, PartialEq, Debug)]
pub enum Abs {
    /// Nothing is known, not even that the two meanings are the same kind.
    Top,
    /// NIL in both meanings.
    Nil,
    /// A number in both meanings, related as `info` says; with `nil`, the
    /// value may instead be NIL in both.
    Num { info: Info, nil: bool },
    /// A truth value or NIL in each meaning; with `agree`, the same one.
    Truth { agree: bool },
    /// A vector of this length in both meanings, lane by lane.
    Lanes(Vec<Abs>),
    /// A vector of the same length in both meanings, at most `max`, every
    /// lane of which `elem` describes.
    Hull { max: usize, elem: Box<Abs> },
    /// A quote not yet consumed: the same code in both meanings.
    Quote(Rc<Vec<Item>>),
}

impl Abs {
    pub fn num(info: Info) -> Abs {
        Abs::Num { info, nil: false }
    }

    pub fn is_vector(&self) -> bool {
        matches!(self, Abs::Lanes(_) | Abs::Hull { .. })
    }

    /// Whether a value other than [`Abs::Top`] is NIL in one meaning exactly
    /// when it is in the other.
    fn nil_agrees(&self) -> bool {
        !matches!(self, Abs::Truth { agree: false })
    }
}

/// A value that is one of two, the same one in both meanings: the branches
/// of a SELECT whose condition agrees, the lanes of a vector, the iterations
/// of a loop.
pub fn join(a: &Abs, b: &Abs) -> Abs {
    use Abs::*;
    match (a, b) {
        (Num { info: x, nil: n }, Num { info: y, nil: m }) => Num {
            info: info::join(*x, *y),
            nil: *n || *m,
        },
        (Nil, Nil) => Nil,
        (Nil, Num { info, .. }) | (Num { info, .. }, Nil) => Num {
            info: *info,
            nil: true,
        },
        (Truth { agree: x }, Truth { agree: y }) => Truth { agree: *x && *y },
        (Nil, Truth { agree }) | (Truth { agree }, Nil) => Truth { agree: *agree },
        (Lanes(xs), Lanes(ys)) if xs.len() == ys.len() => {
            Lanes(xs.iter().zip(ys).map(|(x, y)| join(x, y)).collect())
        }
        // Vectors of different or unknown lengths: one of them, the same in
        // both meanings, so of a length both share.
        (x, y) if x.is_vector() && y.is_vector() => Hull {
            max: max_len(x).max(max_len(y)),
            elem: Box::new(
                [lanes_joined(x), lanes_joined(y)]
                    .into_iter()
                    .flatten()
                    .reduce(|p, q| join(&p, &q))
                    .unwrap_or(Top),
            ),
        },
        // A quote is consumed as data before any Word joins values, so only
        // the kinds above meet here.
        _ => Top,
    }
}

/// The longest a vector can be; `0` for a value that is not a vector.
pub fn max_len(v: &Abs) -> usize {
    match v {
        Abs::Lanes(xs) => xs.len(),
        Abs::Hull { max, .. } => *max,
        _ => 0,
    }
}

/// What describes every lane of a vector, joined; `None` for an empty one
/// and for a value that is not a vector.
pub fn lanes_joined(v: &Abs) -> Option<Abs> {
    match v {
        Abs::Lanes(xs) => {
            let (first, rest) = xs.split_first()?;
            Some(rest.iter().fold(first.clone(), |acc, l| join(&acc, l)))
        }
        Abs::Hull { elem, .. } => Some((**elem).clone()),
        _ => None,
    }
}

/// A value the machine may take from one of two and the exact meaning from
/// the other: a SELECT whose condition may differ between them.
pub fn cross(a: &Abs, b: &Abs) -> Abs {
    use Abs::*;
    match (a, b) {
        (
            Num {
                info: x,
                nil: false,
            },
            Num {
                info: y,
                nil: false,
            },
        ) => Abs::num(info::cross(*x, *y)),
        (Nil, Nil) => Nil,
        (Truth { .. } | Nil, Truth { .. } | Nil) => Truth { agree: false },
        (Lanes(xs), Lanes(ys)) if xs.len() == ys.len() => {
            Lanes(xs.iter().zip(ys).map(|(x, y)| cross(x, y)).collect())
        }
        // Vectors whose lengths may differ between the two meanings, and
        // anything else.
        _ => Top,
    }
}

/// Whether EQ of `a` and `b` answers the same truth value in both meanings
/// (AR-HLR-029). EQ compares whole values by what they denote.
pub fn eq_agrees(a: &Abs, b: &Abs) -> bool {
    use Abs::*;
    match (a, b) {
        (Top, _) | (_, Top) => false,
        (Num { info: x, .. }, Num { info: y, .. }) => info::eq_agrees(*x, *y),
        (Truth { agree: x }, Truth { agree: y }) => *x && *y,
        (Lanes(xs), Lanes(ys)) => {
            xs.len() != ys.len() || xs.iter().zip(ys).all(|(x, y)| eq_agrees(x, y))
        }
        (Hull { elem: e1, .. }, Hull { elem: e2, .. }) => eq_agrees(e1, e2),
        (Hull { elem, .. }, Lanes(ys)) | (Lanes(ys), Hull { elem, .. }) => {
            ys.iter().all(|y| eq_agrees(elem, y))
        }
        // Different kinds are unequal in both meanings, as long as each is
        // NIL in both meanings or in neither.
        (x, y) => x.nil_agrees() && y.nil_agrees(),
    }
}

/// The error budget of a value: the largest error of any number in it, `0`
/// when the two meanings agree exactly, `None` when nothing is known.
pub fn budget(v: &Abs) -> Option<Q> {
    match v {
        Abs::Top | Abs::Quote(_) => None,
        Abs::Nil => Some(Q::ZERO),
        Abs::Num { info, .. } => info::budget(*info),
        Abs::Truth { agree } => agree.then_some(Q::ZERO),
        Abs::Lanes(xs) => xs.iter().try_fold(Q::ZERO, |acc, x| {
            let b = budget(x)?;
            Some(if acc.le(b) { b } else { acc })
        }),
        Abs::Hull { elem, .. } => budget(elem),
    }
}
