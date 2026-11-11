//! The checker over a program (AR-LLR-020 and AR-LLR-021, refining
//! AR-HLR-027 to AR-HLR-029).
//!
//! It runs the program the way the reference interpreter does
//! (`arare-interp`), item by item and Word by Word, but on what is known of
//! each value instead of the value: each numeric Word applies its function
//! from [`crate::info`], lane by lane, and every other Word follows the
//! interpreter's rule for its operands' kinds. It never reads a value a Word
//! computes, only literals and the Words around them, and always takes the
//! same steps on the same program (WORK_ORDER.md §5.5).
//!
//! Along the way it notes every numeric result it cannot bound as a number:
//! the program is `field=closed` exactly when there is none (AR-HLR-028).

use crate::info::{self, Info};
use crate::value::{eq_agrees, max_len, Abs};
use crate::words::any_lane;
use arare_front::{Item, Program, Word};
use arare_kernel::{Num, Width, MAX_SCALE};
use std::rc::Rc;

/// The grid results are coarsened to, `2^-48`: far below any scale a value
/// has (at least `2^-31`), and coarse enough that the product of two such
/// rationals stays within 128 bits.
pub const COARSE: u32 = 48;

/// Why a numeric result is not bounded as a number.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Why {
    /// A wheel point is written, or flows into the result.
    WheelPoint,
    /// The result may leave the widest width, or the width NARROW or
    /// QUANTIZE names.
    MayOverflow,
    /// The divisor of a DIV may be zero.
    DivisorMayBeZero,
    /// An operand is not bounded, so neither is the result.
    Unbounded,
}

impl Why {
    pub fn name(self) -> &'static str {
        match self {
            Why::WheelPoint => "wheelPoint",
            Why::MayOverflow => "mayOverflow",
            Why::DivisorMayBeZero => "divisorMayBeZero",
            Why::Unbounded => "unbounded",
        }
    }
}

/// A Word, or a literal (`None`), whose result is not bounded, and why.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Exit {
    pub word: Option<Word>,
    pub why: Why,
}

/// What the checker found: what is known of each value on the final stack,
/// bottom first, and what keeps the program from being `field=closed`.
#[derive(Clone, PartialEq, Debug)]
pub struct Analysis {
    pub stack: Vec<Abs>,
    /// Sorted and without repeats.
    pub exits: Vec<Exit>,
    /// The comparisons (LT, EQ) whose answer the checker cannot show to be
    /// the exact one, sorted and without repeats (AR-HLR-039).
    pub undecided: Vec<Word>,
}

impl Analysis {
    /// AR-HLR-028: the checker bounds every numeric result as a number.
    pub fn closed(&self) -> bool {
        self.exits.is_empty()
    }

    /// AR-HLR-039: every comparison the program makes answers what the exact
    /// meaning answers, so every truth value it computes is the exact one.
    pub fn decided(&self) -> bool {
        self.undecided.is_empty()
    }
}

/// Check a parsed program.
pub fn analyze(program: &Program) -> Analysis {
    let mut c = Checker {
        exits: Vec::new(),
        undecided: Vec::new(),
        scopes: vec![Vec::new()],
    };
    let mut stack = Vec::new();
    for item in &program.items {
        c.exec(&mut stack, item);
    }
    c.exits.sort();
    c.exits.dedup();
    c.undecided.sort();
    c.undecided.dedup();
    Analysis {
        stack,
        exits: c.exits,
        undecided: c.undecided,
    }
}

pub(crate) struct Checker {
    exits: Vec<Exit>,
    undecided: Vec<Word>,
    /// What is known of the value of each name bound so far, scope by scope,
    /// as `arare-interp`'s `Env` holds the values (AR-LLR-028).
    scopes: Vec<Vec<(Rc<str>, Abs)>>,
}

/// The numeric Words that apply lane by lane to two numbers.
fn arithmetic(w: Word, a: Info, b: Info) -> Info {
    match w {
        Word::Add => info::add(a, b),
        Word::Sub => info::sub(a, b),
        Word::Mul => info::mul(a, b),
        _ => info::div(a, b),
    }
}

