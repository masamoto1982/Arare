//! Lowering a program to the intermediate representation (AR-LLR-024,
//! refining AR-HLR-030 and AR-HLR-031).
//!
//! The program runs once, on a stack of symbols instead of values, the way
//! the reference interpreter would run it (`arare-interp`'s `words.rs`), and
//! each Word adds the node that computes its result. A program, or a part of
//! one, that the fast path does not cover is refused with [`Unsupported`]; the
//! compiled code for it is then the interpreter itself, so a refusal costs
//! speed, never an answer.

use crate::ir::{Input, Ir, Kind, Node, Op, Output, Sum, N};
use arare_front::{parse, Item, Word};
use arare_kernel::{Num, Val, Width};
use std::fmt;
use std::rc::Rc;

/// Why a program, as written for a signature, has no fast path.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Unsupported(pub String);

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) fn no<T>(why: impl Into<String>) -> Result<T, Unsupported> {
    Err(Unsupported(why.into()))
}

/// An operand a Word reads as data: a node, as one value or as the lanes of
/// a vector.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Data {
    pub(crate) n: N,
    pub(crate) vector: bool,
}

/// A stack symbol: a scalar node (inside MAP, a lane node is a scalar), a
/// vector whose lanes are a node, or a quote.
#[derive(Clone, Debug)]
pub(crate) enum Sym {
    Scalar(N),
    Vector(N),
    Quote(Rc<Vec<Item>>),
}

pub(crate) struct Lower {
    nodes: Vec<Node>,
    sums: Vec<Sum>,
}

/// A number of this width and scale, for asking the kernel what width and
/// scale an operation gives.
fn dummy(width: Width, scale: i32) -> Num {
    Num::new(Val::Fin(0), width, scale).expect("a node's scale is within ±31")
}

/// `x << s` as a magnitude bound. Bounds are taken within a width, so below
/// 2^63, and shifts align scales within ±31, so at most 62 bits: the result
/// stays below 2^125.
fn shl(x: u128, s: u32) -> u128 {
    x << s
}

impl Lower {
    pub(crate) fn add(
        &mut self,
        op: Op,
        kind: Kind,
        bound: u128,
        lane: bool,
        late: bool,
    ) -> Result<N, Unsupported> {
        if lane && late {
            return no("a lane-wise value computed from a reduction needs a second pass");
        }
        let (bound, check) = match kind {
            Kind::Num { width, .. } => {
                let max = width.max_finite() as u128;
                (bound, bound > max)
            }
            Kind::Bool => (0, false),
        };
        self.nodes.push(Node {
            op,
            kind,
            bound,
            check,
            lane,
            late,
        });
        Ok(self.nodes.len() - 1)
    }

    pub(crate) fn node(&self, n: N) -> Node {
        self.nodes[n]
    }

    pub(crate) fn num(&self, n: N) -> Result<(Width, i32, u128), Unsupported> {
        let node = self.node(n);
        match node.kind {
            Kind::Num { width, scale } => {
                // A checked node's code fits its width once the check passes.
                let bound = node.bound.min(width.max_finite() as u128);
                Ok((width, scale, bound))
            }
            Kind::Bool => no("a truth value where a number is expected"),
        }
    }

    pub(crate) fn is_bool(&self, n: N) -> bool {
        self.node(n).kind == Kind::Bool
    }

    /// The integer a literal operand is in both meanings, for RESCALE's
    /// shift, NARROW's width and QUANTIZE's exponent.
    pub(crate) fn literal_int(&self, d: &Data) -> Result<i64, Unsupported> {
        if d.vector {
            return no("a vector where a literal integer is expected");
        }
        let node = self.node(d.n);
        match (node.op, node.kind) {
            (Op::Const(c), Kind::Num { width, scale }) => {
                match Num::new(Val::Fin(c), width, scale)
                    .expect("a literal is valid")
                    .exact()
                {
                    (k, 1) => i64::try_from(k).or_else(|_| no("an operand out of range")),
                    _ => no("an operand that is not an integer"),
                }
            }
            _ => no("a computed operand where a literal integer is expected"),
        }
    }

    /// Two operands combined lane by lane: the result is a vector if either
    /// is one.
    pub(crate) fn lanes(&self, a: &Data, b: &Data) -> (N, N, bool) {
        (a.n, b.n, a.vector || b.vector)
    }

    pub(crate) fn wrap(&self, vector: bool, n: N) -> Sym {
        if vector {
            Sym::Vector(n)
        } else {
            Sym::Scalar(n)
        }
    }

