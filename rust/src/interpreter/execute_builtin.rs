use crate::error::{AjisaiError, Result};
use crate::kernel::generated::{generated_word, WordId};
use std::sync::Arc;

use crate::types::{Name, Token, Value, WordDefinition};

use super::interpreter_core::WORD_CACHE;

use super::{
    arithmetic, bindings, cast, comparison, control, declared_nil_contract, execute_def,
    format_ops, higher_order, higher_order_fold, io, json_decode, json_encode, logic, math_ops,
    nil_diagnostics, ordering_ops, record_ops, reflection_ops, search_ops, shape_words, sort,
    tensor_cmds, vector_ops, Interpreter,
};

impl Interpreter {
    /// A Core Word resolved to its registry entry: one step, the Word, and
    /// the nesting check. `dispatch_core_word` asks the scalar route first.
    #[inline]
    pub(crate) fn execute_core_word(
        &mut self,
        word: &'static crate::kernel::generated::GeneratedWord,
    ) -> Result<()> {
        self.charge_execution_step()?;
        self.execute_generated_word(word)
            .and_then(|()| self.check_fresh_nesting())
    }

    /// The cache entry for `key`'s User Word, refreshed when the dictionary
    /// has changed since it was filled. A program calls a few Words many
    /// times, and a lookup by name hashed the text on every call.
    #[inline(always)]
    fn word_cache_entry(&mut self, key: &Name) -> &Option<Arc<WordDefinition>> {
        let bytes = key.as_bytes();
        let at = (bytes.len() + usize::from(bytes.last().copied().unwrap_or(0))) % WORD_CACHE;
        let epoch = self.dictionary_epoch;
        let entry = &mut self.word_cache[at];
        if entry.0 != epoch || entry.1 != *key {
            *entry = (
                epoch,
                key.clone(),
                self.user_words.get(key.as_str()).cloned(),
            );
        }
        &entry.2
    }

    /// The User Word `key` names now.
    #[inline(always)]
    pub(crate) fn user_word(&mut self, key: &Name) -> Option<Arc<WordDefinition>> {
        self.word_cache_entry(key).clone()
    }

    /// Whether a User Word holds `key`.
    #[inline(always)]
    pub(crate) fn is_user_word(&mut self, key: &Name) -> bool {
        self.word_cache_entry(key).is_some()
    }

    /// The failure of a name that resolved to nothing.
    #[cold]
    pub(crate) fn unknown_word(&self, name: &str) -> AjisaiError {
        // `UnknownWord` is the one structural category
        // LANG.DICTIONARY.RESOLUTION's failure has; an out-of-scope binding
        // is a refinement of it, not a condition any Word's own contract
        // could declare.
        if self.binding_exists_beyond_barrier(name) {
            // The reader can see the name in their own source, so the bare
            // "unknown word" is the least useful true thing to say. What went
            // wrong is the scope, and naming it is the difference between
            // reading the rule and rediscovering it.
            AjisaiError::UnknownWord(format!(
                "'{}' is bound in another frame. A binding is reachable in the frame that made it \
                 and in the blocks written there, never inside a Word it calls — pass the value \
                 as an operand instead.",
                name
            ))
        } else {
            AjisaiError::UnknownWord(name.to_string())
        }
    }

    /// Call the User Word `def`, named `name`.
    pub(crate) fn call_user_word(&mut self, name: &Name, def: Arc<WordDefinition>) -> Result<()> {
        self.charge_execution_step()?;

        if def.body.is_empty() {
            return self
                .execute_builtin_direct(name.as_str())
                .and_then(|()| self.check_fresh_nesting());
        }

        // Call-depth ceiling: refuses before the native stack overflows (a
        // WASM trap). It is reached by a long acyclic call chain. The
        // matching decrement is just before the return below; there are no
        // `?` early returns between this point and the decrement.
        self.check_call_depth()?;
        self.call_depth += 1;

        let code = self.body_code(&def);

        // A Word call is a barrier frame: its body names its own locals and
        // reads none of the caller's, so what a Word means depends on its
        // operands and its dictionary and nothing else.
        self.open_frame(true, &code.slots);

        let result = self.run_code(&code);

        self.close_binding_scope();

        self.call_depth -= 1;

        // Attribution stops at a User Word — see `record_user_word_failure`
        // for what that means and the one failure it does not cover.
        if let Err(err) = &result {
            self.record_user_word_failure(name.as_str(), err);
        }

        result
    }

    pub(crate) fn execute_builtin_direct(&mut self, name: &str) -> Result<()> {
        let Some(word) = generated_word(name) else {
            return Err(AjisaiError::UnknownWord(name.to_string()));
        };
        self.execute_generated_word(word)
    }

