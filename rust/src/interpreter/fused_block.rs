//! The column walk: `MAP`, `FILTER`, `FOLD` and `SCAN` run a straight-line
//! block over a Vector's elements without the dispatcher.
//!
//! A higher-order Word runs its block once per element, and each run pays for
//! the whole dispatch apparatus: a scratch stack of boxed `Value`s, a binding
//! frame, the declared NIL contract, the lift check, the meter. For a block
//! like `[ 2 MUL 1 ADD ]` that apparatus is almost all of the time.
//!
//! A block made only of the Words below can do none of the things that
//! apparatus exists for, so long as every operand it meets is a plain
//! rational or Boolean: it mints no NIL, lifts over no container, reads no
//! dictionary and binds only within its own frame.
//!
//! - literals, `TRUE`, `FALSE`, and names bound by `BIND` — in the block, or
//!   in a frame the block can see;
//! - `ADD` `SUB` `MUL` `DIV`, `LT` `GT` `EQ`, `MIN` `MAX`, `FLOOR` `ROUND`,
//!   `NOT` `AND` `SELECT`, and `'NAME' BIND`;
//! - `POW` on a plain base and a non-negative integer exponent whose answer
//!   stays in a machine word, and `[ ... ] LENGTH` on a literal Vector (a
//!   constant): each one step and nothing else, as `quickened` charges them;
//! - any other Word whose contract admits it and that has a plain law
//!   (`fusion_contract`): `GCD`, `NIL?`, `DEPTH`;
//! - in a `MAP` block, one-lane Vector literals (`[ 1 ]`) that meet only
//!   `ADD` `SUB` `MUL` `DIV` and `BIND`, walked as their lane (`lane_flow`);
//! - User Words whose bodies are made of these, inlined.
//!
//! Such a block is lowered once, here, from its compiled code to one program
//! in register form: every value the block's stack would have held is a
//! register written once, a literal is a constant, and a `BIND` or a bound
//! name is nothing at all — the name refers to whichever operand was bound.
//! Every walk runs that one program, on the first lane representation that
//! holds the elements: machine-word rationals and truths a chunk of columns
//! at a time, on the same kernels the element-wise Words use
//! (`fused_block_columns`, `dense_kernels`), or any plain value
//! (`fused_block_general`).
//!
//! The walk is *speculative*. It touches no interpreter state while it runs;
//! it computes what the interpreted walk would have charged — the same steps
//! and the same numeric work — and commits it only if the whole walk finishes
//! without anything it cannot reproduce exactly. A number over zero is a
//! number like any other, answered by the `Fraction`'s total arithmetic
//! (LANG.VALUES.EXACT). An order asked of `0/0` (a `domainMiss` NIL), an
//! operand of the wrong domain (an ERROR), a result past the size ceiling or a
//! budget the walk would exhaust answers `None`, and the caller runs the
//! ordinary walk from the start. Because a block is pure
//! (LANG.DICTIONARY.ACYCLIC, no effects), running it twice — once abandoned —
//! is unobservable; and because the committed charges are the interpreted
//! walk's own, which route ran is unobservable too (LANG.AUTHORITY.FREEDOM).
//! `fused_block_tests` holds the routes equal.

use crate::interpreter::arithmetic::ExactArithmeticSchema;
use crate::interpreter::engine::Code;
use crate::interpreter::runtime_limits::binary_numeric_work;
use crate::interpreter::Interpreter;
use crate::types::fraction::Fraction;
use crate::types::{Value, ValueData};

/// A plain value: the only kind the walk reproduces exactly. Each one is
/// rebuilt by the constructor that built it on the interpreted route
/// (`Value::from_fraction`, `Value::from_bool`), so nothing is lost by holding
/// it unboxed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Plain {
    Num(Fraction),
    Bool(bool),
}

impl Plain {
    pub(crate) fn of(value: &Value) -> Option<Self> {
        match &value.data {
            ValueData::Scalar(f) => Some(Plain::Num(f.clone())),
            ValueData::Boolean(b) => Some(Plain::Bool(*b)),
            _ => None,
        }
    }

