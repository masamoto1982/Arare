//! Stack-flow (arity) simulation for contract inference.
//!
//! Split out of `word_contract.rs`, where the same job was done by a
//! `FlowAccumulator` that read *every* token of a word body as if it stood at
//! the top level. That reading was unsound, because `[ ... ]` is a **literal
//! whose interior is not code**: it is evaluated only if some higher-order
//! Word later runs it as the Vector it built, never at the point it is
//! written — `[ 1 2 ADD ]` is the three-element vector `[ 1/1 2/1 'ADD' ]`,
//! not a call to `ADD`.
//!
//! A literal pushes exactly one value and consumes nothing, whatever it
//! contains. The old walk instead counted each interior `Number`/`String` as
//! a push and *applied* each interior `Symbol`'s arity, so `[ [ 1 2 ] ]`
//! `'PAIR' DEF` inferred `( 0 -- 2 )` for a word that produces one vector,
//! and `[ [ 2 MUL ] MAP ] 'DOUBLE-ALL' DEF` inferred `( 2 -- 1 )` for a word
//! whose true arity is `( 1 -- 1 )`. Those inferences are reported at
//! `ContractConfidence::Complete`, so a *correct* `#:contract` declaration was
//! rejected as a proven violation — a false `error`, which
//! `LANG.CONTRACT.CHECK` forbids.
//!
//! This simulation therefore tracks the literal depth and updates the height
//! only at depth zero, where the tokens really are code. A flow it cannot
//! state — a dependency of variable arity, a name nothing defines, an
//! unbalanced body — is `Dynamic`, which a declared count reads as "cannot
//! verify", never as a violation.
//!
use super::word_contract::ContractFlow;
use crate::types::Token;

/// Execution-free stack-flow simulation over a word body's token stream.
#[derive(Default)]
pub(crate) struct FlowSim {
    /// The flow is data-dependent because a dependency's own mass contract is
    /// `Dynamic`. A *derived* fact — but not a proof that this
    /// body's arity varies either.
    dynamic: bool,
    /// This simulation could not model the body (an unbalanced delimiter, or
    /// a count past `u16::MAX`, which a contract cannot state).
    unmodelled: bool,
    /// Values the body needs beneath what it pushed for itself.
    required: u16,
    /// Simulated stack height contributed by the body so far.
    height: u16,
    vector_depth: u32,
}

impl FlowSim {
    pub(crate) fn new() -> Self {
        FlowSim::default()
    }

    /// Inside a `[ ... ]`, where tokens are literal content
    /// rather than code and contribute nothing to the height.
    pub(crate) fn in_literal(&self) -> bool {
        self.vector_depth > 0
    }

    fn push_value(&mut self) {
        self.add_height(1);
    }

    /// Raise the height by `n`. A count past what `u16` holds is no count at
    /// all: saturating would leave `u16::MAX` standing as a proven arity for a
    /// body that pushes more.
    fn add_height(&mut self, n: u16) {
        match self.height.checked_add(n) {
            Some(height) => self.height = height,
            None => self.unmodelled = true,
        }
    }

    /// A `Number`/`String` literal token.
    pub(crate) fn feed_literal(&mut self) {
        if !self.in_literal() {
            self.push_value();
        }
    }

    /// A structural token. The closing delimiter of a *top-level* literal is
    /// where its single value is pushed — not the opening one, so a body that
    /// never closes it pushes nothing and is caught by `finish`.
    pub(crate) fn feed_structural(&mut self, token: &Token) {
        match token {
            // A literal's interior pushes nothing and its close pushes one
            // value.
            Token::VectorStart => self.vector_depth += 1,
            Token::VectorEnd => self.close(),
            Token::Number(_) | Token::String(_) | Token::Symbol(_) | Token::Value(_) => {}
        }
    }

    /// Close a `]`. A delimiter that closes nothing means the body is
    /// unbalanced, so every height from here on is a guess: give up rather
    /// than resynchronize onto an invented depth.
    fn close(&mut self) {
        if self.vector_depth == 0 {
            self.unmodelled = true;
            return;
        }
        self.vector_depth -= 1;
        if !self.in_literal() {
            self.push_value();
        }
    }

    /// A resolved dependency call: it consumes its operands and pushes its
    /// results.
    pub(crate) fn feed_word(&mut self, flow: &ContractFlow) {
        if self.in_literal() {
            return;
        }
        let ContractFlow::Fixed { consumes, produces } = flow else {
            self.dynamic = true;
            return;
        };
        if self.height < *consumes {
            match self.required.checked_add(consumes - self.height) {
                Some(required) => self.required = required,
                None => self.unmodelled = true,
            }
            self.height = 0;
        } else {
            self.height -= consumes;
        }
        self.add_height(*produces);
    }

    /// A symbol that did not resolve. Inside a literal the symbol is content,
    /// so the height is unaffected.
    pub(crate) fn go_dynamic(&mut self) {
        if !self.in_literal() {
            self.dynamic = true;
        }
    }

    /// The inferred flow. A literal left open at the end of the body is
    /// unbalanced for the same reason `close` treats a stray delimiter as one.
    pub(crate) fn finish(self) -> ContractFlow {
        if self.dynamic || self.unmodelled || self.in_literal() {
            ContractFlow::Dynamic
        } else {
            ContractFlow::Fixed {
                consumes: self.required,
                produces: self.height,
            }
        }
    }
}

/// Names a `BIND` in the body made. The rest of the body reads each as one
/// value; looking it up in the dictionary instead would report a bound name as
/// an unresolved Word and give up on the arity (the lexicon-emergence pilot's
/// finding M-2).
pub(crate) type BoundNames = std::collections::HashSet<String>;

/// At a `BIND`, record the names it binds: the String just before it, or each
/// String of the Vector literal just before it (`[ 'A' 'B' ] BIND`).
pub(crate) fn note_bound_names(
    bound: &mut BoundNames,
    canonical: &str,
    tokens: &[Token],
    idx: usize,
) {
    if canonical != "BIND" {
        return;
    }
    match idx.checked_sub(1).map(|j| (j, &tokens[j])) {
        Some((_, Token::String(name))) => {
            bound.insert(name.to_uppercase());
        }
        Some((close, Token::VectorEnd)) => {
            let mut depth = 0usize;
            for token in tokens[..=close].iter().rev() {
                match token {
                    Token::VectorEnd => depth += 1,
                    Token::VectorStart => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    Token::String(name) if depth == 1 => {
                        bound.insert(name.to_uppercase());
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