    pub(crate) fn flags(&self, xs: &[N]) -> (bool, bool) {
        let lane = xs.iter().any(|&x| self.node(x).lane);
        let late = xs.iter().any(|&x| self.node(x).late);
        (lane, late)
    }

    pub(crate) fn arithmetic(&mut self, w: Word, a: &Data, b: &Data) -> Result<Sym, Unsupported> {
        let (x, y, vector) = self.lanes(a, b);
        let ((wa, sa, ba), (wb, sb, bb)) = (self.num(x)?, self.num(y)?);
        let (da, db) = (dummy(wa, sa), dummy(wb, sb));
        let (lane, late) = self.flags(&[x, y]);
        let n = match w {
            Word::Add | Word::Sub => {
                let r = da.add(db);
                let e = r.scale();
                let (sha, shb) = ((sa - e) as u32, (sb - e) as u32);
                let bound = shl(ba, sha).saturating_add(shl(bb, shb));
                let op = if w == Word::Add {
                    Op::Add {
                        a: x,
                        b: y,
                        da: sha,
                        db: shb,
                    }
                } else {
                    Op::Sub {
                        a: x,
                        b: y,
                        da: sha,
                        db: shb,
                    }
                };
                self.add(
                    op,
                    Kind::Num {
                        width: r.width(),
                        scale: e,
                    },
                    bound,
                    lane,
                    late,
                )?
            }
            Word::Mul => {
                let Ok(r) = da.mul(db) else {
                    return no("MUL's result exponent leaves ±31");
                };
                let kind = Kind::Num {
                    width: r.width(),
                    scale: r.scale(),
                };
                self.add(Op::Mul(x, y), kind, ba.saturating_mul(bb), lane, late)?
            }
            _ => {
                let r = da.div(db);
                let (p, q) = if sb < 0 {
                    ((-sb) as u32, 0)
                } else {
                    (0, sb as u32)
                };
                let kind = Kind::Num {
                    width: r.width(),
                    scale: r.scale(),
                };
                let n = self.add(Op::Div { a: x, b: y, p, q }, kind, shl(ba, p), lane, late)?;
                // A divisor may be zero: the fast path checks every one.
                self.nodes[n].check = true;
                n
            }
        };
        Ok(self.wrap(vector, n))
    }

    pub(crate) fn unary(
        &mut self,
        a: &Data,
        f: impl FnOnce(&mut Lower, N) -> Result<N, Unsupported>,
    ) -> Result<Sym, Unsupported> {
        let n = f(self, a.n)?;
        Ok(self.wrap(a.vector, n))
    }

    pub(crate) fn rescale(&mut self, x: N, s: u32) -> Result<N, Unsupported> {
        let (w, sa, b) = self.num(x)?;
        let Ok(r) = dummy(w, sa).rescale(s) else {
            return no("RESCALE's result exponent leaves ±31");
        };
        let bound = if s == 0 { b } else { b.min((b >> s) + 1) };
        let (lane, late) = self.flags(&[x]);
        self.add(
            Op::Rescale(x, s),
            Kind::Num {
                width: r.width(),
                scale: r.scale(),
            },
            bound,
            lane,
            late,
        )
    }

    pub(crate) fn quantize(&mut self, x: N, w: Width, e: i32) -> Result<N, Unsupported> {
        let (wa, sa, b) = self.num(x)?;
        let r = dummy(wa, sa)
            .quantize(w, e)
            .expect("the Word checked the exponent");
        let (lane, late) = self.flags(&[x]);
        let kind = Kind::Num {
            width: r.width(),
            scale: r.scale(),
        };
        if e >= sa {
            let s = (e - sa) as u32;
            let bound = if s == 0 { b } else { b.min((b >> s) + 1) };
            self.add(Op::Up(x, s), kind, bound, lane, late)
        } else {
            let s = (sa - e) as u32;
            self.add(Op::Down(x, s), kind, shl(b, s), lane, late)
        }
    }

    pub(crate) fn compare(&mut self, w: Word, x: N, y: N) -> Result<N, Unsupported> {
        let ((_, sa, _), (_, sb, _)) = (self.num(x)?, self.num(y)?);
        let e = sa.min(sb);
        let (da, db) = ((sa - e) as u32, (sb - e) as u32);
        let (lane, late) = self.flags(&[x, y]);
        let op = if w == Word::Lt {
            Op::Lt { a: x, b: y, da, db }
        } else {
            Op::Eq { a: x, b: y, da, db }
        };
        self.add(op, Kind::Bool, 0, lane, late)
    }

