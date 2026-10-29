//! The Words the checker follows by their operands' kinds rather than lane by
//! lane through [`crate::info`]: LT, AND, NOT, SELECT, SUM, MAP, FILTER and
//! FOLD (AR-LLR-020, refining AR-HLR-029). Each follows the reference
//! interpreter's rule for the Word (`arare-interp`'s `words.rs` and
//! `lift.rs`) on what is known of its operands.

use crate::check::{Checker, Why};
use crate::info::{self, Info};
use crate::value::{cross, join, lanes_joined, Abs};
use arare_front::Word;

/// The lane of `v` that pairs with lane `i` of a condition with `n` lanes.
fn lane(v: &Abs, i: usize, n: usize) -> Abs {
    match v {
        Abs::Lanes(xs) if xs.len() == n => xs[i].clone(),
        Abs::Lanes(xs) if xs.len() == 1 => xs[0].clone(),
        Abs::Lanes(_) => Abs::Top,
        Abs::Hull { elem, .. } => (**elem).clone(),
        other => other.clone(),
    }
}

/// Every lane of `v`, joined; a scalar is its own lane.
pub(crate) fn any_lane(v: &Abs) -> Abs {
    if v.is_vector() {
        lanes_joined(v).unwrap_or(Abs::Top)
    } else {
        v.clone()
    }
}

impl Checker {
    /// LT: the order of two numbers; of a wheel point, UNKNOWN in both.
    pub(crate) fn less(&mut self, a: &Abs, b: &Abs) -> Abs {
        self.lift2(Word::Lt, a, b, &mut |_, x, y| match (x, y) {
            (Abs::Num { info: p, .. }, Abs::Num { info: q, .. }) => {
                let point = |i: &Info| matches!(i, Info::Inf | Info::Bot);
                if point(p) || point(q) {
                    Abs::Nil
                } else {
                    Abs::Truth {
                        agree: info::lt_agrees(*p, *q).unwrap_or(false),
                    }
                }
            }
            _ => Abs::Top,
        })
    }

    /// AND: a NIL is UNKNOWN here, and vectors pair lane by lane.
    pub(crate) fn and(&mut self, a: &Abs, b: &Abs) -> Abs {
        use Abs::*;
        if a.is_vector() || b.is_vector() {
            let one = |v: &Abs| {
                if v.is_vector() {
                    v.clone()
                } else {
                    Lanes(vec![v.clone()])
                }
            };
            return self.lift2(Word::And, &one(a), &one(b), &mut |c, x, y| c.and(x, y));
        }
        let truth = |v: &Abs| match v {
            Nil => Some(true),
            Truth { agree } => Some(*agree),
            _ => None,
        };
        match (a, b, truth(a), truth(b)) {
            (Nil, Nil, _, _) => Nil,
            (_, _, Some(x), Some(y)) => Truth { agree: x && y },
            _ => Top,
        }
    }

    pub(crate) fn not(&mut self, a: &Abs) -> Abs {
        match a {
            Abs::Nil => Abs::Nil,
            Abs::Truth { agree } => Abs::Truth { agree: *agree },
            Abs::Lanes(xs) => Abs::Lanes(xs.iter().map(|x| self.not(x)).collect()),
            Abs::Hull { max, elem } => Abs::Hull {
                max: *max,
                elem: Box::new(self.not(elem)),
            },
            _ => Abs::Top,
        }
    }

    /// SELECT: THEN only on TRUE, in both meanings alike when the condition
    /// agrees; otherwise each meaning may take either branch (AR-HLR-029).
    pub(crate) fn select(&mut self, then: &Abs, otherwise: &Abs, cond: &Abs) -> Abs {
        match cond {
            Abs::Lanes(cs) => Abs::Lanes(
                (0..cs.len())
                    .map(|i| {
                        let (t, e) = (lane(then, i, cs.len()), lane(otherwise, i, cs.len()));
                        self.select(&t, &e, &cs[i])
                    })
                    .collect(),
            ),
            Abs::Hull { max, elem } => Abs::Hull {
                max: *max,
                elem: Box::new(self.select(&any_lane(then), &any_lane(otherwise), elem)),
            },
            Abs::Nil => otherwise.clone(),
            Abs::Truth { agree: true } => join(then, otherwise),
            Abs::Truth { agree: false } => cross(then, otherwise),
            _ => Abs::Top,
        }
    }