    pub(crate) fn into_value(self) -> Value {
        match self {
            Plain::Num(f) => Value::from_fraction(f),
            Plain::Bool(b) => Value::from_bool(b),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Compare {
    Lt,
    Gt,
    Eq,
}

/// What one instruction computes. Each is one dispatched Word, and so one
/// execution step, except `FloorDiv`, which is the two Words `DIV FLOOR`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Op {
    Arith(ExactArithmeticSchema),
    /// `DIV FLOOR`: charged as the two Words, the work the `DIV`'s.
    FloorDiv,
    Compare(Compare),
    /// `MIN` (`max: false`) or `MAX`: the operand the order picks, the left
    /// on a tie. A step, and no work.
    Extremum {
        max: bool,
    },
    Floor,
    Round,
    Not,
    And,
    /// `when_true when_false mask SELECT`: operands `a`, `b`, `c`.
    Select,
    /// `POW` on plain operands (`quickened::small_power`): a step and one
    /// unit of work; any power that function declines declines the walk.
    Pow,
    /// Any other Word the contract admits (`fusion_contract`): its plain law,
    /// a step, and the work that law reports. Run by the general tier only.
    Kernel(&'static crate::interpreter::fusion_contract::PlainKernel),
}

/// An operand: a register (the inputs first, then one per instruction) or a
/// constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Src {
    Reg(usize),
    Const(usize),
}

/// `register[inputs + index] = op(a, b, c)`; unused operands are `Const(0)`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Instr {
    pub(crate) op: Op,
    pub(crate) a: Src,
    pub(crate) b: Src,
    pub(crate) c: Src,
}

/// A number or a truth value: what each register holds, the same for every
/// run once the inputs' kinds are known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ty {
    Num,
    Bool,
}

/// Which higher-order Word is walking, and so what it answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FusedWalk {
    /// One result per element, the element alone on the block's stack.
    Map,
    /// The elements whose result is TRUE, the element alone on the stack.
    Filter,
    /// The accumulator and the element on the block's stack; the last
    /// accumulator is the answer.
    Fold,
    /// As `Fold`, answering every accumulator.
    Scan,
}

/// A block lowered to one program in register form.
#[derive(Debug)]
pub(crate) struct FusedBlock {
    pub(crate) instrs: Vec<Instr>,
    pub(crate) consts: Vec<Plain>,
    /// Which constants are one-lane Vector literals (`[ 1 ]`), walked as their
    /// lane: the walk computes on the lane and accounts for each as a
    /// one-lane Tensor (`lane_flow`).
    pub(crate) lane_consts: Vec<bool>,
    /// How many values a run starts from: the element, or the accumulator and
    /// the element.
    pub(crate) inputs: usize,
    pub(crate) out: Src,
    /// Words dispatched per run, User Word calls included.
    pub(crate) steps_per_run: u64,
    /// Whether a name bound outside the block was read into it as a
    /// constant, which ties the lowering to that binding.
    pub(crate) reads_outer: bool,
    /// `lane_flow` for a run that starts from a plain element (`MAP` with
    /// lane literals) and from a lane accumulator beside one (a one-lane
    /// seed): fixed by the program, so worked out once.
    lanes_from_element: Option<bool>,
    lanes_from_seed: Option<bool>,
    /// The column tier's preparations of the program, per input kinds.
    pub(crate) preparations: Preparations,
}

/// What a walk would charge, committed only once it has finished.
pub(crate) struct Charges {
    pub(crate) steps: usize,
    pub(crate) work: u64,
}

impl Op {
    pub(crate) fn arity(self) -> usize {
        match self {
            Op::Floor | Op::Round | Op::Not => 1,
            Op::Select => 3,
            Op::Kernel(kernel) => kernel.arity,
            _ => 2,
        }
    }
}

impl FusedBlock {
    /// Lower `code` for a walk that starts the block on `inputs` values, or
    /// `None` when any op is outside the subset or the block would underflow
    /// or leave other than one value (an ERROR the ordinary route reports).
    pub(crate) fn compile(code: &Code, interp: &Interpreter, inputs: usize) -> Option<Self> {
        let lowering = crate::interpreter::fused_block_lower::lower(code, interp, inputs)?;
        let [out] = lowering.stack[..] else {
            return None;
        };
        let mut block = FusedBlock {
            instrs: lowering.instrs,
            consts: lowering.consts,
            lane_consts: lowering.lane_consts,
            inputs,
            out,
            steps_per_run: lowering.words,
            reads_outer: lowering.reads_outer,
            lanes_from_element: None,
            lanes_from_seed: None,
            preparations: Default::default(),
        };
        block.lanes_from_element = block.lane_flow(&[false]);
        block.lanes_from_seed = block.lane_flow(&[true, false]);
        Some(block)
    }