    /// SUM's result: the width the interpreter's running sum settles at, and
    /// the widths before it, for the run-time check that enough lanes were
    /// summed for the settled width to apply.
    pub(crate) fn sum(&mut self, x: N) -> Result<N, Unsupported> {
        let (w, s, _) = self.num(x)?;
        let elem = dummy(w, s);
        let mut widths = vec![w];
        let mut total = elem;
        loop {
            let next = total.add(elem);
            widths.push(next.width());
            if next.width() == total.width() {
                break;
            }
            total = next;
        }
        widths.pop();
        let last = *widths.last().expect("at least one width");
        // The total is checked at run time — lanes × the lane bound must fit
        // int64, the width the running sum settles at — so after the loop it
        // is a code of that width.
        let n = self.add(
            Op::Sum(x),
            Kind::Num {
                width: last,
                scale: s,
            },
            last.max_finite() as u128,
            false,
            true,
        )?;
        self.sums.push(Sum {
            node: n,
            elem: x,
            widths,
        });
        Ok(n)
    }

    pub(crate) fn pop(&self, stack: &mut Vec<Sym>, k: usize) -> Result<Vec<Sym>, Unsupported> {
        if stack.len() < k {
            return no("a Word with too few operands");
        }
        Ok(stack.split_off(stack.len() - k))
    }

    pub(crate) fn data(&self, s: Sym) -> Result<Data, Unsupported> {
        match s {
            Sym::Scalar(n) => Ok(Data { n, vector: false }),
            Sym::Vector(n) => Ok(Data { n, vector: true }),
            Sym::Quote(_) => no("a vector written as a quote"),
        }
    }

    pub(crate) fn pop_data(
        &self,
        stack: &mut Vec<Sym>,
        k: usize,
    ) -> Result<Vec<Data>, Unsupported> {
        self.pop(stack, k)?
            .into_iter()
            .map(|s| self.data(s))
            .collect()
    }

    pub(crate) fn exec(&mut self, stack: &mut Vec<Sym>, item: &Item) -> Result<(), Unsupported> {
        let sym = match item {
            Item::Int(k) => {
                let n = Num::int(*k).expect("the front end checked the range");
                Sym::Scalar(self.add(
                    Op::Const(*k),
                    Kind::Num {
                        width: n.width(),
                        scale: 0,
                    },
                    k.unsigned_abs() as u128,
                    false,
                    false,
                )?)
            }
            Item::Num { num, .. } => match num.val() {
                Val::Fin(c) => Sym::Scalar(self.add(
                    Op::Const(c),
                    Kind::Num {
                        width: num.width(),
                        scale: num.scale(),
                    },
                    c.unsigned_abs() as u128,
                    false,
                    false,
                )?),
                _ => return no("a literal wheel point"),
            },
            Item::Quote(items) => Sym::Quote(Rc::new(items.clone())),
            Item::Word(w) => self.word(*w, stack)?,
        };
        stack.push(sym);
        Ok(())
    }
}

pub(crate) fn width_of(bits: i64) -> Result<Width, Unsupported> {
    Width::ALL
        .into_iter()
        .find(|w| w.bits() as i64 == bits)
        .map_or_else(|| no("a width other than 8, 16, 32 or 64"), Ok)
}

/// Lower `source` for `inputs`, pushed bottom first before it runs.
pub fn lower(source: &str, inputs: &[Input]) -> Result<Ir, Unsupported> {
    let program =
        parse(source).map_err(|e| Unsupported(format!("refused by the front end: {e}")))?;
    let mut l = Lower {
        nodes: Vec::new(),
        sums: Vec::new(),
    };
    let mut stack = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        let (op, width, scale, vector) = match *input {
            Input::Scalar { width, scale } => (Op::Arg(i), width, scale, false),
            Input::Vector { width, scale } => (Op::Lane(i), width, scale, true),
        };
        if Num::new(Val::Fin(0), width, scale).is_none() {
            return no("an input scale outside ±31");
        }
        let n = l.add(
            op,
            Kind::Num { width, scale },
            width.max_finite() as u128,
            vector,
            false,
        )?;
        stack.push(l.wrap(vector, n));
    }
    for item in &program.items {
        l.exec(&mut stack, item)?;
    }
    let outputs = stack
        .into_iter()
        .map(|s| match s {
            // Only inside MAP is a lane node a scalar.
            Sym::Scalar(n) => Ok(Output::Scalar(n)),
            Sym::Vector(n) => Ok(Output::Vector(n)),
            _ => no("a quote left on the stack"),
        })
        .collect::<Result<_, _>>()?;
    Ok(Ir {
        inputs: inputs.to_vec(),
        nodes: l.nodes,
        outputs,
        sums: l.sums,
    })
}
