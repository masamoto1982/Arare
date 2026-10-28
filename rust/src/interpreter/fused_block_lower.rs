//! Lowering a block's compiled code (`engine`) to the register form of a
//! column walk (`fused_block`), User Word calls inlined.

use std::collections::HashMap;

use crate::interpreter::arithmetic::ExactArithmeticSchema;
use crate::interpreter::engine::{Code, Op as CodeOp};
use crate::interpreter::fused_block::{one_lane_of, Compare, Instr, Op, Plain, Src};
use crate::interpreter::interpreter_core::MAX_CALL_DEPTH;
use crate::interpreter::Interpreter;
use crate::kernel::generated::WordId;
use crate::types::fraction::Fraction;
use crate::types::Value;

/// The instruction a Core Word compiles to, when it has one.
pub(crate) fn word_op(id: WordId) -> Option<Op> {
    Some(match id {
        WordId::Add => Op::Arith(ExactArithmeticSchema::Add),
        WordId::Sub => Op::Arith(ExactArithmeticSchema::Sub),
        WordId::Mul => Op::Arith(ExactArithmeticSchema::Mul),
        WordId::Div => Op::Arith(ExactArithmeticSchema::Div),
        WordId::Lt => Op::Compare(Compare::Lt),
        WordId::Gt => Op::Compare(Compare::Gt),
        WordId::Eq => Op::Compare(Compare::Eq),
        WordId::Min => Op::Extremum { max: false },
        WordId::Max => Op::Extremum { max: true },
        WordId::Floor => Op::Floor,
        WordId::Round => Op::Round,
        WordId::Not => Op::Not,
        WordId::And => Op::And,
        WordId::Select => Op::Select,
        WordId::Pow => Op::Pow,
        _ => Op::Kernel(crate::interpreter::fusion_contract::kernel(id)?),
    })
}

/// The one name a `BIND` of `value` would make, when it makes exactly one
/// name it may make.
fn bindable_name(interp: &Interpreter, value: &Value) -> Option<String> {
    let names = crate::interpreter::bindings::binding_names(value).ok()?;
    let [name] = names.as_slice() else {
        return None;
    };
    interp.check_bindable_name(name).ok()?;
    Some(name.to_uppercase())
}

/// Lowering a block's code to register form, User Word calls inlined.
///
/// A User Word called from the block is lowered in place, in a frame of its
/// own, and the call costs the one step the engine charges for it. A Word
/// body is a barrier frame: it reads only the names it binds itself, so a
/// name it did not bind is the ordinary walk's to report, and the call depth
/// is bounded exactly as `call_user_word` bounds it.
pub(crate) struct Lowering<'a> {
    interp: &'a Interpreter,
    pub(crate) stack: Vec<Src>,
    pub(crate) instrs: Vec<Instr>,
    pub(crate) consts: Vec<Plain>,
    pub(crate) lane_consts: Vec<bool>,
    inputs: usize,
    pub(crate) words: u64,
    pub(crate) reads_outer: bool,
}

impl Lowering<'_> {
    fn constant(&mut self, value: Plain, lane: bool) {
        self.consts.push(value);
        self.lane_consts.push(lane);
        self.stack.push(Src::Const(self.consts.len() - 1));
    }

    fn emit(&mut self, op: Op) -> Option<()> {
        if op.arity() > 3 {
            return None;
        }
        let at = self.stack.len().checked_sub(op.arity())?;
        let mut operands = [Src::Const(0); 3];
        operands[..op.arity()].copy_from_slice(&self.stack[at..]);
        self.stack.truncate(at);
        let [a, b, c] = operands;
        self.instrs.push(Instr { op, a, b, c });
        self.stack
            .push(Src::Reg(self.inputs + self.instrs.len() - 1));
        Some(())
    }

    fn line(
        &mut self,
        source: &[CodeOp],
        frame: &mut HashMap<String, Src>,
        sees_outer: bool,
        call_depth: usize,
    ) -> Option<()> {
        let mut i = 0;
        while i < source.len() {
            let next_is = |id: WordId| matches!(source.get(i + 1), Some(CodeOp::Word(w) | CodeOp::Scalar(_, w)) if w.id == id);
            match &source[i] {
                CodeOp::Push(value) => match Plain::of(value) {
                    Some(plain) => self.constant(plain, false),
                    None if next_is(WordId::Bind) => {
                        let name = bindable_name(self.interp, value)?;
                        frame.insert(name, self.stack.pop()?);
                        self.words += 1;
                        i += 1;
                    }
                    None => return None,
                },
                CodeOp::Bracket { value, .. } if next_is(WordId::Length) => {
                    if !value.is_vector() {
                        return None;
                    }
                    self.constant(Plain::Num(Fraction::from(value.len() as i64)), false);
                    self.words += 1;
                    i += 1;
                }
                CodeOp::Bracket { value, .. } => self.constant(one_lane_of(value)?, true),
                CodeOp::Constant(value, _) => match value.as_truth() {
                    Some(b) => {
                        self.constant(Plain::Bool(b), false);
                        self.words += 1;
                    }
                    _ => return None,
                },
                CodeOp::Bind { name, .. } => {
                    let name = bindable_name(self.interp, name)?;
                    frame.insert(name, self.stack.pop()?);
                    self.words += 1;
                }
                CodeOp::Word(word) | CodeOp::Scalar(_, word) => {
                    let mut op = word_op(word.id)?;
                    self.words += 1;
                    if matches!(op, Op::Arith(ExactArithmeticSchema::Div)) && next_is(WordId::Floor)
                    {
                        op = Op::FloorDiv;
                        self.words += 1;
                        i += 1;
                    }
                    self.emit(op)?;
                }
                CodeOp::Name(name) | CodeOp::Load { key: name, .. } => {
                    let name = crate::word_name::canonical_word_name(name);
                    if let Some(src) = frame.get(name.as_ref()) {
                        self.stack.push(*src);
                    } else {
                        match self.interp.lookup_binding(name.as_ref()) {
                            Some(value) if sees_outer => {
                                self.reads_outer = true;
                                self.constant(Plain::of(&value)?, false);
                            }
                            Some(_) => return None,
                            None => self.call(name.as_ref(), call_depth)?,
                        }
                    }
                }
                CodeOp::Raise(_) => return None,
            }
            i += 1;
        }
        Some(())
    }

    fn call(&mut self, name: &str, call_depth: usize) -> Option<()> {
        if call_depth + 1 > MAX_CALL_DEPTH {
            return None;
        }
        let def = self.interp.user_words.get(name)?.clone();
        if def.body.is_empty() {
            return None;
        }
        let code = self.interp.body_code(&def);
        self.words += 1;
        self.line(&code.ops, &mut HashMap::new(), false, call_depth + 1)
    }
}

/// The block's code lowered for a walk that starts it on `inputs` values.
pub(crate) fn lower<'a>(
    code: &Code,
    interp: &'a Interpreter,
    inputs: usize,
) -> Option<Lowering<'a>> {
    let mut lowering = Lowering {
        interp,
        stack: (0..inputs).map(Src::Reg).collect(),
        instrs: Vec::with_capacity(code.ops.len()),
        consts: Vec::new(),
        lane_consts: Vec::new(),
        inputs,
        words: 0,
        reads_outer: false,
    };
    lowering.line(&code.ops, &mut HashMap::new(), true, interp.call_depth)?;
    Some(lowering)
}
