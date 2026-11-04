//! Lowering each Word (AR-LLR-024, refining AR-HLR-030): the node a Word
//! adds, following the interpreter's rule for the Word (`arare-interp`'s
//! `words.rs`) on symbols instead of values.

use crate::ir::{Kind, Op};
use crate::lower::{no, width_of, Lower, Sym, Unsupported};
use arare_front::Word;
use arare_kernel::MAX_SCALE;

impl Lower {
    pub(crate) fn word(&mut self, w: Word, stack: &mut Vec<Sym>) -> Result<Sym, Unsupported> {
        Ok(match w {
            Word::True | Word::False => {
                Sym::Scalar(self.add(Op::Bool(w == Word::True), Kind::Bool, 0, false, false)?)
            }
            Word::Add | Word::Sub | Word::Mul | Word::Div => {
                let o = self.pop_data(stack, 2)?;
                self.arithmetic(w, &o[0], &o[1])?
            }
            Word::Neg => {
                let o = self.pop_data(stack, 1)?;
                self.unary(&o[0], |l, x| {
                    let (wd, s, b) = l.num(x)?;
                    let (lane, late) = l.flags(&[x]);
                    l.add(
                        Op::Neg(x),
                        Kind::Num {
                            width: wd,
                            scale: s,
                        },
                        b,
                        lane,
                        late,
                    )
                })?
            }
            Word::Rescale => {
                let o = self.pop_data(stack, 2)?;
                let s = self.literal_int(&o[1])?;
                if !(0..=62).contains(&s) {
                    return no("a RESCALE shift outside 0 to 62");
                }
                self.unary(&o[0], |l, x| l.rescale(x, s as u32))?
            }
            Word::Narrow => {
                let o = self.pop_data(stack, 2)?;
                let wd = width_of(self.literal_int(&o[1])?)?;
                self.unary(&o[0], |l, x| {
                    let (_, s, b) = l.num(x)?;
                    let (lane, late) = l.flags(&[x]);
                    l.add(
                        Op::Narrow(x),
                        Kind::Num {
                            width: wd,
                            scale: s,
                        },
                        b,
                        lane,
                        late,
                    )
                })?
            }
            Word::Quantize => {
                let o = self.pop_data(stack, 3)?;
                let wd = width_of(self.literal_int(&o[1])?)?;
                let e = self.literal_int(&o[2])?;
                if e.unsigned_abs() > MAX_SCALE as u64 {
                    return no("a QUANTIZE exponent outside ±31");
                }
                self.unary(&o[0], |l, x| l.quantize(x, wd, e as i32))?
            }
            Word::Lt => {
                let o = self.pop_data(stack, 2)?;
                let (x, y, vector) = self.lanes(&o[0], &o[1]);
                let n = self.compare(w, x, y)?;
                self.wrap(vector, n)
            }
            Word::Eq => {
                let o = self.pop_data(stack, 2)?;
                if o[0].vector || o[1].vector {
                    return no("EQ of vectors compares whole values");
                }
                let (x, y) = (o[0].n, o[1].n);
                match (self.is_bool(x), self.is_bool(y)) {
                    (false, false) => Sym::Scalar(self.compare(w, x, y)?),
                    (true, true) => {
                        let (lane, late) = self.flags(&[x, y]);
                        let op = Op::Eq {
                            a: x,
                            b: y,
                            da: 0,
                            db: 0,
                        };
                        Sym::Scalar(self.add(op, Kind::Bool, 0, lane, late)?)
                    }
                    // A number is never equal to a truth value.
                    _ => Sym::Scalar(self.add(Op::Bool(false), Kind::Bool, 0, false, false)?),
                }
            }
            Word::And => {
                let o = self.pop_data(stack, 2)?;
                let (x, y, vector) = self.lanes(&o[0], &o[1]);
                if !(self.is_bool(x) && self.is_bool(y)) {
                    return no("AND of numbers");
                }
                let (lane, late) = self.flags(&[x, y]);
                let n = self.add(Op::And(x, y), Kind::Bool, 0, lane, late)?;
                self.wrap(vector, n)
            }
            Word::Not => {
                let o = self.pop_data(stack, 1)?;
                self.unary(&o[0], |l, x| {
                    if !l.is_bool(x) {
                        return no("NOT of a number");
                    }
                    let (lane, late) = l.flags(&[x]);
                    l.add(Op::Not(x), Kind::Bool, 0, lane, late)
                })?
            }
            Word::Select => {
                let o = self.pop_data(stack, 3)?;
                // A vector condition picks lane by lane; a scalar one picks a
                // whole branch, so both branches must be of one shape.
                let vector = o[2].vector || o[0].vector;
                if !o[2].vector && o[0].vector != o[1].vector {
                    return no("SELECT between a vector and a scalar");
                }
                let (t, e, c) = (o[0].n, o[1].n, o[2].n);
                if !self.is_bool(c) {
                    return no("SELECT on a number");
                }
                let (kt, ke) = (self.node(t).kind, self.node(e).kind);
                if kt != ke {
                    return no("SELECT between values of different widths or scales");
                }
                let bound = self.node(t).bound.max(self.node(e).bound);
                let bound = match kt {
                    Kind::Num { width, .. } => bound.min(width.max_finite() as u128),
                    Kind::Bool => 0,
                };
                let (lane, late) = self.flags(&[t, e, c]);
                let n = self.add(
                    Op::Select {
                        then: t,
                        otherwise: e,
                        cond: c,
                    },
                    kt,
                    bound,
                    lane,
                    late,
                )?;
                self.wrap(vector, n)
            }
            Word::Sum => {
                let o = self.pop_data(stack, 1)?;
                // Inside MAP every operand is a lane, a scalar.
                if !o[0].vector {
                    return no("SUM of a scalar");
                }
                Sym::Scalar(self.sum(o[0].n)?)
            }
            Word::Map => {
                let mut o = self.pop(stack, 2)?;
                let Some(Sym::Quote(code)) = o.pop() else {
                    return no("MAP without a block");
                };
                let x = self.data(o.pop().expect("two operands"))?;
                if !x.vector {
                    return no("MAP over a scalar");
                }
                let x = x.n;
                let mut inner = vec![Sym::Scalar(x)];
                for item in code.iter() {
                    self.exec(&mut inner, item)?;
                }
                match inner[..] {
                    [Sym::Scalar(r)] => Sym::Vector(r),
                    _ => return no("a MAP block that does not leave one scalar"),
                }
            }
            Word::Filter | Word::Fold => return no("FILTER and FOLD are not compiled in Phase 4a"),
            Word::Nil => return no("a literal NIL"),
        })
    }
}
