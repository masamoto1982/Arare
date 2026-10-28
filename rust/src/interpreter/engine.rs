//! The one execution engine: every token stream — a program, a Word's body, a
//! block a higher-order Word runs — is compiled once to [`Code`] and run by
//! [`Interpreter::run_code`]. There is no second route that runs tokens
//! directly, so there is nothing for a compiled route to stay in step with
//! (LANG.AUTHORITY.FREEDOM).
//!
//! Compiling decides only what a token's text decides. A literal becomes the
//! value it denotes — a `[ ... ]` literal is always a constant, since nothing
//! inside one is evaluated (LANG.VALUES.VECTOR) — and a Core Word, which is
//! sealed, is resolved to its registry entry. Every other name is resolved
//! when it runs (a binding, then a User Word), so `Code` never depends on the
//! dictionary and a Word's compiled body never goes stale.

use std::sync::Arc;

use crate::error::{AjisaiError, Result};
use crate::kernel::generated::{core_word_spelled, generated_word, GeneratedWord, WordId};
use crate::types::{Name, Token, Value, ValueData};

use super::quickened::{apply, Kind, Slot};
use super::Interpreter;

#[derive(Debug, Clone)]
pub(crate) enum Op {
    /// A number, a string, or a value carried in whole by the Vector-as-code
    /// bridge (`value_as_code`).
    Push(Value),
    /// A `[ ... ]` literal. When `'NAME' DEF` follows it, `def_body` is the
    /// body as written, which `DEF` keeps as the Word's source.
    Bracket {
        value: Value,
        def_body: Option<Arc<[Token]>>,
        /// Whether the literal names a Word anywhere inside it, so that a
        /// block run from it could run something (`block_is_written`).
        names_words: bool,
    },
    /// A Core Word, resolved at compile time: Core is sealed.
    Word(&'static GeneratedWord),
    /// A Core Word with a scalar law the engine answers in place when its
    /// operands are plain machine-word scalars (`quickened::apply`).
    Scalar(Kind, &'static GeneratedWord),
    /// `TRUE`, `FALSE` or `NIL`: a Word that pushes a constant.
    Constant(Value, &'static GeneratedWord),
    /// `'NAME' BIND` for a name that may be bound: one Word, the `BIND`,
    /// into the frame's slot for the name. Whatever the fast bind cannot
    /// settle runs as the two tokens would.
    Bind { slot: u32, name: Value, key: Name },
    /// A name the code binds itself: its slot once bound, and before that the
    /// name as any other name is looked up.
    Load { slot: u32, key: Name },
    /// Any other name, canonical: a binding or a User Word, looked up when
    /// it runs.
    Name(Name),
    /// A literal that denotes nothing — a malformed number, a literal past the
    /// nesting ceiling — refused when it is reached, as it always was.
    Raise(AjisaiError),
}

#[derive(Debug, Default)]
pub(crate) struct Code {
    pub(crate) ops: Vec<Op>,
    /// The names the code binds with `'NAME' BIND`, in slot order.
    pub(crate) slots: Arc<[Name]>,
}

fn canonical(text: &str) -> Name {
    match crate::word_name::canonical_word_name(text) {
        std::borrow::Cow::Borrowed(text) => Name::from(text),
        std::borrow::Cow::Owned(folded) => Name::from(folded),
    }
}

/// Whether the `[ ... ]` literal spanning `tokens[start..start + len]` is the
/// body a `'NAME' DEF` right after it defines, and if so its tokens as written.
fn def_body(tokens: &[Token], start: usize, len: usize) -> Option<Arc<[Token]>> {
    let after = start + len;
    let is_def = matches!(tokens.get(after), Some(Token::String(_)))
        && is_word(tokens.get(after + 1), WordId::Def);
    let body = &tokens[start + 1..after - 1];
    // A body holding a carried value is not the body as written: `DEF` writes
    // such a value back as what builds it, or refuses one nothing builds.
    (is_def && !body.iter().any(|t| matches!(t, Token::Value(_)))).then(|| Arc::from(body))
}

/// Compile one token stream.
pub(crate) fn compile(tokens: &[Token], interp: &Interpreter) -> Code {
    let mut slots = Vec::new();
    let mut ops = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        ops.push(next_op(tokens, &mut i, &interp.runtime_limits, &mut slots));
    }
    Code {
        ops,
        slots: slots.into(),
    }
}

/// The op the tokens at `*i` compile to, moving `*i` past them.
///
/// `slots` are the names the code has bound with `'NAME' BIND` so far, each
/// given a slot of the frame the code runs in when its first `BIND` is
/// compiled. Code runs straight through, so a name written before its first
/// `BIND` is not the code's own yet and is looked up as any other name.
fn next_op(
    tokens: &[Token],
    i: &mut usize,
    limits: &super::RuntimeLimits,
    slots: &mut Vec<Name>,
) -> Op {
    let slot_of = |slots: &[Name], key: &Name| {
        slots
            .iter()
            .position(|held| held == key)
            .map(|at| at as u32)
    };
    let start = *i;
    *i += 1;
    match &tokens[start] {
        Token::Number(literal) => match literal.parsed() {
            Ok(fraction) => Op::Push(Value::from_fraction(fraction)),
            Err(message) => Op::Raise(AjisaiError::MalformedSource(message)),
        },
        Token::String(text) if is_word(tokens.get(*i), WordId::Bind) && bindable(text) => {
            *i += 1;
            let key = canonical(text);
            let slot = slot_of(slots, &key).unwrap_or_else(|| {
                slots.push(key.clone());
                slots.len() as u32 - 1
            });
            Op::Bind {
                slot,
                name: Value::from_text(text),
                key,
            }
        }
        Token::String(s) => Op::Push(Value::from_text(s)),
        Token::Value(value) => Op::Bracket {
            names_words: names_a_word(value),
            value: (**value).clone(),
            def_body: None,
        },
        Token::VectorStart => {
            match Interpreter::collect_bracketed_with_depth(tokens, start, 1, limits) {
                Ok((values, len)) => {
                    *i = start + len;
                    let value = Value::from_vector_promoted(values);
                    Op::Bracket {
                        names_words: names_a_word(&value),
                        value,
                        def_body: def_body(tokens, start, len),
                    }
                }
                Err(error) => {
                    // Everything after an unreadable literal is unreachable.
                    *i = tokens.len();
                    Op::Raise(error)
                }
            }
        }
        Token::VectorEnd => Op::Raise(AjisaiError::MalformedSource("Unexpected vector end".into())),
        Token::Symbol(name) => match core_word_spelled(name.as_bytes()) {
            Some(word) => word_op(word),
            None => {
                let key = if name
                    .as_bytes()
                    .iter()
                    .all(|b| b.is_ascii() && !b.is_ascii_lowercase())
                {
                    name.clone()
                } else {
                    canonical(name)
                };
                match slot_of(slots, &key) {
                    Some(slot) => Op::Load { slot, key },
                    None => Op::Name(key),
                }
            }
        },
    }
}

fn word_op(word: &'static GeneratedWord) -> Op {
    match word.id {
        WordId::True => Op::Constant(Value::from_bool(true), word),
        WordId::False => Op::Constant(Value::from_bool(false), word),
        WordId::Nil => Op::Constant(Value::nil(), word),
        id => match Kind::of(id) {
            Some(kind) => Op::Scalar(kind, word),
            None => Op::Word(word),
        },
    }
}

fn is_word(token: Option<&Token>, id: WordId) -> bool {
    matches!(token, Some(Token::Symbol(s)) if core_word_spelled(s.as_bytes()).is_some_and(|w| w.id == id))
}

/// Whether `'NAME' BIND` can be compiled to a fast bind: a name no Core
/// Word holds. Whether a User Word holds it is asked when it runs.
fn bindable(text: &str) -> bool {
    crate::tokenizer::is_symbol_token_lexeme(text) && core_word_spelled(text.as_bytes()).is_none()
}

impl Interpreter {
    /// A scalar Word answered in place, when its operands are plain
    /// machine-word scalars and every ceiling it could meet is clear; it then
    /// charges what the dispatch would, one step and the work.
    #[inline(always)]
    fn scalar_in_place(&mut self, kind: Kind) -> bool {
        if !self.quickening_enabled || !self.scalar_fastpath_enabled {
            return false;
        }
        let arity = kind.arity();
        let values = self.stack.as_slice();
        let Some(base) = values.len().checked_sub(arity) else {
            return false;
        };
        let mut operands = [Slot::Bool(false); 3];
        for (operand, value) in operands.iter_mut().zip(&values[base..]) {
            match Slot::of(value) {
                Some(slot) => *operand = slot,
                None => return false,
            }
        }
        let Some(answer) = apply(kind, &operands[..arity]) else {
            return false;
        };
        if self.runtime_limits.max_bigint_bits < 64
            || self.execution_step_count >= self.max_execution_steps
            || self.numeric_work_used + answer.work > self.runtime_limits.max_numeric_work
        {
            return false;
        }
        self.execution_step_count += 1;
        self.numeric_work_used += answer.work;
        // The operands are plain scalars and truth values, which own nothing,
        // so they are overwritten rather than dropped.
        for _ in 1..arity {
            std::mem::forget(self.stack.pop());
        }
        std::mem::forget(std::mem::replace(
            &mut self.stack[base],
            answer.value.into_value(),
        ));
        #[cfg(test)]
        super::quickened::note_quickened();
        true
    }

    /// `'NAME' BIND` of one name, when nothing about it needs the full
    /// `BIND`: a value to bind that names nothing (so no cycle can form) and
    /// a name no User Word holds.
    #[inline]
    fn bind_in_place(&mut self, slot: u32, key: &Name) -> bool {
        let Some(subject) = self.stack.last() else {
            return false;
        };
        if matches!(
            subject.data,
            ValueData::Symbol(_) | ValueData::Vector(_) | ValueData::Record(_)
        ) || self.execution_step_count >= self.max_execution_steps
            || self.is_user_word(key)
        {
            return false;
        }
        self.execution_step_count += 1;
        let subject = self.stack.pop().expect("checked above");
        self.set_local(slot, subject);
        true
    }

    /// Run `code` in the current frame.
    pub(crate) fn run_code(&mut self, code: &Code) -> Result<()> {
        let base = self.open_code_frame();
        let result = code.ops.iter().try_for_each(|op| self.run_op(op));
        self.close_code_frame(base);
        result
    }

    /// Begin recording the block literals of a code that starts running:
    /// they are what a block it runs may be written as (`block_is_written`).
    fn open_code_frame(&mut self) -> usize {
        let base = self.code_literals.len();
        self.code_literal_bases.push(base);
        base
    }

    fn close_code_frame(&mut self, base: usize) {
        self.code_literals.truncate(base);
        self.code_literal_bases.pop();
    }

    /// Whether the block on top of the stack is one the running code wrote:
    /// equal to a block literal it has pushed, or to a block inside one. Such
    /// a block is part of the code that runs it, so running it runs less code
    /// (LANG.DICTIONARY.ACYCLIC). A block that names nothing runs nothing
    /// and counts as written. Anything else — a block taken apart and put
    /// back together, or one handed in from another frame — was built from
    /// data. A top that is not a block is the Word's own refusal to make.
    pub(crate) fn block_is_written(&self) -> bool {
        let Some(top) = self.stack.last() else {
            return true;
        };
        if !names_a_word(top) {
            return true;
        }
        let Some(&base) = self.code_literal_bases.last() else {
            return false;
        };
        self.code_literals[base..]
            .iter()
            .any(|literal| holds_block(literal, top))
    }

    /// Run a program's own tokens, compiling each op as it is reached and
    /// keeping none: a program runs once, so storing its code would only be
    /// built and dropped again. A failure records which top-level token it
    /// was reached from: a block and a Word body have no source of their
    /// own, so that token is the one a reader is sent to.
    ///
    /// A program writes a handful of names and numbers many times over, and
    /// the tokenizer knows which tokens spell the same lexeme (`firsts`), so
    /// each lexeme is compiled once and its op reused.
    pub(crate) fn run_program(
        &mut self,
        tokens: &[Token],
        firsts: &[u32],
        lexemes: usize,
    ) -> Result<()> {
        let base = self.open_code_frame();
        let result = self.run_program_ops(tokens, firsts, lexemes);
        self.close_code_frame(base);
        result
    }

    fn run_program_ops(&mut self, tokens: &[Token], firsts: &[u32], lexemes: usize) -> Result<()> {
        let mut slots = Vec::new();
        self.set_frame_slots(Arc::default());
        let mut memo: Vec<Option<Op>> = Vec::new();
        memo.resize_with(lexemes, || None);
        let mut i = 0;
        while i < tokens.len() {
            let start = i;
            let first = firsts.get(i).copied().unwrap_or(u32::MAX);
            let outcome = if first == u32::MAX {
                let bound = slots.len();
                let op = next_op(tokens, &mut i, &self.runtime_limits, &mut slots);
                if slots.len() != bound {
                    self.grow_frame_slots(Arc::from(slots.as_slice()));
                }
                self.run_op(&op)
            } else {
                i += 1;
                let op = memo[first as usize].get_or_insert_with(|| {
                    let mut at = start;
                    next_op(tokens, &mut at, &self.runtime_limits, &mut slots)
                });
                self.run_op(op)
            };
            if let Err(error) = outcome {
                self.current_source_span = self.source_spans.get(start).copied();
                return Err(error);
            }
        }
        Ok(())
    }

    /// One op: the ops a run spends its time in, inline in the loop that
    /// runs them, and every other one out of line.
    #[inline(always)]
    fn run_op(&mut self, op: &Op) -> Result<()> {
        match op {
            Op::Push(value) => {
                self.stack.push(value.clone());
                return Ok(());
            }
            Op::Scalar(kind, _) => {
                if self.scalar_in_place(*kind) {
                    return Ok(());
                }
            }
            Op::Load { slot, .. } => {
                if let Some(value) = self.local(*slot) {
                    let value = value.clone();
                    self.stack.push(value);
                    return Ok(());
                }
            }
            _ => {}
        }
        self.run_op_slow(op)
    }

    #[inline(never)]
    fn run_op_slow(&mut self, op: &Op) -> Result<()> {
        match op {
            Op::Push(value) => {
                self.stack.push(value.clone());
                Ok(())
            }
            // `run_op` has tried the in-place route and it declined.
            Op::Scalar(_, word) | Op::Word(word) => self.dispatch_core_word(word),
            Op::Constant(value, word) => {
                if self.execution_step_count < self.max_execution_steps {
                    self.execution_step_count += 1;
                    self.stack.push(value.clone());
                    Ok(())
                } else {
                    self.dispatch_core_word(word)
                }
            }
            Op::Bind { slot, name, key } => {
                if self.bind_in_place(*slot, key) {
                    Ok(())
                } else {
                    self.stack.push(name.clone());
                    self.dispatch_core_word(generated_word("BIND").expect("BIND is a Core Word"))
                }
            }
            Op::Load { slot, key } => match self.local(*slot) {
                Some(value) => {
                    let value = value.clone();
                    self.stack.push(value);
                    Ok(())
                }
                None => self.dispatch_word_beyond_core(key),
            },
            Op::Name(name) => self.dispatch_word_beyond_core(name),
            Op::Bracket {
                value,
                def_body,
                names_words,
            } => {
                self.pending_def_body_tokens = def_body.as_deref().map(<[Token]>::to_vec);
                if *names_words {
                    self.code_literals.push(value.clone());
                }
                self.stack.push(value.clone());
                Ok(())
            }
            Op::Raise(error) => Err(error.clone()),
        }
    }

    /// Compile and run a block in a transparent frame: it reads the names of
    /// the frame it was written in, and the names it makes end with it.
    pub(crate) fn execute_nested_block(&mut self, tokens: &[Token]) -> Result<()> {
        let code = compile(tokens, self);
        self.run_block(&code)
    }

    /// [`Self::execute_nested_block`] for code already compiled.
    pub(crate) fn run_block(&mut self, code: &Code) -> Result<()> {
        self.open_frame(false, &code.slots);
        let result = self.run_code(code);
        self.close_binding_scope();
        result
    }
}

/// Whether `value` names a Word anywhere inside it: a block that names none
/// pushes its elements and runs nothing.
fn names_a_word(value: &Value) -> bool {
    match &value.data {
        ValueData::Symbol(_) => true,
        ValueData::Vector(children) => children.iter().any(names_a_word),
        _ => false,
    }
}

/// Whether `block` is `literal` or a block somewhere inside it.
fn holds_block(literal: &Value, block: &Value) -> bool {
    match (&literal.data, &block.data) {
        (ValueData::Vector(a), ValueData::Vector(b)) if Arc::ptr_eq(a, b) => return true,
        _ => {}
    }
    if literal == block {
        return true;
    }
    match &literal.data {
        ValueData::Vector(children) => children
            .iter()
            .any(|child| child.is_vector() && holds_block(child, block)),
        _ => false,
    }
}
