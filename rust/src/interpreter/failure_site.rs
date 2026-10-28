//! Which Word a failed run names (LANG.OBSERVATION.DIAGNOSIS).
//!
//! An ERROR is reported by its category, its message and the Word it is
//! attributed to. The Word is the innermost one that raised the error, except
//! that attribution stops at a User Word: from the caller's side the Word is
//! what failed. Every dispatch the engine makes records through here, so
//! compiling stays unobservable (LANG.AUTHORITY.FREEDOM).

use crate::error::{AjisaiError, ErrorCategory, Result};
use crate::interpreter::Interpreter;

/// The Word a failure is attributed to, with the error text it was recorded
/// for, so an enclosing frame that sees the same error leaves the record alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FailedWord {
    pub(crate) word: String,
    error_text: String,
}

impl Interpreter {
    /// A name compiled as no Core Word's: a binding, else a User Word, with
    /// the failure recorded before it propagates.
    #[inline]
    pub(crate) fn dispatch_word_beyond_core(&mut self, name: &crate::types::Name) -> Result<()> {
        // A binding may not take a User Word's name and `DEF` may not take a
        // bound one, so at most one of the two answers and the order of the
        // two lookups is free: the Word is asked first, from its cache.
        let outcome = match self.user_word(name) {
            Some(def) => self.call_user_word(name, def),
            None => match self.lookup_binding(name) {
                Some(value) => {
                    self.stack.push(value);
                    return Ok(());
                }
                None => Err(self.unknown_word(name.as_str())),
            },
        };
        if let Err(err) = &outcome {
            self.record_word_dispatch_failure(name.as_str(), err);
        }
        outcome
    }

    /// A Core Word resolved to its registry entry when its code was compiled,
    /// with the failure recorded before it propagates.
    #[inline]
    pub(crate) fn dispatch_core_word(
        &mut self,
        word: &'static crate::kernel::generated::GeneratedWord,
    ) -> Result<()> {
        if super::quickened::try_scalar_call(self, word.id) {
            return Ok(());
        }
        let outcome = self.execute_core_word(word);
        if let Err(err) = &outcome {
            self.record_word_dispatch_failure(word.name, err);
        }
        outcome
    }

    /// Attribute `err` to `word` unless a Word inside it already holds it.
    #[cold]
    pub(crate) fn record_word_dispatch_failure(&mut self, word: &str, err: &AjisaiError) {
        let error_text = err.to_string();
        if self
            .failed_word
            .as_ref()
            .is_some_and(|failed| failed.error_text == error_text)
        {
            return;
        }
        self.failed_word = Some(FailedWord {
            word: word.to_string(),
            error_text,
        });
    }

    /// A User Word whose body failed takes the attribution over, except for a
    /// name in its body that resolved to nothing: that failure is the name's.
    #[cold]
    pub(crate) fn record_user_word_failure(&mut self, word: &str, err: &AjisaiError) {
        if matches!(
            ErrorCategory::from_error(err),
            Some(ErrorCategory::UnknownWord)
        ) {
            return;
        }
        self.failed_word = Some(FailedWord {
            word: word.to_string(),
            error_text: err.to_string(),
        });
    }

    /// The Word `err` is attributed to, if this run recorded one for it.
    pub(crate) fn failed_word_for(&self, err: &AjisaiError) -> Option<&str> {
        let failed = self.failed_word.as_ref()?;
        (failed.error_text == err.to_string()).then_some(failed.word.as_str())
    }
}
