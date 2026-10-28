//! The block a higher-order Word runs, checked before anything runs.
//!
//! MAP and FILTER give their block one value and FOLD and SCAN give it two,
//! and every one of them requires the block to leave exactly one
//! (LANG.SOURCE.FRAME, LANG.COLLECTIONS.HIGHER). When the block is the literal
//! written immediately before the Word — the one place LANG.CONTRACT.FIELD
//! already reads code — its inferred arity says whether it can: a block that
//! takes `c` values and leaves `p` leaves `k - c + p` of the `k` it is given,
//! and reaches below its frame when `c > k`. A block whose arity the inference
//! cannot fix (a `variable` arity: a binding from the enclosing frame, a Word
//! of variable arity) is not judged, so nothing correct is ever reported.
//!
//! `check` reports a violation in the category running the block raises once
//! its own Words have succeeded — `blockContractViolation`, or `stackUnderflow`
//! for a block that reads below its frame — as it reports a name that would
//! fail when run. `compute`
//! leaves it to the run: a block over an empty collection never runs, and a
//! program that never runs it is not wrong.

use super::contract::{build_definitions_interpreter, collect_top_level_defs};
use crate::interpreter::word_contract::{code_positions, ContractFlow};
use crate::interpreter::Interpreter;
use crate::tokenizer::SourceSpan;
use crate::types::Token;
use crate::word_name::canonical_word_name;

/// A higher-order Word whose literal block cannot leave exactly one value.
pub(crate) struct BlockArityViolation {
    pub word: &'static str,
    /// What running the block raises: `stackUnderflow` when it reads below
    /// its frame, `blockContractViolation` when it leaves other than one.
    pub category: &'static str,
    pub message: String,
    pub position: Option<SourceSpan>,
}

/// How many values the Word's block is given, for a Word that runs one under
/// the isolated-frame rule.
fn frame_size(name: &str) -> Option<(&'static str, u16)> {
    match name {
        "MAP" => Some(("MAP", 1)),
        "FILTER" => Some(("FILTER", 1)),
        "FOLD" => Some(("FOLD", 2)),
        "SCAN" => Some(("SCAN", 2)),
        _ => None,
    }
}

/// The first literal block in `source`, at top level or in a top-level
/// definition's body, that the Word after it cannot run without a
/// `blockContractViolation`.
pub(crate) fn first_block_arity_violation(source: &str) -> Option<BlockArityViolation> {
    let (tokens, spans) = crate::tokenizer::tokenize_with_spans(source).ok()?;
    let (interp, _, _) = build_definitions_interpreter(source);
    let mut contexts = vec![(0, tokens.as_slice())];
    for (_, body, def_at) in collect_top_level_defs(&tokens) {
        // `def_at` is the DEF; before it, the name and the body's `]`.
        let start = def_at - 2 - body.len();
        contexts.push((start, &tokens[start..start + body.len()]));
    }
    contexts.into_iter().find_map(|(offset, context)| {
        let (index, word, category, message) = violation_in(&interp, context)?;
        Some(BlockArityViolation {
            word,
            category,
            message,
            position: spans.get(offset + index).copied(),
        })
    })
}

/// The first violation among the Words `context` runs, with the index of
/// the Word that would raise it.
fn violation_in(
    interp: &Interpreter,
    context: &[Token],
) -> Option<(usize, &'static str, &'static str, String)> {
    let code = code_positions(context);
    let mut depth = 0usize;
    for (idx, token) in context.iter().enumerate() {
        match token {
            Token::VectorStart => depth += 1,
            Token::VectorEnd => depth = depth.saturating_sub(1),
            Token::Symbol(symbol) if depth == 0 || code[idx] => {
                let Some((word, given)) = frame_size(&canonical_word_name(symbol)) else {
                    continue;
                };
                let Some(block) = literal_block_before(context, idx) else {
                    continue;
                };
                let ContractFlow::Fixed { consumes, produces } =
                    interp.infer_contract_for_block(block).flow
                else {
                    continue;
                };
                if consumes > given {
                    return Some((
                        idx,
                        word,
                        "stackUnderflow",
                        format!(
                            "{word}: the block written before it reads {consumes} values, and \
                             {word} gives it {given}"
                        ),
                    ));
                }
                let left = given - consumes + produces;
                if left != 1 {
                    return Some((
                        idx,
                        word,
                        "blockContractViolation",
                        format!(
                            "{word}: the block written before it leaves {left} values from the \
                             {given} {word} gives it, and a {word} block must leave exactly one"
                        ),
                    ));
                }
            }
            _ => {}
        }
    }
    None
}

/// The tokens inside the `[ ... ]` that closes immediately before `idx`.
fn literal_block_before(context: &[Token], idx: usize) -> Option<&[Token]> {
    let close = idx.checked_sub(1)?;
    if context[close] != Token::VectorEnd {
        return None;
    }
    let mut depth = 0usize;
    for open in (0..close).rev() {
        match context[open] {
            Token::VectorEnd => depth += 1,
            Token::VectorStart if depth == 0 => return Some(&context[open + 1..close]),
            Token::VectorStart => depth -= 1,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "block_arity_tests.rs"]
mod block_arity_tests;
