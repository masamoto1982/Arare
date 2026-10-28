//! The general tier of a fused walk (`fused_block`): any plain rationals and
//! Booleans, each op charged as the interpreted route charges it.
//!
//! What each op costs on the interpreted route, besides its one step:
//!
//! - `ADD`/`SUB`/`MUL`/`DIV` on two scalars take the scalar fast path, so
//!   `binary_numeric_work` of the operands' widths, charged before the
//!   operation runs; the result is held to the size ceiling.
//! - `POW` that `quickened::small_power` answers is one unit of work, as a
//!   power whose answer fits a machine word is; under a size ceiling below a
//!   machine word it is the interpreted route's to refuse.
//! - `LT`/`GT` on two scalars take the comparison fast path: no work. `EQ` takes it only for two scalars; any other pair is
//!   decided by `pairwise_eq`, which counts nothing.
//! - `FLOOR`, `ROUND`, `NOT`, `AND`, `SELECT` and `BIND` charge nothing else.
//! - A Word run by its plain law (`fusion_contract`) charges what that law
//!   reports, which is what its dispatch charges.
//!
//! The two points over zero are numbers here like any other: `ADD`/`SUB`/
//! `MUL`/`DIV` answer them, and a zero divisor, by the `Fraction`'s total
//! arithmetic, as the fast path does. An operand outside the domain a Word
//! accepts is that Word's ERROR, and an order asked of `0/0` its `domainMiss`
//! NIL; both answer `None` here, for the ordinary walk to reproduce with its
//! diagnostics.

use crate::interpreter::arithmetic::ExactArithmeticSchema;
use crate::interpreter::dense_kernels::floor_pair;
use crate::interpreter::fused_block::{Charges, Compare, FusedBlock, FusedWalk, Op, Plain, Src};
use crate::interpreter::runtime_limits::{
    binary_numeric_work, fraction_result_bits, fraction_work_bits,
};
use crate::interpreter::Interpreter;
use crate::types::fraction::{Fraction, FractionRepr};
use crate::types::small_rational::{self, add_total, div_total, mul_total, order_total, Pair};
use crate::types::{DenseTensor, Value, ValueData};
use std::sync::Arc;

/// Every element of `target` as a plain value, or `None` if any is not.
///
/// A one-dimensional dense Tensor — what `RANGE` and every arithmetic Word
/// over a Vector build — is read straight from its columns; `Value::child`
/// would box each lane as a `Value` only for this to unbox it again.
pub(crate) enum Elements<'a> {
    Columns(&'a [i64], &'a [i64]),
    Plains(Vec<Plain>),
}

impl Elements<'_> {
    pub(crate) fn of(target: &Value) -> Option<Elements<'_>> {
        if let ValueData::Tensor { data, shape } = &target.data {
            if shape.len() == 1 {
                return Some(Elements::Columns(&data.numerators, &data.denominators));
            }
        }
        (0..target.len())
            .map(|i| target.child(i).and_then(|v| Plain::of(&v)))
            .collect::<Option<_>>()
            .map(Elements::Plains)
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            Elements::Columns(nums, _) => nums.len(),
            Elements::Plains(plains) => plains.len(),
        }
    }

    #[inline]
    pub(crate) fn at(&self, i: usize) -> Plain {
        match self {
            Elements::Columns(nums, dens) => small((nums[i], dens[i])),
            Elements::Plains(plains) => plains[i].clone(),
        }
    }
}

/// The answers of a `MAP` or the kept elements of a `FILTER`: the two
/// columns a dense Tensor stores while every one is a machine-word rational,
/// the truth values while every one is a Boolean, and plain values once they
/// are neither.
pub(crate) enum Gather {
    Pairs(Vec<i64>, Vec<i64>),
    Truths(Vec<bool>),
    Plains(Vec<Plain>),
}

impl Gather {
    pub(crate) fn with_capacity(n: usize) -> Self {
        Gather::Pairs(Vec::with_capacity(n), Vec::with_capacity(n))
    }

    #[inline(always)]
    pub(crate) fn push_pair(&mut self, (n, d): Pair) {
        match self {
            Gather::Pairs(nums, dens) => {
                nums.push(n);
                dens.push(d);
            }
            _ => self.push_plain(small((n, d))),
        }
    }

