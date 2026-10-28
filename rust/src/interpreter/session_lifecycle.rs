//! Session lifecycle.
//!
//! `execute_reset` returns the interpreter to a clean state: stack,
//! dictionary and output. The epochs keep counting, so nothing compiled
//! before the reset can match a dictionary made after it. Compiling a word
//! body is an unobservable
//! implementation detail (LANG.AUTHORITY.FREEDOM), so nothing here changes what
//! a program produces.

use crate::error::Result;

use super::Interpreter;

/// One saved definition that could not be restored, and why.
///
/// A restore reports these rather than raising: see
/// [`Interpreter::restore_user_word_definitions`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedRestore {
    pub name: String,
    pub reason: String,
}

impl Interpreter {
    /// Full reset: clears every trace of the previous program.
    pub fn execute_reset(&mut self) -> Result<()> {
        self.reset_session_state();
        Ok(())
    }

    /// Restore saved User Word definitions, skipping the ones that cannot be
    /// restored instead of abandoning the ones that can.
    ///
    /// Each entry is `(name, definition source, description)`. A saved
    /// definition is source text, so restoring it re-runs the lexer and `DEF`
    /// against *today's* rules — and those rules are not frozen. A dictionary
    /// saved before a lexical or naming rule changed can therefore contain an
    /// entry this build no longer accepts, which is not a reason to lose the
    /// rest of it: one unreadable definition used to abort the whole restore
    /// and leave the session holding whichever words happened to precede it.
    ///
    /// The skipped entries are returned instead, so the host can say which
    /// words did not come back. This is the same contract the host already
    /// states for a partially corrupt import — "valid words in a
    /// partially-corrupt file still import" (`interpreter-state-persistence.ts`)
    /// — which it could only honour for entries malformed structurally enough
    /// to spot without the lexer.
    ///
    /// An entry with an empty definition is passed over without a report:
    /// there is nothing to restore and nothing went wrong.
    pub fn restore_user_word_definitions<I>(&mut self, words: I) -> Result<Vec<SkippedRestore>>
    where
        I: IntoIterator<Item = (String, String, Option<String>)>,
    {
        let mut skipped = Vec::new();

        // Defer the whole-dictionary work a DEF does — the referrer scan, the
        // identity recomputation, the body-store sweep and the epoch bump —
        // during the bulk restore, and do each once below via
        // `rebuild_dependencies`. Per restored word that work is a pass over
        // every word, so running it inline made the restore O(N^2). The flag
        // is always cleared, even on error, so later interactive definitions
        // do it inline again.
        self.defer_identity_recompute = true;
        for (name, definition, description) in words {
            if definition.is_empty() {
                continue;
            }
            match self.restore_one_word(&name, &definition, description) {
                Ok(()) => {}
                Err(reason) => skipped.push(SkippedRestore { name, reason }),
            }
        }
        self.defer_identity_recompute = false;

        self.rebuild_dependencies()?;
        Ok(skipped)
    }

    /// Restore one saved definition, reporting why it could not be as a string.
    fn restore_one_word(
        &mut self,
        name: &str,
        definition: &str,
        description: Option<String>,
    ) -> std::result::Result<(), String> {
        let tokens = crate::tokenizer::tokenize(definition)?;
        super::execute_def::op_def_inner(self, name, &tokens).map_err(|e| e.to_string())?;
        if description.is_some() {
            super::execute_def::set_word_description(self, name, description);
        }
        Ok(())
    }

    /// Clear all ephemeral session state and re-register the core vocabulary.
    fn reset_session_state(&mut self) {
        self.stack.clear();
        self.core_vocabulary.clear();
        self.user_words.clear();
        self.bump_dictionary_epoch();
        self.dependents.clear();
        self.output_buffer.clear();
        self.host_effects.clear();
        self.pending_def_body_tokens = None;
        self.pending_word_descriptions.clear();
        self.call_depth = 0;
        self.code_literals.clear();
        self.code_literal_bases.clear();
        self.data_block_running = false;
        self.source_spans.clear();
        self.current_source_span = None;
        self.word_identities.clear();
        self.body_store.clear();
        self.defer_identity_recompute = false;
        self.next_registration_order = 1;
        self.failed_word = None;
        crate::builtins::register_builtins(&mut self.core_vocabulary);
    }
}

