//! Inferred contracts for User Words and blocks (LANG.CONTRACT.*): the stack
//! arity and the `field` axis, read off the body without running any of it.
//!
//! A Core Word's contract is registered (`spec/words.json`); a User Word's is
//! the join of what its body writes. Where the walk cannot know — a Word of
//! variable arity, a name nothing defines, code it cannot read — it answers
//! the weaker claim: a `variable` arity, or `leaving` with no exit to name.

use std::collections::HashMap;
use std::sync::Arc;

use super::word_contract_flow::{note_bound_names, BoundNames, FlowSim};
use super::Interpreter;
use crate::coreword_registry::{get_coreword_metadata, FieldClosure, MassContract};
use crate::types::{Token, Value, ValueData, WordDefinition};
use crate::word_name::canonical_word_name;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractFlow {
    Fixed { consumes: u16, produces: u16 },
    Dynamic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordContract {
    pub flow: ContractFlow,
    /// Whether the Word can answer a point over zero from operands that hold
    /// none (LANG.CONTRACT.FIELD).
    pub field: FieldClosure,
    /// What makes it `leaving`, in body order: the calls that can leave the
    /// field there and the literals written over zero.
    pub field_exits: Vec<String>,
    /// `leaving` rests only on code the walk could not read, so it cannot
    /// refute a declaration of `closed`.
    pub field_unread: bool,
}

impl WordContract {
    fn core(name: &str) -> Self {
        match get_coreword_metadata(name) {
            Some(meta) => WordContract {
                flow: match meta.mass {
                    MassContract::Fixed { consumes, produces } => ContractFlow::Fixed {
                        consumes: consumes.into(),
                        produces: produces.into(),
                    },
                    MassContract::Dynamic => ContractFlow::Dynamic,
                },
                field: meta.field,
                field_exits: Vec::new(),
                field_unread: false,
            },
            None => WordContract::unread(),
        }
    }

    /// What nothing could be read about.
    fn unread() -> Self {
        WordContract {
            flow: ContractFlow::Dynamic,
            field: FieldClosure::Leaving,
            field_exits: Vec::new(),
            field_unread: true,
        }
    }
}

/// The contracts already inferred during one inference, so a dependency
/// called from many places is walked once.
type Memo = HashMap<Arc<str>, Arc<WordContract>>;

/// The Words that run the `[ ... ]` written immediately before them as code.
fn runs_code(name: &str) -> bool {
    matches!(name, "MAP" | "FILTER" | "FOLD" | "SCAN" | "EXEC")
}

/// For every token, whether it sits inside a `[ ... ]` some Word runs as
/// code: a group whose close a code-running Word follows, inside no group
/// that is data. Every other group is data and none of its Words run.
pub(crate) fn code_positions(tokens: &[Token]) -> Vec<bool> {
    let mut close_of = vec![None; tokens.len()];
    let mut opens = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::VectorStart => opens.push(i),
            Token::VectorEnd => {
                if let Some(open) = opens.pop() {
                    close_of[open] = Some(i);
                }
            }
            _ => {}
        }
    }
    let mut code = vec![false; tokens.len()];
    let mut levels: Vec<bool> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        let enclosing = levels.last().copied();
        match token {
            Token::VectorStart => {
                let runs = enclosing != Some(false)
                    && close_of[i]
                        .and_then(|close| tokens.get(close + 1))
                        .is_some_and(|next| {
                            matches!(next, Token::Symbol(s) if runs_code(&canonical_word_name(s)))
                        });
                code[i] = enclosing.unwrap_or(false);
                levels.push(runs);
            }
            Token::VectorEnd => code[i] = levels.pop().unwrap_or(false),
            _ => code[i] = enclosing.unwrap_or(false),
        }
    }
    code
}

/// Whether a literal token pushes one of the two points over zero, at any
/// depth. A lexeme that does not parse is not counted: running it raises
/// before anything is pushed.
fn pushes_point_over_zero(token: &Token) -> bool {
    fn holds(value: &Value) -> bool {
        match &value.data {
            ValueData::Scalar(f) => !f.is_finite(),
            ValueData::Vector(items) => items.iter().any(holds),
            ValueData::Tensor { data, .. } => !data.all_finite(),
            ValueData::Record(record) => record.keys().iter().chain(record.values()).any(holds),
            _ => false,
        }
    }
    match token {
        Token::Number(literal) => literal.value().is_some_and(|f| !f.is_finite()),
        Token::Value(value) => holds(value),
        _ => false,
    }
}