    /// SUM: skips NIL; NIL when nothing is left (AR-HLR-016).
    pub(crate) fn sum(&mut self, v: &Abs) -> Abs {
        // What is known of the sum so far: whether nothing may have been
        // added yet, and of the total when something has.
        let (mut empty, mut total): (bool, Option<Info>) = (true, None);
        let mut step = |c: &mut Checker, lane: &Abs, may_skip: bool| -> bool {
            let (i, nil) = match lane {
                Abs::Nil => return true,
                Abs::Num { info, nil } => (*info, *nil || may_skip),
                _ => return false,
            };
            let added = total.map(|t| c.result(Word::Sum, &[t, i], info::add(t, i)));
            let mut next = match (empty, added) {
                (true, Some(a)) => Some(info::join(i, a)),
                (true, None) => Some(i),
                (false, a) => a,
            };
            if nil {
                next = match (next, total) {
                    (Some(n), Some(t)) => Some(info::join(n, t)),
                    (n, _) => n,
                };
            } else {
                empty = false;
            }
            total = next;
            true
        };
        let ok = match v {
            Abs::Lanes(xs) => xs.iter().all(|x| step(self, x, false)),
            Abs::Hull { max, elem } => (0..*max).all(|_| step(self, elem, true)),
            _ => false,
        };
        match (ok, total) {
            (false, _) => {
                self.note(Some(Word::Sum), Why::Unbounded);
                Abs::Top
            }
            (true, None) => Abs::Nil,
            (true, Some(t)) => Abs::Num {
                info: t,
                nil: empty,
            },
        }
    }

    /// MAP, FILTER and FOLD: the block runs on what is known of each lane;
    /// for a vector of unknown length, on what is known of any lane, once
    /// per lane it may have.
    pub(crate) fn higher(&mut self, w: Word, stack: &mut Vec<Abs>) -> Abs {
        let n = if w == Word::Fold { 3 } else { 2 };
        let mut o = self.pop(stack, n);
        let Some(Abs::Quote(code)) = o.pop() else {
            return Abs::Top;
        };
        let init = if w == Word::Fold {
            let v = o.pop().expect("three operands");
            Some(self.data(v))
        } else {
            None
        };
        let v = self.data(o.pop().expect("an operand"));
        match (w, v) {
            (_, Abs::Nil) => Abs::Nil,
            (Word::Map, Abs::Lanes(xs)) => {
                Abs::Lanes(xs.into_iter().map(|x| self.block(&code, vec![x])).collect())
            }
            (Word::Map, Abs::Hull { max, elem }) => Abs::Hull {
                max,
                elem: Box::new(self.block(&code, vec![*elem])),
            },
            (Word::Filter, v) if v.is_vector() => {
                let lanes: Vec<Abs> = match &v {
                    Abs::Lanes(xs) => xs.clone(),
                    _ => vec![any_lane(&v)],
                };
                let agree = lanes.iter().all(|x| {
                    matches!(
                        self.block(&code, vec![x.clone()]),
                        Abs::Nil | Abs::Truth { agree: true }
                    )
                });
                match (agree, &v) {
                    (false, _) => Abs::Top,
                    (true, Abs::Lanes(xs)) if xs.is_empty() => Abs::Lanes(Vec::new()),
                    (true, Abs::Lanes(xs)) => Abs::Hull {
                        max: xs.len(),
                        elem: Box::new(any_lane(&v)),
                    },
                    (true, _) => v,
                }
            }
            (Word::Fold, Abs::Lanes(xs)) => xs.into_iter().fold(init.expect("FOLD"), |acc, x| {
                self.block(&code, vec![acc, x])
            }),
            (Word::Fold, Abs::Hull { max, elem }) => {
                // Joined over every length the vector may have; a state
                // that one more iteration leaves unchanged stays so.
                let mut state = init.expect("FOLD");
                let mut all = state.clone();
                for _ in 0..max {
                    let next = self.block(&code, vec![state.clone(), (*elem).clone()]);
                    let settled = next == state;
                    all = join(&all, &next);
                    state = next;
                    if settled || all == Abs::Top {
                        break;
                    }
                }
                all
            }
            _ => Abs::Top,
        }
    }
}
