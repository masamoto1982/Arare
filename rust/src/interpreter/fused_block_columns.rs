//! The column tier of a walk (`fused_block`): every number a rational whose
//! halves fit machine words, every other value a Boolean, run on the column
//! kernels the element-wise Words use (`dense_kernels`).
//!
//! A `MAP` or `FILTER` runs the block's program a chunk of elements at a time:
//! each instruction is one kernel call over [`CHUNK`] lanes, so its dispatch
//! is paid once per chunk, an all-integer chunk takes the integer kernels
//! (plain array loops the compiler can vectorise), and a rational one the
//! gcd kernels the Words use. A `FOLD` or `SCAN` needs each run's accumulator
//! before the next, so only the instructions that never read it run a column
//! at a time; the rest run once per element on pairs, and a block that is one
//! arithmetic Word on the accumulator — `[ ADD ]`, `[ 3 MUL ADD ]` — on
//! integers gets a loop of its own.
//!
//! The kinds of every register are known before the walk (`FusedBlock::types`),
//! and so are its charges: every number here is a machine-word pair, which the
//! meter prices at one limb, so each arithmetic Word costs
//! `binary_numeric_work(1, 1)`, and every result fits 64 bits for the size
//! ceiling. A lane that would leave a pair — an overflow — or an order asked
//! of a point over zero declines the walk (`None`), for the general tier.
//! The points themselves are pairs like any other, answered by the
//! `Fraction`'s total law, as the kernels answer them.

use std::borrow::Cow;

use crate::interpreter::arithmetic::ExactArithmeticSchema;
use crate::interpreter::comparison::OrderingKind;
use crate::interpreter::dense_kernels::{
    self as kernel, floor_pair, fold_op, Lanes, Out, Rounding,
};
use crate::interpreter::fused_block::{
    Charges, Compare, FusedBlock, FusedWalk, Op, Plain, Prepared, Src, Ty,
};
use crate::interpreter::fused_block_general::small;
use crate::interpreter::Interpreter;
use crate::types::small_rational::{self, add_total, div_total, mul_total, order_total, Pair};
use crate::types::{Value, ValueData};

/// How many elements a chunk holds.
const CHUNK: usize = 256;

/// The elements as two columns, and whether they are numbers or truths.
struct Columns<'a> {
    nums: Cow<'a, [i64]>,
    dens: Cow<'a, [i64]>,
    integer: bool,
    ty: Ty,
}

impl Columns<'_> {
    fn of(target: &Value) -> Option<Columns<'_>> {
        match &target.data {
            ValueData::Tensor { data, shape } if shape.len() == 1 => Some(Columns {
                nums: Cow::Borrowed(&data.numerators),
                dens: Cow::Borrowed(&data.denominators),
                integer: data.is_pure_integer,
                ty: Ty::Num,
            }),
            ValueData::Vector(items) => {
                let (mut nums, mut dens) = (Vec::new(), Vec::new());
                let mut ty = None;
                for item in items.iter() {
                    let (t, (n, d)) = pair_of(&Plain::of(item)?)?;
                    if *ty.get_or_insert(t) != t {
                        return None;
                    }
                    nums.push(n);
                    dens.push(d);
                }
                Some(Columns {
                    integer: dens.iter().all(|&d| d == 1),
                    nums: Cow::Owned(nums),
                    dens: Cow::Owned(dens),
                    ty: ty?,
                })
            }
            _ => None,
        }
    }

    fn chunk(&self, start: usize, end: usize) -> Lanes<'_> {
        Lanes::Columns {
            nums: &self.nums[start..end],
            dens: &self.dens[start..end],
            integer: self.integer,
        }
    }
}

/// The `[start, end)` of each chunk of `n` elements.
fn chunks(n: usize) -> impl Iterator<Item = (usize, usize)> {
    (0..n.div_ceil(CHUNK)).map(move |c| (c * CHUNK, (c * CHUNK + CHUNK).min(n)))
}

/// A plain value as a pair, with its kind; a Boolean is `(0 or 1, 1)`.
fn pair_of(p: &Plain) -> Option<(Ty, Pair)> {
    match p {
        Plain::Num(f) => Some((Ty::Num, f.extract_i64_pair()?)),
        Plain::Bool(b) => Some((Ty::Bool, (i64::from(*b), 1))),
    }
}

fn plain(ty: Ty, pair: Pair) -> Plain {
    match ty {
        Ty::Num => small(pair),
        Ty::Bool => Plain::Bool(pair.0 != 0),
    }
}