/// Whether a call of the `leaving` Word `name` stays in the field because the
/// literal written right before it is the operand that would have taken it
/// out: a `DIV` by a finite non-zero literal, a `POW` to a finite
/// non-negative one. Nothing can come between that literal and the call.
fn literal_keeps_field(name: &str, prev: Option<&Token>) -> bool {
    let Some(Token::Number(literal)) = prev else {
        return false;
    };
    let Some(operand) = literal.value().filter(|f| f.is_finite()) else {
        return false;
    };
    match name {
        "DIV" => !operand.is_zero(),
        "POW" => operand.signum() != std::cmp::Ordering::Less,
        _ => false,
    }
}

fn note_exit(exits: &mut Vec<String>, exit: &str) {
    if !exits.iter().any(|e| e == exit) {
        exits.push(exit.to_string());
    }
}

impl Interpreter {
    /// The contract of the Word `name` resolves to, or `None` when it names
    /// none.
    pub fn infer_word_contract(&self, name: &str) -> Option<Arc<WordContract>> {
        let (resolved, def) = self.resolve_word_entry(name)?;
        Some(self.contract_of(&resolved, &def, &mut Memo::new()))
    }

    /// The contract of a block, as `CONTRACT` reads it: never run.
    pub(crate) fn infer_contract_for_block(&self, tokens: &[Token]) -> WordContract {
        self.infer_body(tokens, &mut Memo::new())
    }

    fn contract_of(
        &self,
        name: &Arc<str>,
        def: &WordDefinition,
        memo: &mut Memo,
    ) -> Arc<WordContract> {
        if def.is_builtin {
            return Arc::new(WordContract::core(name));
        }
        if let Some(known) = memo.get(name) {
            return known.clone();
        }
        // `DEF` refuses a cycle (LANG.DICTIONARY.ACYCLIC); the placeholder
        // keeps the walk finite regardless.
        memo.insert(name.clone(), Arc::new(WordContract::unread()));
        let contract = Arc::new(self.infer_body(&def.body, memo));
        memo.insert(name.clone(), contract.clone());
        contract
    }

    fn infer_body(&self, body: &[Token], memo: &mut Memo) -> WordContract {
        let mut flow = FlowSim::new();
        let mut bound = BoundNames::new();
        let mut exits = Vec::new();
        let mut unread = false;
        let code = code_positions(body);
        for (idx, token) in body.iter().enumerate() {
            let prev = idx.checked_sub(1).map(|i| &body[i]);
            let Token::Symbol(symbol) = token else {
                if pushes_point_over_zero(token) {
                    let spelled = match token {
                        Token::Number(literal) => literal.lexeme(),
                        _ => "a literal over zero",
                    };
                    note_exit(&mut exits, spelled);
                }
                match token {
                    Token::VectorStart | Token::VectorEnd => flow.feed_structural(token),
                    _ => flow.feed_literal(),
                }
                continue;
            };
            let canonical = canonical_word_name(symbol);
            let top_level = !flow.in_literal();
            if top_level {
                // A name a `BIND` made: one value, no call.
                if bound.contains(canonical.as_ref()) {
                    flow.feed_literal();
                    continue;
                }
                note_bound_names(&mut bound, &canonical, body, idx);
            } else if !code[idx] {
                continue;
            }
            // Code that is not the literal written right before the Word that
            // runs it was never read here.
            if runs_code(&canonical) && !(prev == Some(&Token::VectorEnd) && code[idx - 1]) {
                unread = true;
            }
            let Some((name, def)) = self.resolve_word_entry(&canonical) else {
                if top_level {
                    flow.go_dynamic();
                }
                continue;
            };
            let callee = self.contract_of(&name, &def, memo);
            if top_level {
                flow.feed_word(&callee.flow);
            }
            if callee.field == FieldClosure::Leaving && !literal_keeps_field(&name, prev) {
                if callee.field_unread {
                    unread = true;
                } else {
                    note_exit(&mut exits, &name);
                }
            }
        }
        let field = if exits.is_empty() && !unread {
            FieldClosure::Closed
        } else {
            FieldClosure::Leaving
        };
        WordContract {
            flow: flow.finish(),
            field,
            field_unread: unread && exits.is_empty(),
            field_exits: exits,
        }
    }
}
