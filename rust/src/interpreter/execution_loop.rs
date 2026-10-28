use crate::error::{AjisaiError, Result};
use crate::types::{Token, Value};

use super::Interpreter;

impl Interpreter {
    pub async fn execute(&mut self, code: &str) -> Result<()> {
        // CS5: bound the input before it is expanded into values. Source bytes
        // are checked before tokenization allocates per-character buffers; each
        // numeric literal's digit count is checked after tokenization but
        // before `Fraction::from_str` parses it into a (potentially enormous)
        // BigInt-backed value.
        self.runtime_limits.check_source_bytes(code.len())?;
        self.execution_step_count = 0;
        self.numeric_work_used = 0;
        self.collection_work_used = 0;
        self.algebraic_terms_peak.set(0);
        self.dictionary_changes_this_run.clear();
        // A literal captured for a `DEF` that never ran — the dispatch failed
        // before `op_def` took it — must not become the body of the next run's
        // first `DEF`.
        self.pending_def_body_tokens = None;
        self.reset_binding_scopes();
        // Merge rather than replace: a `#:contract` line and the `DEF` it
        // documents can arrive in separate `execute()` calls (the Playground
        // runs one submission at a time), so an entry here must survive until
        // `op_def` actually consumes it for the Word it names.
        for (name, description) in
            crate::interpreter::execute_def::extract_pending_word_descriptions(code)
        {
            self.pending_word_descriptions.insert(name, description);
        }
        // Source entry is the one place a token has a position, so it is the
        // one place the positions are recorded. They are index-aligned with
        // `tokens`, and a failure is reported at the token it was reached from
        // (`run_program`).
        // A lexical failure is a fault in the writing, not in any value, so it
        // is classified as one rather than arriving as an uncategorized
        // `Custom` whose only next check was "read the message".
        let source =
            crate::tokenizer::tokenize_source(code).map_err(AjisaiError::MalformedSource)?;
        self.source_spans = source.spans;
        self.current_source_span = None;
        // The ceiling depends on a literal's lexeme alone, and every other
        // Number token is a copy of a first spelling that comes before it, so
        // checking the first spellings in order fails at the same token, with
        // the same count, as checking every token did.
        let first_numbers = source.first_numbers.iter().map(|&at| &source.tokens[at]);
        self.check_source_numeric_literals(first_numbers)?;
        self.run_program(&source.tokens, &source.firsts, source.lexemes)?;
        self.check_fresh_nesting()
    }

    /// Enforce the numeric-literal digit ceiling on every `Token::Number`
    /// produced from source, before any of them is parsed into a value. The
    /// count is of the digits the literal denotes — written digits plus the
    /// exponent's magnitude (`tokenizer::denoted_digit_count`) — so the bound
    /// tracks the magnitude of the BigInt that would be built: `1e99999999`
    /// is nine characters and used to spend minutes building its integer.
    pub(crate) fn check_source_numeric_literals<'a>(
        &self,
        tokens: impl IntoIterator<Item = &'a Token>,
    ) -> Result<()> {
        for token in tokens {
            if let Token::Number(literal) = token {
                let digits = literal.denoted_digits();
                self.runtime_limits
                    .check_numeric_literal_digits(usize::try_from(digits).unwrap_or(usize::MAX))?;
            }
        }
        Ok(())
    }
}

// Building a Vector literal (`[ ... ]`) from source tokens.
//
// What an *element* denotes is one piece of knowledge: a literal value, with
// a bare name (other than `TRUE`/`FALSE`/`NIL`, which still denote their
// values) becoming a `Value::Symbol` rather than a `Value::Text`. Nothing is
// looked up: a literal denotes the same value under every dictionary state.
impl Interpreter {
    /// The elements of one Vector literal, and how many tokens it spans.
    pub(crate) fn collect_bracketed_with_depth(
        tokens: &[Token],
        start_index: usize,
        depth: usize,
        limits: &crate::interpreter::RuntimeLimits,
    ) -> Result<(Vec<Value>, usize)> {
        if tokens.get(start_index) != Some(&Token::VectorStart) {
            return Err(AjisaiError::MalformedSource(
                "Expected a literal start".to_string(),
            ));
        }

        // The nesting ceiling (LANG.MACHINE.LIMITS), checked before recursing:
        // this builder descends one native frame per level, so a few thousand
        // levels of `[ [ [ ... ] ] ]` would overflow the native stack here,
        // before the value could be checked at all. It is the same ceiling a
        // value Words build meets, and fails the same way.
        limits.check_nesting_depth(depth)?;

        let mut values = Vec::new();
        let mut i = start_index + 1;

        while i < tokens.len() {
            match &tokens[i] {
                Token::VectorStart => {
                    let (nested_values, consumed) =
                        Self::collect_bracketed_with_depth(tokens, i, depth + 1, limits)?;
                    values.push(Value::from_vector_promoted(nested_values));
                    i += consumed;
                }
                Token::VectorEnd => {
                    return Ok((values, i - start_index + 1));
                }
                Token::Value(value) => {
                    values.push((**value).clone());
                    i += 1;
                }
                Token::Number(literal) => {
                    values.push(Value::from_number(
                        literal.parsed().map_err(AjisaiError::MalformedSource)?,
                    ));
                    i += 1;
                }
                Token::String(s) => {
                    values.push(Value::from_text(s));
                    i += 1;
                }
                Token::Symbol(s) => {
                    let upper = Self::normalize_symbol(s);
                    match upper.as_ref() {
                        "TRUE" => values.push(Value::from_bool(true)),
                        "FALSE" => values.push(Value::from_bool(false)),
                        "NIL" => values.push(Value::nil()),
                        // A bare name is a Symbol: data until something
                        // executes it, dictionary-independent (building the
                        // literal never looks anything up).
                        _ => values.push(Value::from_symbol(s)),
                    }
                    i += 1;
                }
            }
        }
        Err(AjisaiError::MalformedSource(
            "Unclosed bracketed literal".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    //! Vector literals are data: a name inside one is a Symbol, never a call,
    //! whatever the dictionary holds (LANG.VALUES.VECTOR).

    use crate::agent::block_on;
    use crate::interpreter::Interpreter;

    fn top(src: &str) -> String {
        let mut interp = Interpreter::new();
        block_on(interp.execute(src)).unwrap();
        interp
            .get_stack()
            .last()
            .map(|v| format!("{v}"))
            .unwrap_or_default()
    }

    #[test]
    fn symbol_in_vector_is_data_not_executed() {
        let rendered = top("[ [ 10 ] ] 'TEN' DEF\n[ [ TEN 2 3 ] ] 'W' DEF\nW");
        assert!(rendered.contains("TEN"), "{rendered}");
        assert!(!rendered.contains("10"), "{rendered}");
    }

    #[test]
    fn vector_literal_is_independent_of_dictionary_state() {
        assert_eq!(top("[ [ 99 ] ] 'FOO' DEF\n[ FOO 1 ]"), top("[ FOO 1 ]"));
    }

    #[test]
    fn literal_vectors_in_a_body_are_built_as_written() {
        let rendered = top("[ [ 1 2 3 ] [ 4 5 6 ] ADD ] 'W' DEF W");
        assert!(
            rendered.contains("5/1") && rendered.contains("9/1"),
            "{rendered}"
        );
        assert_eq!(top("[ [ ] ] 'W' DEF W"), top("[ ]"));
        assert_eq!(top("[ [ 1 NIL 3 ] ] 'W' DEF W"), top("[ 1 NIL 3 ]"));
    }
}