/// One instruction over `n` lanes into `out`.
fn apply(op: Op, a: Lanes, b: Lanes, c: Lanes, n: usize, out: &mut Out) -> Option<()> {
    match op {
        Op::Arith(schema) => kernel::arith_into(schema, a, b, n, out),
        Op::FloorDiv => kernel::floor_div_into(a, b, n, out),
        Op::Compare(Compare::Lt) => kernel::ordering_into(OrderingKind::Lt, a, b, n, out),
        Op::Compare(Compare::Gt) => kernel::ordering_into(OrderingKind::Gt, a, b, n, out),
        Op::Compare(Compare::Eq) => {
            kernel::eq_into(a, b, n, out);
            Some(())
        }
        Op::Extremum { max } => kernel::extremum_into(max, a, b, n, out),
        Op::Floor => kernel::rounded_into(Rounding::Floor, a, n, out),
        Op::Round => kernel::rounded_into(Rounding::Round, a, n, out),
        Op::Not => {
            kernel::not_into(a, n, out);
            Some(())
        }
        Op::And => {
            kernel::and_into(a, b, n, out);
            Some(())
        }
        Op::Select => {
            kernel::select_into(a, b, c, n, out);
            Some(())
        }
        Op::Pow => kernel::pow_into(a, b, n, out),
        Op::Kernel(_) => None,
    }
}

/// One instruction on one lane of pairs, as the kernels answer it lane by
/// lane.
fn apply_pair(op: Op, a: Pair, b: Pair, c: Pair) -> Option<Pair> {
    use std::cmp::Ordering::{Greater, Less};
    let truth = |t: bool| (i64::from(t), 1);
    Some(match op {
        Op::Arith(ExactArithmeticSchema::Add) => add_total(a, b, false)?,
        Op::Arith(ExactArithmeticSchema::Sub) => add_total(a, b, true)?,
        Op::Arith(ExactArithmeticSchema::Mul) => mul_total(a, b)?,
        Op::Arith(ExactArithmeticSchema::Div) => div_total(a, b)?,
        Op::FloorDiv => floor_pair(div_total(a, b)?),
        Op::Compare(Compare::Lt) => truth(order_total(a, b)? == Less),
        Op::Compare(Compare::Gt) => truth(order_total(a, b)? == Greater),
        Op::Compare(Compare::Eq) => truth(a == b),
        // The left operand on a tie, as MIN and MAX keep it.
        Op::Extremum { max: true } if order_total(a, b)? == Less => b,
        Op::Extremum { max: false } if order_total(b, a)? == Less => b,
        Op::Extremum { .. } => a,
        Op::Floor => floor_pair(a),
        Op::Round if a.1 == 0 => a,
        Op::Round => (small_rational::round_half_away_from_zero(a.0, a.1), 1),
        Op::Not => truth(a.0 == 0),
        Op::And => truth(a.0 != 0 && b.0 != 0),
        Op::Select if c.0 != 0 => a,
        Op::Select => b,
        Op::Pow => crate::interpreter::quickened::small_power(a, b)?,
        Op::Kernel(_) => return None,
    })
}

/// The lanes `s` names: an input, a register computed earlier in the
/// chunk, or a constant.
fn lanes_of<'a>(s: Src, inputs: &[Lanes<'a>], regs: &'a [Out], consts: &[Pair]) -> Lanes<'a> {
    match s {
        Src::Reg(r) if r < inputs.len() => inputs[r],
        Src::Reg(r) => regs[r - inputs.len()].lanes(),
        Src::Const(k) => Lanes::Splat(consts[k].0, consts[k].1),
    }
}

/// Every instruction not in `skip` over one chunk of `n` lanes.
fn run_chunk(
    block: &FusedBlock,
    consts: &[Pair],
    inputs: &[Lanes],
    regs: &mut [Out],
    skip: &[bool],
    n: usize,
) -> Option<()> {
    for (i, ins) in block.instrs.iter().enumerate() {
        if skip.get(i).copied().unwrap_or(false) {
            continue;
        }
        let (done, rest) = regs.split_at_mut(i);
        let lane = |s| lanes_of(s, inputs, done, consts);
        let arity = ins.op.arity();
        let a = lane(ins.a);
        let b = if arity > 1 { lane(ins.b) } else { a };
        let c = if arity > 2 { lane(ins.c) } else { a };
        apply(ins.op, a, b, c, n, &mut rest[0])?;
    }
    Some(())
}

