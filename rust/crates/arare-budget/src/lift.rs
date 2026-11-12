//! Lifting in the checker (AR-LLR-020, after AR-LLR-014): a Word applied
//! lane by lane on what is known of its operands, aligned as the reference
//! interpreter's `arare_interp::lift` aligns the values themselves.

use crate::check::{Checker, Why};
use crate::value::{max_len, Abs};
use crate::words::any_lane;
use arare_front::Word;

impl Checker {
    /// Lane-wise application, as `arare_interp::lift::lift2`: NIL is the
    /// result, and the operands align as [`Checker::lift`] says.
    pub(crate) fn lift2(
        &mut self,
        w: Word,
        a: &Abs,
        b: &Abs,
        f: &mut dyn FnMut(&mut Checker, &Abs, &Abs) -> Abs,
    ) -> Abs {
        self.lift(w, &[a.clone(), b.clone()], true, &mut |c, o| {
            f(c, &o[0], &o[1])
        })
    }

    /// `f` lifted over `ops`, as `arare_interp::lift` lifts a leaf: a vector
    /// paired only with scalars is walked whatever its nesting; two or more
    /// vectors align from the innermost axis, a lane of length 1 and a vector
    /// of lower rank reused. A vector of unknown length is walked through
    /// what is known of any of its lanes. With `nil_absorbs`, a NIL operand
    /// is the result.
    pub(crate) fn lift(
        &mut self,
        w: Word,
        ops: &[Abs],
        nil_absorbs: bool,
        f: &mut dyn FnMut(&mut Checker, &[Abs]) -> Abs,
    ) -> Abs {
        if nil_absorbs && ops.contains(&Abs::Nil) {
            return Abs::Nil;
        }
        if ops.contains(&Abs::Top) {
            return self.unknown(w);
        }
        let vectors: Vec<(usize, Shape)> = ops
            .iter()
            .enumerate()
            .filter_map(|(k, v)| match v {
                Abs::Lanes(xs) => Some((k, Shape::Lanes(xs))),
                Abs::Hull { max, elem } => Some((k, Shape::Hull(*max, elem))),
                _ => None,
            })
            .collect();
        match vectors[..] {
            [] => f(self, ops),
            [(k, shape)] => {
                let mut with = |c: &mut Checker, lane: &Abs| {
                    let mut o = ops.to_vec();
                    o[k] = lane.clone();
                    c.lift(w, &o, nil_absorbs, f)
                };
                match shape {
                    Shape::Lanes(xs) => Abs::Lanes(xs.iter().map(|x| with(self, x)).collect()),
                    Shape::Hull(max, elem) => Abs::Hull {
                        max,
                        elem: Box::new(with(self, elem)),
                    },
                }
            }
            _ => match ops.iter().map(rank).collect::<Option<Vec<_>>>() {
                Some(ranks) => self.aligned(w, ops, ranks, nil_absorbs, f),
                None => self.unknown(w),
            },
        }
    }

    /// What is known of a lifted result whose operands are not known well
    /// enough to align: nothing, noted as the Word's own result would be.
    fn unknown(&mut self, w: Word) -> Abs {
        if w != Word::Select {
            self.note(Some(w), Why::Unbounded);
        }
        if w == Word::Lt {
            self.undecided(w);
        }
        Abs::Top
    }

    /// Operands of known rank, aligned: those of the highest rank are walked
    /// lane by lane, the others reused whole.
    fn aligned(
        &mut self,
        w: Word,
        ops: &[Abs],
        ranks: Vec<usize>,
        nil_absorbs: bool,
        f: &mut dyn FnMut(&mut Checker, &[Abs]) -> Abs,
    ) -> Abs {
        let top = ranks.iter().copied().max().unwrap_or(0);
        if top == 0 {
            return self.lift(w, ops, nil_absorbs, f);
        }
        let walked: Vec<usize> = (0..ops.len()).filter(|k| ranks[*k] == top).collect();
        let next: Vec<usize> = ranks
            .iter()
            .map(|r| if *r == top { r - 1 } else { *r })
            .collect();
        let known: Option<Vec<&Vec<Abs>>> = walked
            .iter()
            .map(|k| match &ops[*k] {
                Abs::Lanes(xs) => Some(xs),
                _ => None,
            })
            .collect();
        match known {
            Some(lanes) => {
                let mut width = 1;
                for xs in &lanes {
                    let n = xs.len();
                    if n != 1 {
                        if width != 1 && width != n {
                            // Lengths that do not pair: the program stops.
                            return Abs::Top;
                        }
                        width = n;
                    }
                }
                Abs::Lanes(
                    (0..width)
                        .map(|i| {
                            let mut o = ops.to_vec();
                            for (k, xs) in walked.iter().zip(&lanes) {
                                o[*k] = xs[if xs.len() == 1 { 0 } else { i }].clone();
                            }
                            self.aligned(w, &o, next.clone(), nil_absorbs, f)
                        })
                        .collect(),
                )
            }
            None if walked.iter().any(|k| ops[*k] == Abs::Lanes(Vec::new())) => {
                Abs::Lanes(Vec::new())
            }
            None => {
                // A length that is the same in both meanings but not known:
                // every lane pairs with what is known of any lane.
                let max = walked.iter().map(|k| max_len(&ops[*k])).max().unwrap_or(0);
                let mut o = ops.to_vec();
                for k in &walked {
                    o[*k] = any_lane(&ops[*k]);
                }
                Abs::Hull {
                    max,
                    elem: Box::new(self.aligned(w, &o, next, nil_absorbs, f)),
                }
            }
        }
    }
}

/// The rank of a value, as the interpreter's `lift::shape` measures it: 0
/// for a scalar or NIL; `None` when not known, or for a vector known to be
/// ragged, which the interpreter refuses to pair with another vector.
fn rank(v: &Abs) -> Option<usize> {
    match v {
        Abs::Top | Abs::Quote(_) => None,
        Abs::Lanes(xs) => {
            let mut lanes = xs.iter().map(rank);
            let inner = match lanes.next() {
                None => 0,
                Some(r) => r?,
            };
            for r in lanes {
                if r? != inner {
                    return None;
                }
            }
            // Lanes of known lengths that differ: ragged too.
            let mut known = xs.iter().filter_map(|x| match x {
                Abs::Lanes(ys) => Some(ys.len()),
                _ => None,
            });
            if let Some(n) = known.next() {
                if known.any(|m| m != n) {
                    return None;
                }
            }
            Some(inner + 1)
        }
        Abs::Hull { elem, .. } => Some(rank(elem)? + 1),
        _ => Some(0),
    }
}

/// A vector operand: its lanes, or its longest length and every lane.
#[derive(Clone, Copy)]
enum Shape<'a> {
    Lanes(&'a [Abs]),
    Hull(usize, &'a Abs),
}