    fn has_lanes(&self) -> bool {
        self.lane_consts.contains(&true)
    }

    /// Run the walk over `target`, starting from `seed` for `Fold`/`Scan`.
    ///
    /// Answers the value the Word leaves on the stack, having committed the
    /// interpreted walk's charges, or `None` — with nothing touched — when the
    /// ordinary walk must run instead.
    pub(crate) fn run(
        &self,
        interp: &mut Interpreter,
        walk: FusedWalk,
        target: &Value,
        seed: Option<&Value>,
    ) -> Option<Value> {
        if !interp.scalar_fastpath_enabled || !interp.fused_block_enabled {
            return None;
        }
        // A block that pushes one-lane literals is walked on the lanes, for
        // `MAP` alone: each run starts from a plain element, and a lane answer
        // is one per element. A one-lane seed (`[ 0 ]`) is walked as its lane.
        let mut lane_answer = false;
        if self.has_lanes() {
            if walk != FusedWalk::Map || !interp.dense_kernels_enabled {
                return None;
            }
            lane_answer = self.lanes_from_element?;
        }
        let seed = match (walk, seed) {
            (FusedWalk::Map | FusedWalk::Filter, _) => None,
            (_, None) => return None,
            (_, Some(seed)) => match Plain::of(seed) {
                Some(plain) => Some(plain),
                None => {
                    if !interp.dense_kernels_enabled || !self.lanes_from_seed? {
                        return None;
                    }
                    lane_answer = true;
                    Some(one_lane_of(seed)?)
                }
            },
        };
        let (mut value, charges) =
            crate::interpreter::fused_block_columns::run(self, interp, walk, target, seed.as_ref())
                .or_else(|| {
                    crate::interpreter::fused_block_general::run(
                        self,
                        interp,
                        walk,
                        target,
                        seed.as_ref(),
                    )
                })?;
        if lane_answer {
            value = crate::interpreter::fused_block_lane::rewrap_one_lane(walk, &value)?;
        }

        #[cfg(test)]
        FUSED_RUNS.with(|c| c.set(c.get() + 1));
        interp.execution_step_count += charges.steps;
        interp.numeric_work_used += charges.work;
        Some(value)
    }

    /// The kind of every register and of the answer for inputs of kinds
    /// `inputs`, and the work one run charges when every number is a machine
    /// word (each arithmetic Word `binary_numeric_work(1, 1)`), or `None` when
    /// some op meets the wrong kind or is a kernel, which only the general
    /// tier runs.
    pub(crate) fn types(&self, inputs: &[Ty]) -> Option<(Vec<Ty>, u64)> {
        let mut types = inputs.to_vec();
        let mut work = 0u64;
        for ins in &self.instrs {
            let ty = |s: Src| match s {
                Src::Reg(r) => types[r],
                Src::Const(k) => match self.consts[k] {
                    Plain::Num(_) => Ty::Num,
                    Plain::Bool(_) => Ty::Bool,
                },
            };
            let arity = ins.op.arity();
            let a = ty(ins.a);
            let b = if arity > 1 { ty(ins.b) } else { a };
            let c = if arity > 2 { ty(ins.c) } else { a };
            use Ty::{Bool, Num};
            let out = match (ins.op, a, b) {
                (Op::Arith(_) | Op::FloorDiv | Op::Pow, Num, Num) => {
                    work += binary_numeric_work(1, 1);
                    Num
                }
                (Op::Extremum { .. }, Num, Num) => Num,
                (Op::Floor | Op::Round, Num, _) => Num,
                (Op::Not, Bool, _) => Bool,
                (Op::And, Bool, Bool) => Bool,
                (Op::Compare(Compare::Eq), x, y) if x == y => Bool,
                (Op::Compare(_), Num, Num) => Bool,
                (Op::Select, x, y) if x == y && c == Bool => x,
                _ => return None,
            };
            types.push(out);
        }
        Some((types, work))
    }