pub(crate) fn prepare(block: &FusedBlock, inputs: &[Ty]) -> Option<Prepared> {
    let (types, work_per_run) = block.types(inputs)?;
    let consts: Vec<Pair> = block
        .consts
        .iter()
        .map(|p| Some(pair_of(p)?.1))
        .collect::<Option<_>>()?;
    let out_ty = match block.out {
        Src::Reg(r) => types[r],
        Src::Const(k) => pair_of(&block.consts[k])?.0,
    };
    // Which registers depend on the accumulator, however indirectly: those
    // instructions run once per element; every other one a column at a time.
    let mut reads_acc: Vec<bool> = (0..inputs.len())
        .map(|i| i == 0 && inputs.len() == 2)
        .collect();
    for ins in &block.instrs {
        let operands = [ins.a, ins.b, ins.c];
        let reads = operands[..ins.op.arity()]
            .iter()
            .any(|s| matches!(s, Src::Reg(r) if reads_acc[*r]));
        reads_acc.push(reads);
    }
    let per_run = |j: usize| reads_acc[inputs.len() + j];
    let single = match (
        (0..block.instrs.len()).filter(|&j| per_run(j)).count(),
        block.out,
    ) {
        (1, Src::Reg(out)) if out >= 2 && per_run(out - 2) => {
            let ins = block.instrs[out - 2];
            use ExactArithmeticSchema::{Add, Mul, Sub};
            match (ins.op, ins.a, ins.b) {
                (Op::Arith(schema @ (Add | Sub | Mul)), Src::Reg(0), x) if x != Src::Reg(0) => {
                    Some((schema, false, x))
                }
                (Op::Arith(schema @ (Add | Sub | Mul)), x, Src::Reg(0)) if x != Src::Reg(0) => {
                    Some((schema, true, x))
                }
                _ => None,
            }
        }
        _ => None,
    };
    Some(Prepared {
        work_per_run,
        out_ty,
        consts,
        reads_acc,
        single,
    })
}

pub(crate) fn run(
    block: &FusedBlock,
    interp: &Interpreter,
    walk: FusedWalk,
    target: &Value,
    seed: Option<&Plain>,
) -> Option<(Value, Charges)> {
    if interp.runtime_limits.max_bigint_bits < 64 {
        return None;
    }
    let columns = Columns::of(target)?;
    let seed = match seed {
        Some(p) => Some(pair_of(p)?),
        None => None,
    };
    let inputs: smallvec::SmallVec<[Ty; 2]> =
        seed.iter().map(|(t, _)| *t).chain([columns.ty]).collect();
    let prepared = block.preparations.get(block, &inputs)?;
    match walk {
        FusedWalk::Filter if prepared.out_ty != Ty::Bool => return None,
        FusedWalk::Fold | FusedWalk::Scan if Some(prepared.out_ty) != seed.map(|(t, _)| t) => {
            return None
        }
        _ => {}
    }

    let runs = columns.nums.len() as u64;
    let steps = block.steps_within_ceiling(interp, runs)?;
    let work = runs.checked_mul(prepared.work_per_run)?;
    if interp.numeric_work_used.checked_add(work)? > interp.runtime_limits.max_numeric_work {
        return None;
    }

    let value = match seed {
        None => element_walk(block, &prepared, &columns, walk)?,
        Some((_, seed)) => accumulating_walk(block, &prepared, &columns, seed, walk)?,
    };
    #[cfg(test)]
    COLUMN_RUNS.with(|c| c.set(c.get() + 1));
    Some((value, Charges { steps, work }))
}

/// `MAP` and `FILTER`, a chunk at a time.
fn element_walk(
    block: &FusedBlock,
    prepared: &Prepared,
    columns: &Columns,
    walk: FusedWalk,
) -> Option<Value> {
    let (consts, out_ty) = (&prepared.consts[..], prepared.out_ty);
    let n = columns.nums.len();
    let mut regs: Vec<Out> = (0..block.instrs.len()).map(|_| Out::default()).collect();
    let mut gathered = Out::with_capacity(n);
    for (start, end) in chunks(n) {
        let len = end - start;
        let element = columns.chunk(start, end);
        run_chunk(block, consts, &[element], &mut regs, &[], len)?;
        let answer = lanes_of(block.out, &[element], &regs, consts);
        match walk {
            FusedWalk::Map => gathered.extend(answer, len),
            _ => {
                let mut buf = Vec::new();
                let keep = answer.nums(len, &mut buf);
                match element {
                    Lanes::Columns {
                        nums,
                        integer: true,
                        ..
                    } if gathered.integer => gathered.nums.extend(
                        nums.iter()
                            .zip(keep)
                            .filter(|(_, &k)| k != 0)
                            .map(|(&x, _)| x),
                    ),
                    _ => {
                        for (i, &k) in keep.iter().enumerate() {
                            if k != 0 {
                                let (x, d) = element.at(i);
                                gathered.push(x, d);
                            }
                        }
                    }
                }
            }
        }
    }
    let ty = match walk {
        FusedWalk::Map => out_ty,
        _ => columns.ty,
    };
    Some(match ty {
        // Booleans are never promoted: a Vector of them is what
        // `from_truths` builds.
        Ty::Bool => Value::from_truths(gathered.nums.iter().map(|&t| t != 0)),
        Ty::Num if gathered.nums.is_empty() => Value::from_vector(Vec::new()),
        Ty::Num => gathered.into_value(),
    })
}

