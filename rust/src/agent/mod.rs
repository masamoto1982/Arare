//! Host-neutral agent boundary: pure computation over Ajisai source with no
//! filesystem, terminal or process I/O, so it compiles for every host target
//! that enables the `std` feature (native and wasm32 alike). The native CLI
//! (`crate::cli`) and the WASM one-shot entry point
//! (`crate::wasm_interpreter_bindings`) both render through this module so
//! they observe the identical stack, NIL reasons, error, output and
//! resource-usage envelope (`docs/dev/agent-cli-output-contract.md`).

pub mod api;
#[cfg(test)]
mod api_tests;
mod block_arity;
#[cfg(test)]
mod check_locals_tests;
pub(crate) mod contract;
#[cfg(test)]
mod contract_decl_tests;
mod error_stack;
#[cfg(test)]
mod error_stack_tests;
#[cfg(test)]
mod profile_liveness_tests;
pub(crate) mod report;
#[cfg(test)]
mod resource_usage_tests;
// The playground's stack view: read only through the WASM boundary.
#[cfg(any(feature = "wasm", test))]
pub(crate) mod stack_view;
#[cfg(test)]
mod stack_view_tests;

use crate::interpreter::{HostEffect, Interpreter, RuntimeLimits};
use crate::types::Token;
use crate::word_name::canonical_word_name;
use report::{ErrorInfo, Report};
use std::collections::HashMap;

pub(crate) fn error_report(
    interp: &Interpreter,
    mut error: ErrorInfo,
    message: String,
    output: Vec<String>,
) -> Report {
    // Every error gets the position of the top-level token that reached it.
    if error.position.is_none() {
        error.position = interp.current_source_position();
    }
    // The residue a failed run was holding is not worth the error that
    // explains it — see `agent::error_stack`.
    let residue = error_stack::elided_error_stack(interp);
    let resource_usage = interp.resource_usage();
    Report {
        status: "error",
        stack: residue.stack,
        stack_display: residue.stack_display,
        output,
        message: Some(message),
        error: Some(error),
        resource_usage,
        contract_decls: None,
        stack_elided: residue.elided,
        outcome: None,
    }
}

pub(crate) fn print_payloads(interp: &Interpreter) -> Vec<String> {
    interp
        .host_effects()
        .iter()
        .map(|effect| match effect {
            HostEffect::Print(payload) => payload.clone(),
        })
        .collect()
}

pub(crate) fn stack_display(interp: &Interpreter) -> Vec<String> {
    // One shared rendering (LANG.OBSERVATION.PROTOCOL) for every observation
    // surface.
    crate::types::display::render_stack(interp.get_stack())
}

/// The outcome of best-effort static word resolution.
pub(crate) struct ResolvedWords {
    /// Unknown words in first-appearance order, deduplicated.
    pub unknown: Vec<String>,
    /// Unknown words that some *other* frame of the same file binds: a name
    /// written inside a DEF body but bound at the top level, or the reverse.
    /// The runtime refuses these with its "bound in another frame" message,
    /// and `check` says the same thing instead of a bare "unknown word".
    pub bound_elsewhere: Vec<String>,
}