    /// For a walk that holds one-lane Tensors — a one-lane accumulator, or
    /// one-lane literals the block pushes — whether the run's result is a
    /// lane; `None` when a lane reaches anything but `ADD`/`SUB`/`MUL`/`DIV`
    /// and a binding. `inputs` says which of the values the run starts from
    /// are lanes.
    ///
    /// Those four Words lift over the lane and answer a one-lane Tensor, so
    /// the walk computes on the lane alone; the work is the same, since a
    /// one-lane Tensor and a machine-word scalar both measure one lane of one
    /// limb (`measure_operand`). Every other Word treats a one-lane Tensor
    /// differently from its lane — `EQ` compares whole values, `LT` answers a
    /// Vector of Booleans — so the walk declines it.
    fn lane_flow(&self, inputs: &[bool]) -> Option<bool> {
        if inputs.len() != self.inputs {
            return None;
        }
        let mut lanes = inputs.to_vec();
        let lane = |lanes: &[bool], s: Src| match s {
            Src::Reg(r) => lanes[r],
            Src::Const(k) => self.lane_consts.get(k).copied().unwrap_or(false),
        };
        for ins in &self.instrs {
            let any = (0..ins.op.arity()).any(|i| lane(&lanes, [ins.a, ins.b, ins.c][i]));
            match ins.op {
                Op::Arith(_) => lanes.push(any),
                _ if any => return None,
                _ => lanes.push(false),
            }
        }
        Some(lane(&lanes, self.out))
    }

    /// The steps `runs` runs take, if the step ceiling lets all of them run.
    /// A walk the ceiling would stop part way is the ordinary walk's to stop.
    pub(crate) fn steps_within_ceiling(&self, interp: &Interpreter, runs: u64) -> Option<usize> {
        let steps = usize::try_from(runs.checked_mul(self.steps_per_run)?).ok()?;
        (interp.execution_step_count.checked_add(steps)? <= interp.max_execution_steps)
            .then_some(steps)
    }
}

/// What a walk over inputs of given kinds needs from the block, worked out
/// once per kinds: a block walked again and again — the inner `FOLD` of
/// `[ [ 0 ] [ ADD ] FOLD ] MAP` — is prepared once.
#[derive(Debug)]
pub(crate) struct Prepared {
    pub(crate) work_per_run: u64,
    pub(crate) out_ty: Ty,
    pub(crate) consts: Vec<(i64, i64)>,
    /// Per register, whether it depends on the accumulator (register 0).
    pub(crate) reads_acc: Vec<bool>,
    /// `acc = acc op x` (or `x op acc`), when that is all that reads the
    /// accumulator.
    pub(crate) single: Option<(ExactArithmeticSchema, bool, Src)>,
}

/// The preparations kept on a block, per input kinds; a block meets one or
/// two.
#[derive(Debug, Default)]
pub(crate) struct Preparations(std::sync::Mutex<Vec<KeptPreparation>>);

type KeptPreparation = (u8, Option<std::sync::Arc<Prepared>>);

impl Preparations {
    pub(crate) fn get(
        &self,
        block: &FusedBlock,
        inputs: &[Ty],
    ) -> Option<std::sync::Arc<Prepared>> {
        let mut kept = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // The kinds as one small key: a bit per input, and the count.
        let key = inputs
            .iter()
            .enumerate()
            .fold(inputs.len() as u8, |key, (i, ty)| {
                key | (u8::from(*ty == Ty::Bool) << (4 + i))
            });
        if let Some((_, prepared)) = kept.iter().find(|(kept_key, _)| *kept_key == key) {
            return prepared.clone();
        }
        let prepared = crate::interpreter::fused_block_columns::prepare(block, inputs)
            .map(std::sync::Arc::new);
        if kept.len() < 4 {
            kept.push((key, prepared.clone()));
        }
        prepared
    }
}

/// The lane of `value` when it is a one-lane Tensor, as `[ 0 ]` is; a lane
/// over zero (`[ 1/0 ]`) is a lane like any other.
pub(crate) fn one_lane_of(value: &Value) -> Option<Plain> {
    let ValueData::Tensor { data, shape } = &value.data else {
        return None;
    };
    if shape.as_slice() != [1] || data.len() != 1 {
        return None;
    }
    Some(Plain::Num(data.fraction_at(0)))
}

#[cfg(test)]
thread_local! {
    static FUSED_RUNS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Fused walks committed on this thread, for tests that pin which route ran.
#[cfg(test)]
pub(crate) fn fused_runs_on_this_thread() -> u64 {
    FUSED_RUNS.with(|c| c.get())
}
