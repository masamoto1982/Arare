//! The ERROR a Word raises when the result it was asked to build will not
//! fit under a host ceiling (LANG.MACHINE.LIMITS).
//!
//! Separate from `runtime_limits`, whose checks measure a value that already
//! exists. These measure a request before anything is built — the element
//! count a shape names, the depth a JSON text nests, the digits a lexeme
//! denotes — and refuse it the same way: `resourceLimitExceeded`, naming the
//! ceiling that fired, its configured value and the size that crossed it.
//!
//! A ceiling is a property of the host, not of the program, so crossing one
//! is never a value. Through 1.0.0-beta.1 a generative Word *projected* a
//! `NIL(spaceExhausted)` here, and that NIL flowed on like any other: the
//! same program then answered `status: ok` with different values on two
//! conforming hosts — `0 X X NIL? SELECT` silently took the fallback on the
//! host with the smaller ceiling and the real answer on the other. An ERROR
//! stops the run instead, so a value a program observes denotes the same
//! thing on every host (LANG.VALUES.DENOTATION) and a host difference is
//! always visible as a failure, never as a different answer.

use crate::error::{AjisaiError, ResourceLimit};

/// A result of more elements than `materializedElements` admits.
///
/// `observed` is the whole requested count, measured before anything was
/// built; `None` when the count itself overflows the machine's word, which
/// is past every ceiling there is. `progress` is `None` on purpose: it exists
/// for a cumulative meter that stops the instant the budget is crossed, and
/// this is a size ceiling, measured whole before the build.
pub(crate) fn materialization_refused(limit: usize, observed: Option<u128>) -> AjisaiError {
    AjisaiError::ResourceLimitExceeded {
        resource: ResourceLimit::MaterializedElements,
        limit: limit as u64,
        observed: observed.and_then(|count| u64::try_from(count).ok()),
        progress: None,
    }
}

/// A result that would nest deeper than `nestingDepth`: a shape of too many
/// axes, or JSON text nested too deep. The same refusal
/// `RuntimeLimits::check_nesting_depth` makes for a value that already
/// exists, made before the value is built.
pub(crate) fn nesting_refused(limit: usize, observed: usize) -> AjisaiError {
    AjisaiError::ResourceLimitExceeded {
        resource: ResourceLimit::NestingDepth,
        limit: limit as u64,
        observed: Some(observed as u64),
        progress: None,
    }
}

/// Text that spells a number of more digits, the exponent's magnitude
/// counted, than `numericLiteralDigits` allows a source literal
/// (`tokenizer::denoted_digit_count`). A Word that reads the numeric grammar
/// out of data refuses it exactly as the tokenizer refuses the same spelling
/// in source.
pub(crate) fn numeric_literal_refused(limit: usize, observed: u64) -> AjisaiError {
    AjisaiError::ResourceLimitExceeded {
        resource: ResourceLimit::NumericLiteralDigits,
        limit: limit as u64,
        observed: Some(observed),
        progress: None,
    }
}