/// An operand that must be the same integer in both meanings.
fn known(v: &Abs) -> Option<i64> {
    match v {
        Abs::Num { info, nil: false } => info::known_int(*info),
        _ => None,
    }
}

fn width(v: &Abs) -> Option<Width> {
    let bits = known(v)?;
    Width::ALL.into_iter().find(|w| w.bits() as i64 == bits)
}

impl Checker {
    pub(crate) fn note(&mut self, word: Option<Word>, why: Why) {
        self.exits.push(Exit { word, why });
    }

    /// A comparison whose answer may differ from the exact one (AR-LLR-044).
    pub(crate) fn undecided(&mut self, word: Word) {
        self.undecided.push(word);
    }

    /// A numeric result: noted when it is not bounded as a number. Its range
    /// and error are coarsened to multiples of `2^-COARSE` once their
    /// denominators grow past it (AR-LLR-030), so that a long loop keeps its
    /// rationals within 128 bits.
    pub(crate) fn result(&mut self, w: Word, operands: &[Info], out: Info) -> Info {
        let out = info::coarsen(out, COARSE);
        if !out.is_num() {
            let why = if matches!(out, Info::Inf | Info::Bot)
                || operands.iter().any(|i| matches!(i, Info::Inf | Info::Bot))
            {
                Why::WheelPoint
            } else if operands.contains(&Info::Top) {
                Why::Unbounded
            } else if w == Word::Div {
                Why::DivisorMayBeZero
            } else {
                Why::MayOverflow
            };
            self.note(Some(w), why);
        }
        out
    }

    pub(crate) fn exec(&mut self, stack: &mut Vec<Abs>, item: &Item) {
        match item {
            Item::Int(k) => {
                let n = Num::int(*k).expect("the front end checked the range");
                stack.push(Abs::num(info::int(n)));
            }
            Item::Num { num, exact: (p, q) } => {
                let i = info::lit(*p, *q, num.width(), num.scale(), num.val());
                if !i.is_num() {
                    let why = if *q == 0 {
                        Why::WheelPoint
                    } else {
                        Why::MayOverflow
                    };
                    self.note(None, why);
                }
                stack.push(Abs::num(i));
            }
            Item::Quote(items) => stack.push(Abs::Quote(Rc::new(items.clone()))),
            Item::Text(name) => stack.push(Abs::Text(name.as_str().into())),
            // A name the checker cannot see bound is one the interpreter
            // stops at, or one bound where the checker knows nothing.
            Item::Name(name) => stack.push(self.lookup(name)),
            Item::Word(w) => self.apply(*w, stack),
        }
    }