    #[inline(always)]
    pub(crate) fn push_truth(&mut self, b: bool) {
        match self {
            Gather::Truths(truths) => truths.push(b),
            Gather::Pairs(nums, _) if nums.is_empty() => *self = Gather::Truths(vec![b]),
            _ => self.push_plain(Plain::Bool(b)),
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, value: Plain) {
        match (&value, num_pair(&value)) {
            (_, Some(pair)) => self.push_pair(pair),
            (Plain::Bool(b), _) => self.push_truth(*b),
            _ => self.push_plain(value),
        }
    }

    #[cold]
    fn push_plain(&mut self, value: Plain) {
        let plains = match std::mem::replace(self, Gather::Plains(Vec::new())) {
            Gather::Pairs(nums, dens) => nums.into_iter().zip(dens).map(small).collect(),
            Gather::Truths(truths) => truths.into_iter().map(Plain::Bool).collect(),
            Gather::Plains(plains) => plains,
        };
        *self = Gather::Plains(plains);
        if let Gather::Plains(plains) = self {
            plains.push(value);
        }
    }

    /// `Value::from_vector_promoted` of what was gathered. Booleans are
    /// never promoted: a Vector of them is what `from_truths` builds.
    pub(crate) fn finish(self) -> Value {
        match self {
            Gather::Plains(plains) => promote(plains),
            Gather::Truths(truths) => Value::from_truths(truths.into_iter()),
            Gather::Pairs(nums, _) if nums.is_empty() => Value::from_vector(Vec::new()),
            Gather::Pairs(nums, dens) => {
                let shape = vec![nums.len()];
                let integer = dens.iter().all(|d| *d == 1);
                let tensor = DenseTensor::from_columns(nums, dens, shape.clone(), integer);
                Value::new(
                    ValueData::Tensor {
                        data: Arc::new(tensor),
                        shape: Arc::new(shape),
                    },
                    None,
                )
            }
        }
    }
}

#[inline(always)]
pub(crate) fn num_pair(p: &Plain) -> Option<Pair> {
    match p {
        Plain::Num(Fraction {
            repr: FractionRepr::Small(n, d),
        }) => Some((*n, *d)),
        _ => None,
    }
}

#[inline(always)]
pub(crate) fn small((n, d): Pair) -> Plain {
    Plain::Num(Fraction::from_repr(FractionRepr::Small(n, d)))
}

/// `Value::from_vector_promoted` of plain values. A non-empty run of
/// rationals that each fit a machine word is the dense Tensor promotion
/// builds for it, built from the rationals directly; anything else goes
/// through `from_vector_promoted` itself.
pub(crate) fn promote(values: Vec<Plain>) -> Value {
    let fits = |p: &Plain| matches!(p, Plain::Num(f) if f.extract_i64_pair().is_some());
    if !values.is_empty() && values.iter().all(fits) {
        let shape = vec![values.len()];
        let fractions: Vec<Fraction> = values
            .into_iter()
            .map(|p| match p {
                Plain::Num(f) => f,
                Plain::Bool(_) => unreachable!("checked above"),
            })
            .collect();
        let tensor =
            DenseTensor::from_fractions(fractions, shape.clone()).expect("every lane fits");
        return Value::new(
            ValueData::Tensor {
                data: Arc::new(tensor),
                shape: Arc::new(shape),
            },
            None,
        );
    }
    Value::from_vector_promoted(values.into_iter().map(Plain::into_value).collect())
}

struct Meter<'a> {
    interp: &'a Interpreter,
    work: u64,
    work_budget: u64,
    /// Whether the size ceiling admits every machine-word rational, so the
    /// pair laws may answer without asking it.
    pairs_fit: bool,
}