/// Best-effort static resolution: a word resolves when it is a builtin, a
/// word the file itself defines via DEF, or a name a
/// `BIND` in the same frame region binds.
///
/// Frame regions follow `bindings.rs`'s rule: a binding is reachable in the
/// frame that made it and in the blocks written there, never inside a Word
/// called from it. Statically, a Word body is the block a `] 'NAME' DEF`
/// closes; every token outside such a body belongs to the run's own frame,
/// and every token inside one belongs to that body's frame. Blocks a Core
/// Word evaluates (`EXEC`, `MAP`, `FOLD`) are transparent at runtime, so this
/// does not open a region for them — which is also why order within a region
/// does not matter: a block may be bound first and evaluated later.
pub(crate) fn resolve_words(interp: &Interpreter, tokens: &[Token]) -> ResolvedWords {
    use std::collections::{HashMap, HashSet};

    let mut defined: HashSet<String> = HashSet::new();
    // Pre-pass: `'NAME' DEF` definitions anywhere in the file (definitions may
    // be referenced before they appear, e.g. mutual recursion between user
    // words).
    for (i, token) in tokens.iter().enumerate() {
        let Token::String(text) = token else {
            continue;
        };
        let next_words: Vec<std::borrow::Cow<str>> = tokens[i + 1..]
            .iter()
            .take(2)
            .filter_map(|t| match t {
                Token::Symbol(s) => Some(canonical_word_name(s)),
                _ => None,
            })
            .collect();
        if next_words.iter().any(|w| w == "DEF") {
            defined.insert(text.to_uppercase());
        }
    }

    // Region of each token: 0 is the run's frame; a DEF body's region is the
    // index of its opening `[`, and nested DEF bodies get their own.
    let regions = frame_regions(tokens);

    // Every `BIND` makes bindings rather than Words — one name (`'N' BIND`)
    // or several (`[ 'A' 'B' ] BIND`). They are not in the dictionary, and
    // `check` resolves without running, so without this every bound name read
    // as an unknown Word and `check` refused programs that run.
    let mut bound: HashMap<usize, HashSet<String>> = HashMap::new();
    for (i, token) in tokens.iter().enumerate() {
        if !matches!(token, Token::Symbol(s) if canonical_word_name(s) == "BIND") {
            continue;
        }
        let region = regions[i];
        match i.checked_sub(1).map(|j| (j, &tokens[j])) {
            Some((_, Token::String(name))) => {
                bound.entry(region).or_default().insert(name.to_uppercase());
            }
            Some((close, Token::VectorEnd)) => {
                let mut depth = 0usize;
                for t in tokens[..=close].iter().rev() {
                    match t {
                        Token::VectorEnd => depth += 1,
                        Token::VectorStart => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        Token::String(name) if depth == 1 => {
                            bound.entry(region).or_default().insert(name.to_uppercase());
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let mut unknown: Vec<String> = Vec::new();
    let mut bound_elsewhere: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (i, token) in tokens.iter().enumerate() {
        let Token::Symbol(symbol) = token else {
            continue;
        };
        let canonical = canonical_word_name(symbol);
        let bound_here = bound
            .get(&regions[i])
            .is_some_and(|names| names.contains(canonical.as_ref()));
        let resolved = interp.core_vocabulary.contains_key(canonical.as_ref())
            || defined.contains(canonical.as_ref())
            || bound_here;
        if !resolved && seen.insert(canonical.to_string()) {
            if bound
                .values()
                .any(|names| names.contains(canonical.as_ref()))
            {
                bound_elsewhere.push(canonical.to_string());
            }
            unknown.push(canonical.into_owned());
        }
    }
    ResolvedWords {
        unknown,
        bound_elsewhere,
    }
}

/// The frame region of every token (see [`resolve_words`]). Assumes the
/// bracket structure already passed the tokenizer's structural validation
/// (`tokenizer::validate_code_tokens`'s rule, which `tokenize` holds every
/// result to).
fn frame_regions(tokens: &[Token]) -> Vec<usize> {
    // Match every `[` to its `]` first, so a block can be recognised as a
    // DEF body from its opening side.
    let mut close_of: HashMap<usize, usize> = HashMap::new();
    let mut open_stack: Vec<usize> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::VectorStart => open_stack.push(i),
            Token::VectorEnd => {
                if let Some(open) = open_stack.pop() {
                    close_of.insert(open, i);
                }
            }
            _ => {}
        }
    }
    let is_def_body = |open: usize| -> bool {
        let Some(&close) = close_of.get(&open) else {
            return false;
        };
        matches!(tokens.get(close + 1), Some(Token::String(_)))
            && contract::is_def_symbol(tokens.get(close + 2))
    };

    let mut regions = Vec::with_capacity(tokens.len());
    // (region id, index of the `]` that ends it)
    let mut region_stack: Vec<(usize, usize)> = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        while region_stack.last().is_some_and(|(_, end)| *end < i) {
            region_stack.pop();
        }
        if matches!(token, Token::VectorStart) && is_def_body(i) {
            region_stack.push((i + 1, close_of[&i]));
        }
        regions.push(region_stack.last().map_or(0, |(id, _)| *id));
    }
    regions
}

/// Poll the interpreter future to completion. `Interpreter::execute` is
/// `async` for the WASM host's benefit but contains no await points on either
/// the native or the one-shot WASM agent path (both drive it to completion
/// synchronously), so a no-op waker is sufficient; the yield is a safety
/// valve. Public for the calibration harnesses in `rust/examples/`.
pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::task::{Context, Poll};
    let mut fut = Box::pin(fut);
    let waker = std::task::Waker::noop();
    let mut cx = Context::from_waker(waker);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// A fresh interpreter with every priced ceiling lifted, for the calibration
/// harnesses in `rust/examples/`: they measure the price, not the limit. The
/// nesting depth is not lifted — it bounds the native stack, not a price.
pub fn unbounded_interpreter() -> Interpreter {
    let mut interp = Interpreter::new();
    interp.set_runtime_limits(RuntimeLimits {
        max_materialized_elements: 10_000_000,
        max_source_bytes: 64 * 1024 * 1024,
        max_numeric_literal_digits: 1_000_000,
        max_numeric_work: u64::MAX,
        max_collection_work: u64::MAX,
        max_bigint_bits: u64::MAX,
        max_algebraic_terms: usize::MAX,
        max_nesting_depth: RuntimeLimits::default().max_nesting_depth,
    });
    interp.set_max_execution_steps(usize::MAX);
    interp
}

/// Build the operands with `setup` on an [`unbounded_interpreter`], then time
/// `source` alone; returns the interpreter (for its meters and stack) and the
/// milliseconds `source` took. Panics if either fails: an unfinished run has
/// no price to measure.
///
/// The operand is built outside the timed region because a rate is `charged
/// units / elapsed ms`: time inside the interval that charges the measured
/// meter nothing drags the rate down (`0 99999 RANGE` is real milliseconds of
/// `collectionWork` and almost no `numericWork`). `execute` keeps the stack
/// across calls and resets the counters, so the second call sees the first
/// call's operand and counts only its own work.
#[cfg(not(target_arch = "wasm32"))]
pub fn time_after_setup(setup: &str, source: &str) -> (Interpreter, f64) {
    let mut interp = unbounded_interpreter();
    if !setup.is_empty() {
        if let Err(error) = block_on(interp.execute(setup)) {
            panic!("setup `{setup}` must succeed, got: {error:?}");
        }
    }
    let started = std::time::Instant::now();
    let outcome = block_on(interp.execute(source));
    let millis = started.elapsed().as_secs_f64() * 1000.0;
    if let Err(error) = outcome {
        panic!("`{source}` after `{setup}` must complete to be measurable, got: {error:?}");
    }
    (interp, millis)
}