    fn lookup(&self, name: &str) -> Abs {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.iter().find(|(n, _)| &**n == name).map(|(_, v)| v.clone()))
            .unwrap_or(Abs::Top)
    }

    fn bind_one(&mut self, name: Rc<str>, value: Abs) {
        let scope = self.scopes.last_mut().expect("the program's scope");
        match scope.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value,
            None => scope.push((name, value)),
        }
    }

    /// BIND, as the interpreter binds: one name takes the value; a block of
    /// names takes the lanes of a vector of known length, and any lane of one
    /// of unknown length.
    fn bind(&mut self, stack: &mut Vec<Abs>) {
        let mut o = self.pop(stack, 2);
        let names = o.pop().expect("two operands");
        let value = o.pop().expect("two operands");
        match names {
            Abs::Text(name) => self.bind_one(name, value),
            names => {
                let Abs::Lanes(names) = self.data(names) else {
                    return;
                };
                let value = self.data(value);
                for (k, n) in names.into_iter().enumerate() {
                    let v = match &value {
                        Abs::Lanes(xs) => xs.get(k).cloned().unwrap_or(Abs::Top),
                        v if v.is_vector() => any_lane(v),
                        _ => Abs::Top,
                    };
                    if let Abs::Text(n) = n {
                        self.bind_one(n, v);
                    }
                }
            }
        }
    }

    /// Run a block on `inputs`; what it leaves when it leaves one value.
    pub(crate) fn block(&mut self, items: &[Item], inputs: Vec<Abs>) -> Abs {
        self.scopes.push(Vec::new());
        let mut stack = inputs;
        for item in items {
            self.exec(&mut stack, item);
        }
        self.scopes.pop();
        match stack.len() {
            1 => stack.pop().expect("one value"),
            _ => Abs::Top,
        }
    }

    /// A value as data: a quote is evaluated, on a stack of its own, into the
    /// vector of what it leaves.
    pub(crate) fn data(&mut self, v: Abs) -> Abs {
        match v {
            Abs::Quote(items) => {
                let mut stack = Vec::new();
                for item in items.iter() {
                    self.exec(&mut stack, item);
                }
                Abs::Lanes(stack.into_iter().map(|x| self.data(x)).collect())
            }
            other => other,
        }
    }

    pub(crate) fn pop(&mut self, stack: &mut Vec<Abs>, n: usize) -> Vec<Abs> {
        let have = stack.len().min(n);
        let mut out = vec![Abs::Top; n - have];
        out.extend(stack.split_off(stack.len() - have));
        out
    }

    pub(crate) fn pop_data(&mut self, stack: &mut Vec<Abs>, n: usize) -> Vec<Abs> {
        let o = self.pop(stack, n);
        o.into_iter().map(|v| self.data(v)).collect()
    }

    /// Lane-wise application, as `arare_interp::lift::lift2`: NIL is the
    /// result; vectors pair lane by lane, a one-lane vector or a scalar is
    /// reused, and a vector of unknown length pairs with whatever its lanes
    /// may meet.
    pub(crate) fn lift2(
        &mut self,
        w: Word,
        a: &Abs,
        b: &Abs,
        f: &mut dyn FnMut(&mut Checker, &Abs, &Abs) -> Abs,
    ) -> Abs {
        use Abs::*;
        match (a, b) {
            (Nil, _) | (_, Nil) => Nil,
            (Top, _) | (_, Top) => {
                self.note(Some(w), Why::Unbounded);
                if w == Word::Lt {
                    self.undecided(w);
                }
                Top
            }
            (Lanes(xs), Lanes(ys)) => match (xs.len(), ys.len()) {
                (n, m) if n == m => Lanes(
                    xs.iter()
                        .zip(ys)
                        .map(|(x, y)| self.lift2(w, x, y, f))
                        .collect(),
                ),
                (1, _) => Lanes(ys.iter().map(|y| self.lift2(w, &xs[0], y, f)).collect()),
                (_, 1) => Lanes(xs.iter().map(|x| self.lift2(w, x, &ys[0], f)).collect()),
                _ => Top,
            },
            (x, y) if x.is_vector() && y.is_vector() => {
                let (p, q) = (any_lane(x), any_lane(y));
                Hull {
                    max: max_len(x).max(max_len(y)),
                    elem: Box::new(self.lift2(w, &p, &q, f)),
                }
            }
            (Lanes(xs), y) => Lanes(xs.iter().map(|x| self.lift2(w, x, y, f)).collect()),
            (x, Lanes(ys)) => Lanes(ys.iter().map(|y| self.lift2(w, x, y, f)).collect()),
            (Hull { max, elem }, y) => Hull {
                max: *max,
                elem: Box::new(self.lift2(w, elem, y, f)),
            },
            (x, Hull { max, elem }) => Hull {
                max: *max,
                elem: Box::new(self.lift2(w, x, elem, f)),
            },
            (x, y) => f(self, x, y),
        }
    }

    pub(crate) fn lift1(
        &mut self,
        w: Word,
        a: &Abs,
        f: &mut dyn FnMut(&mut Checker, Info) -> Info,
    ) -> Abs {
        match a {
            Abs::Nil => Abs::Nil,
            Abs::Num { info, nil } => Abs::Num {
                info: f(self, *info),
                nil: *nil,
            },
            Abs::Lanes(xs) => Abs::Lanes(xs.iter().map(|x| self.lift1(w, x, f)).collect()),
            Abs::Hull { max, elem } => Abs::Hull {
                max: *max,
                elem: Box::new(self.lift1(w, elem, f)),
            },
            _ => {
                self.note(Some(w), Why::Unbounded);
                Abs::Top
            }
        }
    }

    /// A numeric Word on two numbers, lane by lane.
    pub(crate) fn numeric2(&mut self, w: Word, a: &Abs, b: &Abs) -> Abs {
        self.lift2(w, a, b, &mut |c, x, y| match (x, y) {
            (Abs::Num { info: p, nil: n }, Abs::Num { info: q, nil: m }) => Abs::Num {
                info: c.result(w, &[*p, *q], arithmetic(w, *p, *q)),
                nil: *n || *m,
            },
            _ => {
                c.note(Some(w), Why::Unbounded);
                Abs::Top
            }
        })
    }

    /// A numeric Word on one number, lane by lane; nothing is known when its
    /// operand `arg` is not the same in both meanings.
    pub(crate) fn numeric1(&mut self, w: Word, a: &Abs, g: Option<&dyn Fn(Info) -> Info>) -> Abs {
        match g {
            Some(g) => self.lift1(w, a, &mut |c, i| c.result(w, &[i], g(i))),
            None => {
                self.note(Some(w), Why::Unbounded);
                Abs::Top
            }
        }
    }

    fn apply(&mut self, w: Word, stack: &mut Vec<Abs>) {
        let result = match w {
            Word::Bind => return self.bind(stack),
            Word::True | Word::False => Abs::Truth { agree: true },
            Word::Nil => Abs::Nil,
            Word::Add | Word::Sub | Word::Mul | Word::Div => {
                let o = self.pop_data(stack, 2);
                self.numeric2(w, &o[0], &o[1])
            }
            Word::Neg => {
                let o = self.pop_data(stack, 1);
                self.numeric1(w, &o[0], Some(&info::neg))
            }
            Word::Lt => {
                let o = self.pop_data(stack, 2);
                self.less(&o[0], &o[1])
            }
            Word::Eq => {
                let o = self.pop_data(stack, 2);
                match (&o[0], &o[1]) {
                    (Abs::Nil, _) | (_, Abs::Nil) => Abs::Nil,
                    (a, b) => {
                        let agree = eq_agrees(a, b);
                        if !agree {
                            self.undecided(w);
                        }
                        Abs::Truth { agree }
                    }
                }
            }
            Word::And => {
                let o = self.pop_data(stack, 2);
                self.and(&o[0], &o[1])
            }
            Word::Not => {
                let o = self.pop_data(stack, 1);
                self.not(&o[0])
            }
            Word::Rescale => {
                let o = self.pop_data(stack, 2);
                let s = known(&o[1]).filter(|s| (0..=62).contains(s));
                let g = s.map(|s| move |i| info::rescale(i, s as u32));
                self.numeric1(w, &o[0], g.as_ref().map(|g| g as &dyn Fn(Info) -> Info))
            }
            Word::Narrow => {
                let o = self.pop_data(stack, 2);
                let g = width(&o[1]).map(|wd| move |i| info::narrow(i, wd));
                self.numeric1(w, &o[0], g.as_ref().map(|g| g as &dyn Fn(Info) -> Info))
            }
            Word::Quantize => {
                let o = self.pop_data(stack, 3);
                let e = known(&o[2]).filter(|e| e.unsigned_abs() <= MAX_SCALE as u64);
                let g = match (width(&o[1]), e) {
                    (Some(wd), Some(e)) => Some(move |i| info::quantize(i, wd, e as i32)),
                    _ => None,
                };
                self.numeric1(w, &o[0], g.as_ref().map(|g| g as &dyn Fn(Info) -> Info))
            }
            Word::Select => {
                let o = self.pop_data(stack, 3);
                self.select(&o[0], &o[1], &o[2])
            }
            Word::Sum => {
                let o = self.pop_data(stack, 1);
                match &o[0] {
                    Abs::Nil => Abs::Nil,
                    v => self.sum(v),
                }
            }
            Word::Map | Word::Filter | Word::Fold => self.higher(w, stack),
        };
        stack.push(result);
    }
}