impl Meter<'_> {
    fn charge(&mut self, work: u64) -> Option<()> {
        self.work = self.work.saturating_add(work);
        (self.work <= self.work_budget).then_some(())
    }

    fn arith(
        &mut self,
        schema: ExactArithmeticSchema,
        a: &Fraction,
        b: &Fraction,
    ) -> Option<Fraction> {
        self.charge(binary_numeric_work(
            fraction_work_bits(a),
            fraction_work_bits(b),
        ))?;
        let r = schema.fraction(a, b);
        self.interp
            .runtime_limits
            .check_algebraic_size(0, fraction_result_bits(&r))
            .ok()?;
        Some(r)
    }

    /// One instruction on two machine-word rationals, by `small_rational`'s
    /// pair laws: the value `Fraction` reaches (a lowest-terms form with a
    /// positive denominator is unique, and the `_total` laws answer the points
    /// over zero as `Fraction` does), charged as the meter prices a machine
    /// word, one limb. `None` for anything else, or an answer past a pair,
    /// which `Fraction` answers.
    #[inline(always)]
    fn pairs(&mut self, op: Op, a: &Plain, b: &Plain) -> Option<Plain> {
        use std::cmp::Ordering::{Greater, Less};
        let x = num_pair(a)?;
        match op {
            Op::Floor => return Some(small(floor_pair(x))),
            Op::Round if x.1 == 0 => return Some(small(x)),
            Op::Round => {
                return Some(small((
                    small_rational::round_half_away_from_zero(x.0, x.1),
                    1,
                )))
            }
            _ => {}
        }
        let y = num_pair(b)?;
        let answer = match op {
            Op::Arith(ExactArithmeticSchema::Add) => add_total(x, y, false)?,
            Op::Arith(ExactArithmeticSchema::Sub) => add_total(x, y, true)?,
            Op::Arith(ExactArithmeticSchema::Mul) => mul_total(x, y)?,
            Op::Arith(ExactArithmeticSchema::Div) => div_total(x, y)?,
            Op::FloorDiv => floor_pair(div_total(x, y)?),
            Op::Compare(Compare::Lt) => return Some(Plain::Bool(order_total(x, y)? == Less)),
            Op::Compare(Compare::Gt) => return Some(Plain::Bool(order_total(x, y)? == Greater)),
            // The left operand on a tie, as MIN and MAX keep it.
            Op::Extremum { max } => {
                let take_right = if max {
                    order_total(x, y)? == Less
                } else {
                    order_total(y, x)? == Less
                };
                return Some(small(if take_right { y } else { x }));
            }
            _ => return None,
        };
        self.charge(binary_numeric_work(1, 1))?;
        Some(small(answer))
    }

    /// One instruction on plain operands.
    fn apply(&mut self, op: Op, a: Plain, b: Plain, c: Plain) -> Option<Plain> {
        use std::cmp::Ordering::{Greater, Less};
        if self.pairs_fit {
            if let Some(answer) = self.pairs(op, &a, &b) {
                return Some(answer);
            }
        }
        Some(match (op, a, b) {
            (Op::Arith(schema), Plain::Num(a), Plain::Num(b)) => {
                Plain::Num(self.arith(schema, &a, &b)?)
            }
            (Op::FloorDiv, Plain::Num(a), Plain::Num(b)) => {
                Plain::Num(self.arith(ExactArithmeticSchema::Div, &a, &b)?.floor())
            }
            (Op::Pow, Plain::Num(x), Plain::Num(y)) => {
                let r = crate::interpreter::quickened::small_power(
                    x.extract_i64_pair()?,
                    y.extract_i64_pair()?,
                )?;
                if self.interp.runtime_limits.max_bigint_bits < 64 {
                    return None;
                }
                self.charge(binary_numeric_work(1, 1))?;
                Plain::Num(Fraction::from_repr(
                    crate::types::fraction::FractionRepr::Small(r.0, r.1),
                ))
            }
            (Op::Kernel(kernel), a, b) => {
                let operands = [a, b, c];
                let answer = (kernel.apply)(&operands[..kernel.arity], self.interp)?;
                self.charge(answer.work)?;
                answer.value
            }
            (Op::Floor, Plain::Num(f), _) => Plain::Num(f.floor()),
            (Op::Round, Plain::Num(f), _) => Plain::Num(f.round()),
            (Op::Not, Plain::Bool(x), _) => Plain::Bool(!x),
            (Op::And, Plain::Bool(x), Plain::Bool(y)) => Plain::Bool(x && y),
            (Op::Select, when_true, when_false) => match c {
                Plain::Bool(true) => when_true,
                Plain::Bool(false) => when_false,
                Plain::Num(_) => return None,
            },
            // The left operand on a tie, as MIN and MAX keep it.
            (Op::Extremum { max }, Plain::Num(x), Plain::Num(y)) => {
                let take_right = if max {
                    x.order(&y)? == Less
                } else {
                    y.order(&x)? == Less
                };
                Plain::Num(if take_right { y } else { x })
            }
            (Op::Compare(Compare::Lt), Plain::Num(x), Plain::Num(y)) => {
                Plain::Bool(x.order(&y)? == Less)
            }
            (Op::Compare(Compare::Gt), Plain::Num(x), Plain::Num(y)) => {
                Plain::Bool(x.order(&y)? == Greater)
            }
            (Op::Compare(Compare::Eq), x, y) => Plain::Bool(x == y),
            _ => return None,
        })
    }
}