    /// Run a Core Word from its registry entry.
    ///
    /// Split out from `execute_builtin_direct` so the hot path can hand over the
    /// entry it already holds (`WordDefinition::generated`) instead of scanning
    /// the registry by name to find it again. Both routes run exactly this, so
    /// the declared-NIL contract is applied once and in one place whichever way
    /// the Word was reached — which is what keeps the choice of route
    /// unobservable (LANG.AUTHORITY.FREEDOM).
    pub(crate) fn execute_generated_word(
        &mut self,
        word: &'static crate::kernel::generated::GeneratedWord,
    ) -> Result<()> {
        self.unpack_truth_columns_for(word.id);
        let result = match self.apply_declared_nil_contract(word) {
            Some(decided) => decided,
            None => match self.apply_declared_lift(word) {
                Some(lifted) => lifted,
                None => self.execute_builtin_by_id(word.id),
            },
        };
        self.note_truth_column_on_top();
        result.map_err(|err| err.attributed_to(word.name))
    }

    /// Run a Word that evaluates a block (`EXEC`, `MAP`, `FILTER`, `FOLD`,
    /// `SCAN`) as one more level of `call_depth`.
    ///
    /// The block nests native frames just as a User Word body does, and the
    /// two multiply: a body of 250 nested `[ … ] EXEC` levels called down a
    /// chain of 22 Words, or a chain of blocks bound to one another, is
    /// thousands of frames deep while each guard alone is satisfied. So both
    /// count against the one guard. The level is taken here, at the Word,
    /// before any route is chosen, so a fused or compiled route reaches the
    /// same answer as the interpreted one (LANG.AUTHORITY.FREEDOM); a refusal
    /// comes before the operands are taken, so they stay where they were.
    ///
    /// It is also where the run of a block built from data is limited to one
    /// at a time (LANG.DICTIONARY.ACYCLIC): a block the running code wrote
    /// runs less code than the code running it, but a block built from data
    /// can be a copy of the very block running it, and two such runs nested
    /// are how a block would run itself without end. The refusal comes
    /// before the operands are taken, as the depth's does.
    fn in_block_frame(&mut self, op: fn(&mut Interpreter) -> Result<()>) -> Result<()> {
        self.check_call_depth()?;
        let from_data = !self.block_is_written();
        if from_data && self.data_block_running {
            return Err(AjisaiError::declared(
                "nestedDataBlock",
                "this block was built from data while another block built from data is running; \
                 a block run inside one must be written there (LANG.DICTIONARY.ACYCLIC)",
            ));
        }
        let outer = self.data_block_running;
        self.data_block_running = outer || from_data;
        self.call_depth += 1;
        let result = op(self);
        self.call_depth -= 1;
        self.data_block_running = outer;
        result
    }

    /// Refuse one more nested call or block evaluation past `MAX_CALL_DEPTH`
    /// as `resourceLimitExceeded`, naming `callDepth` like every other ceiling
    /// (LANG.MACHINE.LIMITS). Attribution to the Word that tried is the
    /// dispatcher's, as for any other refusal.
    fn check_call_depth(&self) -> Result<()> {
        let limit = super::interpreter_core::MAX_CALL_DEPTH;
        if self.call_depth + 1 > limit {
            return Err(AjisaiError::ResourceLimitExceeded {
                resource: crate::error::ResourceLimit::CallDepth,
                limit: limit as u64,
                observed: Some((self.call_depth + 1) as u64),
                progress: None,
            });
        }
        Ok(())
    }