/// `FOLD` and `SCAN`: the accumulator in register 0, the element in register
/// 1.
fn accumulating_walk(
    block: &FusedBlock,
    prepared: &Prepared,
    columns: &Columns,
    seed: Pair,
    walk: FusedWalk,
) -> Option<Value> {
    let (consts, out_ty, single) = (&prepared.consts[..], prepared.out_ty, prepared.single);
    let reads_acc = &prepared.reads_acc;
    let per_run = &reads_acc[2..];
    let n = columns.nums.len();
    let scan = walk == FusedWalk::Scan;
    // `[ ADD ]` on integers — the commonest fold, and often over a short row —
    // is one loop over the whole column, with nothing set up for chunks.
    if let Some((schema, flipped, x)) = single {
        let whole = Lanes::Columns {
            nums: &columns.nums,
            dens: &columns.dens,
            integer: columns.integer,
        };
        let x = match x {
            Src::Reg(1) => Some(whole),
            Src::Const(k) => Some(Lanes::Splat(consts[k].0, consts[k].1)),
            _ => None,
        };
        if let Some(x) = x.filter(|x| seed.1 == 1 && x.integer()) {
            if !scan {
                let acc = fold_op(schema, flipped, seed.0, x, n, |_| {})?;
                return Some(plain(out_ty, (acc, 1)).into_value());
            }
            let mut visited = Vec::with_capacity(n);
            let each = |a| visited.push(plain(out_ty, (a, 1)).into_value());
            fold_op(schema, flipped, seed.0, x, n, each)?;
            return Some(Value::from_vector(visited));
        }
    }
    // Registers for the instructions that run a column at a time; none when
    // every one reads the accumulator, as in `[ ADD ]`.
    let column_wise = per_run.iter().any(|p| !p);
    let mut regs: Vec<Out> = if column_wise {
        (0..block.instrs.len()).map(|_| Out::default()).collect()
    } else {
        Vec::new()
    };
    let mut scalars: smallvec::SmallVec<[Pair; 16]> = smallvec::SmallVec::new();
    let mut acc = seed;
    let mut visited: Vec<Value> = Vec::with_capacity(if scan { n } else { 0 });
    for (start, end) in chunks(n) {
        let len = end - start;
        let inputs = [Lanes::Splat(0, 1), columns.chunk(start, end)];
        if column_wise {
            run_chunk(block, consts, &inputs, &mut regs, per_run, len)?;
        }
        if let Some((schema, flipped, x)) = single {
            let x = lanes_of(x, &inputs, &regs, consts);
            if acc.1 == 1 && x.integer() {
                acc.0 = if scan {
                    let each = |a| visited.push(plain(out_ty, (a, 1)).into_value());
                    fold_op(schema, flipped, acc.0, x, len, each)?
                } else {
                    fold_op(schema, flipped, acc.0, x, len, |_| {})?
                };
                continue;
            }
        }
        if scalars.is_empty() {
            scalars.resize(2 + block.instrs.len(), (0, 1));
        }
        for i in 0..len {
            scalars[0] = acc;
            for (j, ins) in block.instrs.iter().enumerate() {
                if !per_run[j] {
                    continue;
                }
                let value = |s: Src| match s {
                    Src::Reg(r) if reads_acc[r] => scalars[r],
                    s => lanes_of(s, &inputs, &regs, consts).at(i),
                };
                let arity = ins.op.arity();
                let a = value(ins.a);
                let b = if arity > 1 { value(ins.b) } else { a };
                let c = if arity > 2 { value(ins.c) } else { a };
                scalars[2 + j] = apply_pair(ins.op, a, b, c)?;
            }
            acc = match block.out {
                Src::Reg(r) if reads_acc[r] => scalars[r],
                s => lanes_of(s, &inputs, &regs, consts).at(i),
            };
            if scan {
                visited.push(plain(out_ty, acc).into_value());
            }
        }
    }
    Some(if scan {
        Value::from_vector(visited)
    } else {
        plain(out_ty, acc).into_value()
    })
}

#[cfg(test)]
thread_local! {
    static COLUMN_RUNS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Walks this tier answered on this thread, for tests that pin which tier ran.
#[cfg(test)]
pub(crate) fn column_runs_on_this_thread() -> u64 {
    COLUMN_RUNS.with(|c| c.get())
}