/// For each instruction, which of its operands (bit 0 for `a`, 1 for `b`, 2
/// for `c`) is the last read of its register: that one is moved out rather
/// than cloned, so a growing accumulator is not copied every run.
fn last_reads(block: &FusedBlock) -> Vec<u8> {
    let mut read = vec![false; block.inputs + block.instrs.len()];
    if let Src::Reg(r) = block.out {
        read[r] = true;
    }
    let mut moves = vec![0u8; block.instrs.len()];
    for (at, ins) in block.instrs.iter().enumerate().rev() {
        for i in (0..ins.op.arity()).rev() {
            if let Src::Reg(r) = [ins.a, ins.b, ins.c][i] {
                if !read[r] {
                    read[r] = true;
                    moves[at] |= 1 << i;
                }
            }
        }
    }
    moves
}

/// Run the program once on `regs`, whose first registers hold the inputs,
/// answering its result.
#[inline]
fn run_once(
    block: &FusedBlock,
    moves: &[u8],
    meter: &mut Meter,
    regs: &mut [Plain],
) -> Option<Plain> {
    let inputs = block.inputs;
    for (at, (ins, &moved)) in block.instrs.iter().zip(moves).enumerate() {
        let mut read = |s: Src, bit: u8| match s {
            Src::Reg(r) if moved & bit != 0 => std::mem::replace(&mut regs[r], Plain::Bool(false)),
            Src::Reg(r) => regs[r].clone(),
            Src::Const(k) => block.consts[k].clone(),
        };
        let arity = ins.op.arity();
        let a = read(ins.a, 1);
        let b = if arity > 1 {
            read(ins.b, 2)
        } else {
            Plain::Bool(false)
        };
        let c = if arity > 2 {
            read(ins.c, 4)
        } else {
            Plain::Bool(false)
        };
        regs[inputs + at] = meter.apply(ins.op, a, b, c)?;
    }
    Some(match block.out {
        Src::Reg(r) => std::mem::replace(&mut regs[r], Plain::Bool(false)),
        Src::Const(k) => block.consts[k].clone(),
    })
}

pub(crate) fn run(
    block: &FusedBlock,
    interp: &Interpreter,
    walk: FusedWalk,
    target: &Value,
    seed: Option<&Plain>,
) -> Option<(Value, Charges)> {
    let elements = Elements::of(target)?;
    let runs = elements.len() as u64;
    let steps = block.steps_within_ceiling(interp, runs)?;
    let mut meter = Meter {
        interp,
        work: 0,
        work_budget: interp
            .runtime_limits
            .max_numeric_work
            .checked_sub(interp.numeric_work_used)?,
        pairs_fit: interp.runtime_limits.max_bigint_bits >= 64,
    };

    let moves = last_reads(block);
    let mut accumulator = seed.cloned();
    let mut gathered = Gather::with_capacity(match walk {
        FusedWalk::Map | FusedWalk::Filter => elements.len(),
        _ => 0,
    });
    let mut visited: Vec<Value> = Vec::new();
    let mut regs: Vec<Plain> = vec![Plain::Bool(false); block.inputs + block.instrs.len()];
    for i in 0..elements.len() {
        match accumulator.take() {
            Some(acc) => {
                regs[0] = acc;
                regs[1] = elements.at(i);
            }
            None => regs[0] = elements.at(i),
        }
        let result = run_once(block, &moves, &mut meter, &mut regs)?;
        match walk {
            FusedWalk::Map => gathered.push(result),
            FusedWalk::Filter => match result {
                Plain::Bool(true) => gathered.push(elements.at(i)),
                Plain::Bool(false) => {}
                // A predicate must decide in the truth domain.
                Plain::Num(_) => return None,
            },
            FusedWalk::Scan => {
                visited.push(result.clone().into_value());
                accumulator = Some(result);
            }
            FusedWalk::Fold => accumulator = Some(result),
        }
    }

    let value = match walk {
        FusedWalk::Map | FusedWalk::Filter => gathered.finish(),
        FusedWalk::Scan => Value::from_vector(visited),
        FusedWalk::Fold => accumulator?.into_value(),
    };
    let charges = Charges {
        steps,
        work: meter.work,
    };
    Some((value, charges))
}