    /// Run the primitive for a Word's canonical identity.
    ///
    /// `WordId` is generated from `spec/words.json`, so this match is total
    /// over the Words the specification declares: adding one to the canon
    /// fails the build here rather than at runtime, which is the registration
    /// gap the old hand-written key enum could not catch.
    pub(crate) fn execute_builtin_by_id(&mut self, id: WordId) -> Result<()> {
        match id {
            WordId::Add => arithmetic::op_add(self),
            WordId::Sub => arithmetic::op_sub(self),
            WordId::Mul => arithmetic::op_mul(self),
            WordId::Div => arithmetic::op_div(self),
            WordId::Eq => comparison::op_eq(self),
            WordId::Lt => comparison::op_lt(self),
            WordId::Gt => comparison::op_gt(self),
            WordId::Map => self.in_block_frame(higher_order::op_map),
            WordId::Filter => self.in_block_frame(higher_order::op_filter),
            WordId::Fold => self.in_block_frame(higher_order_fold::op_fold),
            WordId::Scan => self.in_block_frame(higher_order_fold::op_scan),
            WordId::Get => vector_ops::op_get(self),
            WordId::Length => vector_ops::op_length(self),
            WordId::Concat => vector_ops::op_concat(self),
            WordId::And => logic::op_and(self),
            WordId::Not => logic::op_not(self),
            WordId::Select => logic::op_select(self),
            // One arm, one temporary: this match sits on every nested
            // call's native frame, and an unoptimized build gives each
            // arm's `Value` its own slot.
            WordId::True | WordId::False | WordId::Nil => {
                self.stack.push(match id {
                    WordId::True => Value::from_bool(true),
                    WordId::False => Value::from_bool(false),
                    _ => Value::nil(),
                });
                Ok(())
            }
            WordId::Reconcile => logic::op_reconcile(self),
            WordId::Exec => self.in_block_frame(control::op_exec),
            WordId::Contract => reflection_ops::op_contract(self),
            WordId::Digest => reflection_ops::op_digest(self),
            WordId::Bind => bindings::op_bind(self),
            WordId::Def => execute_def::op_def(self),
            WordId::Del => execute_def::op_del(self),
            WordId::Print => io::op_print(self),
            WordId::Shape => shape_words::op_shape(self),
            WordId::Reshape => shape_words::op_reshape(self),
            WordId::Flatten => shape_words::op_flatten(self),
            WordId::Depth => shape_words::op_depth(self),
            WordId::Take => vector_ops::op_take(self),
            WordId::Drop => vector_ops::op_drop(self),
            WordId::Reverse => vector_ops::op_reverse(self),
            WordId::Range => vector_ops::op_range(self),
            WordId::Collect => vector_ops::op_collect(self),
            WordId::Fill => tensor_cmds::op_fill(self),
            WordId::Floor => tensor_cmds::op_floor(self),
            WordId::Round => tensor_cmds::op_round(self),
            WordId::Str => cast::op_str(self),
            WordId::Num => cast::op_num(self),
            WordId::Format => format_ops::op_format(self),
            WordId::JsonDecode => json_decode::op_json_decode(self),
            WordId::JsonEncode => json_encode::op_json_encode(self),
            WordId::Chars => cast::op_chars(self),
            WordId::Join => cast::op_join(self),
            WordId::Trim => cast::op_trim(self),
            WordId::Upper => cast::op_upper(self),
            WordId::Lower => cast::op_lower(self),
            WordId::Tokenize => cast::op_tokenize(self),
            WordId::Search => cast::op_search(self),
            WordId::Replace => cast::op_replace(self),
            WordId::NilCheck => nil_diagnostics::op_nil_check(self),
            WordId::NilReason => nil_diagnostics::op_nil_reason(self),
            WordId::Min => math_ops::op_min(self),
            WordId::Max => math_ops::op_max(self),
            WordId::Sqrt => math_ops::op_sqrt(self),
            WordId::Pow => math_ops::op_pow(self),
            WordId::Gcd => math_ops::op_gcd(self),
            WordId::Ratio => math_ops::op_ratio(self),
            WordId::Sort => sort::op_sort(self),
            WordId::Order => ordering_ops::op_order(self),
            WordId::Unique => ordering_ops::op_unique(self),
            WordId::Tally => ordering_ops::op_tally(self),
            WordId::Group => ordering_ops::op_group(self),
            WordId::Zip => shape_words::op_zip(self),
            WordId::Put => shape_words::op_put(self),
            WordId::Absent => declared_nil_contract::op_absent(self),
            WordId::Fail => declared_nil_contract::op_fail(self),
            WordId::Bsearch => search_ops::op_bsearch(self),
            WordId::Record => record_ops::op_record(self),
            WordId::Keys => record_ops::op_keys(self),
            WordId::Values => record_ops::op_values(self),
            WordId::Without => record_ops::op_without(self),
            WordId::Has => record_ops::op_has(self),
            WordId::Merge => record_ops::op_merge(self),
        }
    }

    /// A User Word's body, compiled on its first call and kept on the
    /// definition: compiled code depends on no dictionary state, so it never
    /// goes stale (`engine`).
    pub(crate) fn body_code(
        &self,
        def: &crate::types::WordDefinition,
    ) -> std::sync::Arc<super::engine::Code> {
        def.code
            .get_or_init(|| std::sync::Arc::new(super::engine::compile(&def.body, self)))
            .clone()
    }

    pub(crate) fn format_token_to_string(&self, token: &Token) -> String {
        match token {
            Token::Number(literal) => literal.lexeme().to_string(),
            Token::String(s) => format!("'{}'", s),
            Token::Symbol(s) => s.to_string(),
            Token::VectorStart => "[".to_string(),
            Token::VectorEnd => "]".to_string(),
            Token::Value(value) => value.to_string(),
        }
    }

    pub fn lookup_word_definition_tokens(&self, name: &str) -> Option<String> {
        let (_, def) = self.resolve_word_entry(name)?;
        if def.is_builtin || def.body.is_empty() {
            return None;
        }

        let words: Vec<String> = def
            .body
            .iter()
            .map(|token| self.format_token_to_string(token))
            .collect();
        Some(words.join(" "))
    }

    /// A User Word's `description` (SPEC: host affordance only, not a
    /// language effect) — the `#:contract` line text `DEF` captured for it,
    /// or a restored word's saved description. `None` for a builtin, or a
    /// user word that was never given one.
    pub fn lookup_word_description(&self, name: &str) -> Option<String> {
        let (_, def) = self.resolve_word_entry(name)?;
        if def.is_builtin {
            return None;
        }
        def.description.clone()
    }
}