#[cfg(test)]
mod tests {
    //! Restoring a saved dictionary.
    //!
    //! A saved definition is source text, so restoring it re-runs the lexer and
    //! `DEF` against today's rules — and those rules are not frozen. One entry this
    //! build no longer accepts used to abort the whole restore, leaving the session
    //! holding whichever words happened to precede it; these tests hold the
    //! opposite contract, the one the host already states for a partially corrupt
    //! import: everything restorable is restored, and what was not comes back named.

    use crate::interpreter::Interpreter;

    fn word(name: &str, definition: &str) -> (String, String, Option<String>) {
        (name.to_string(), definition.to_string(), None)
    }

    #[tokio::test]
    async fn an_unreadable_entry_does_not_take_the_readable_ones_with_it() {
        let mut interp = Interpreter::new();
        let skipped = interp
            .restore_user_word_definitions([
                word("FIRST", "[ 1 ]"),
                // No longer lexes: a bracket must stand alone (LANG.SOURCE.TEXT).
                word("LEGACY", "[1]"),
                word("LAST", "[ 3 ]"),
            ])
            .expect("a skippable entry is not a restore failure");

        assert_eq!(skipped.len(), 1, "exactly one entry was unreadable");
        assert_eq!(skipped[0].name, "LEGACY");
        assert!(
            skipped[0].reason.contains("must stand alone"),
            "the skip should carry the lexer's reason, got: {}",
            skipped[0].reason
        );

        // The point of the exercise: the words either side of it survived.
        assert!(interp.user_words.contains_key("FIRST"));
        assert!(interp.user_words.contains_key("LAST"));
        assert!(!interp.user_words.contains_key("LEGACY"));

        // And they are callable, not just present.
        interp.execute("FIRST").await.expect("FIRST should run");
        assert_eq!(interp.stack.len(), 1);
    }

    /// The same holds when the entry is refused by `DEF` rather than by the
    /// lexer — a name saved before the unwritable-name rule, say. The two
    /// changes meet here: skipping is what makes tightening `DEF` safe for a
    /// dictionary saved under the older rule.
    #[tokio::test]
    async fn an_entry_def_refuses_is_skipped_too() {
        let mut interp = Interpreter::new();
        let skipped = interp
            .restore_user_word_definitions([
                word("KEPT", "[ 1 ]"),
                word("A[B", "[ 2 ]"),
                word("ALSO-KEPT", "[ 3 ]"),
            ])
            .expect("a refused name is not a restore failure");

        assert_eq!(
            skipped.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["A[B"]
        );
        assert!(interp.user_words.contains_key("KEPT"));
        assert!(interp.user_words.contains_key("ALSO-KEPT"));
    }

    #[tokio::test]
    async fn a_clean_dictionary_restores_whole_and_reports_nothing() {
        let mut interp = Interpreter::new();
        let skipped = interp
            .restore_user_word_definitions([word("ONE", "[ 1 ]"), word("TWO", "[ 2 ]")])
            .expect("nothing here is unreadable");

        assert!(skipped.is_empty(), "nothing should be reported skipped");
        assert!(interp.user_words.contains_key("ONE"));
        assert!(interp.user_words.contains_key("TWO"));
    }

    /// An entry with no saved body is not a failure to report — there is
    /// nothing to restore and nothing went wrong. The host relies on this:
    /// `restore_user_words` skips a definition-less word.
    #[tokio::test]
    async fn a_definition_less_entry_is_passed_over_silently() {
        let mut interp = Interpreter::new();
        let skipped = interp
            .restore_user_word_definitions([word("EMPTY", ""), word("REAL", "[ 1 ]")])
            .expect("an empty definition is not a failure");

        assert!(skipped.is_empty(), "an absent body is not a skip to report");
        assert!(!interp.user_words.contains_key("EMPTY"));
        assert!(interp.user_words.contains_key("REAL"));
    }

    /// A word whose body calls one that was skipped still restores: the
    /// reference simply does not resolve, exactly as a forward reference does
    /// not, and it fails at call time rather than at restore time.
    #[tokio::test]
    async fn a_dependent_of_a_skipped_word_still_restores() {
        let mut interp = Interpreter::new();
        let skipped = interp
            .restore_user_word_definitions([word("MISSING", "[1]"), word("CALLER", "[ MISSING ]")])
            .expect("the dependency rebuild must survive an unresolved reference");

        assert_eq!(skipped.len(), 1);
        assert!(interp.user_words.contains_key("CALLER"));
    }
}
