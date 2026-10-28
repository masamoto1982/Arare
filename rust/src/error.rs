use crate::diagnostic_hint::{reported_domain, unknown_word_hint, usage_hint};
use crate::interpreter::tensor_ops::aligned_dim;
use std::fmt;

pub type Result<T> = std::result::Result<T, AjisaiError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NilReason {
    NotFound,
    InvalidEncoding,
    IndexOutOfBounds,
    // `LogicallyUnknown` was retired: no `NilReason` value represents the
    // logical truth value UNKNOWN (LANG.VALUES.TRUTH): UNKNOWN is a NIL read
    // in truth position, carrying whatever reason that NIL has.
    //
    // `SpaceExhausted` was retired after 1.0.0-beta.1: a generative Word whose
    // result would not fit under a host ceiling now raises
    // `ResourceLimitExceeded` like every other ceiling (LANG.MACHINE.LIMITS).
    // A ceiling is the host's, not the program's, and a NIL that flowed on
    // from it let the same program answer `ok` with different values on two
    // hosts; no reason may name a condition that depends on the host.
    /// A well-formed operation applied to an input outside its domain — the
    /// canonical case being `SQRT` of a negative rational, which
    /// LANG.FAILURE.PROJECT calls a "well-formed domain miss". The NIL
    /// Projection Rule projects it to NIL with `absence.reason = domainMiss`
    /// (LANG.FAILURE.PROJECT).
    ///
    /// Deliberately named for the classification, not for the operation: a
    /// domain miss is recoverable by supplying a different input, which is what
    /// distinguishes it from an execution failure and what makes the same
    /// variant right for future domain misses in other words.
    DomainMiss,
    /// A NIL the program *wrote* rather than computed — the `NIL` Word and the
    /// `NIL` symbol inside a vector literal.
    ///
    /// `LANG.VALUES.NIL` makes the reason a NIL's entire observable content, so
    /// a written absence needs one too; without it `NIL NIL-REASON` answered
    /// NIL and the value had nothing to observe (audit finding D24). It names
    /// the only thing true of it: nothing failed, it was written down.
    Literal,
    /// An absence the program declared itself with `ABSENT`. The reason the
    /// registry closes over is this one id; the text the program gave lives
    /// beside it as the value's `detail` (`AbsenceMetadata::detail`), is what
    /// `NIL-REASON` answers, and is part of the value (LANG.VALUES.NIL) — the
    /// first parameterized reason, recorded as such in `spec/outcomes.json`.
    UserDeclared,
    /// Two sources `RECONCILE` was asked to agree on hold different values
    /// that are not both truth values, so there is no one value they agree
    /// on (LANG.VALUES.TRUTH). Unlike every other absence it is absorbing
    /// under `RECONCILE`: a disagreement already found is not settled by a
    /// third source, which keeps the Word associative.
    Conflict,
}

/// One named internal-computation ceiling from
/// [`crate::interpreter::RuntimeLimits`].
///
/// A host declares each ceiling separately — the MCP adapter publishes all of
/// them in `mcp.limits` — so an agent that hits one has to be able to tell
/// *which* one fired without reading an English sentence. The protocol
/// spelling is deliberately the same identifier the host profile publishes,
/// so `resourceLimit.resource` indexes straight into the declared limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimit {
    /// Byte length of one source program (`max_source_bytes`).
    SourceBytes,
    /// Digit count of one numeric literal (`max_numeric_literal_digits`).
    NumericLiteralDigits,
    /// Accumulated internal numeric work units (`max_numeric_work`).
    NumericWork,
    /// Accumulated collection work units (`max_collection_work`) — the element
    /// copies, comparisons and equality probes a collection Word performs
    /// inside a single execution step.
    ///
    /// Separate from `NumericWork` because the two count different things and
    /// prescribe different fixes. `numericWork` counts limb multiplies and says
    /// "compute less"; this counts element operations and says "work over a
    /// smaller collection". Folding them into one number would leave an agent
    /// unable to tell a wide arithmetic chain from a large `UNIQUE`, and would
    /// send it to shrink the wrong thing.
    CollectionWork,
    /// Bit length of a BigInt arithmetic result (`max_bigint_bits`).
    BigintBits,
    /// Algebraic term count of one exact value (`max_algebraic_terms`).
    AlgebraicTerms,
    /// How many containers deep one value nests (`max_nesting_depth`) —
    /// whether written as a literal, decoded from JSON, or built by Words.
    NestingDepth,
    /// How many User Word calls and block evaluations are active at once
    /// (`interpreter_core::MAX_CALL_DEPTH`). Each nests native frames, so the
    /// depth is bounded where the native stack would otherwise overflow.
    /// Fixed by the engine rather than chosen by a host, and published in
    /// every limit profile beside the ceilings a host does choose.
    CallDepth,
    /// Execution-step budget (`Interpreter::max_execution_steps`). Kept in the
    /// same vocabulary even though it lives outside `RuntimeLimits`, because a
    /// host publishes it as one more entry in the same limit table.
    ExecutionSteps,
    /// Materialized element count of one generated collection
    /// (`max_materialized_elements`): what `RANGE`, `FILL`, `RESHAPE`,
    /// `JSON-DECODE`, `CHARS` and `TOKENIZE` measure before they build, and
    /// raise by this name when the request will not fit
    /// (`interpreter::ceiling_refusal`).
    MaterializedElements,
}

/// How far an incrementally charged operation had got when its budget ran out.
///
/// For a *size* ceiling `observed` is the whole story: `bigintBits` observing
/// 272,133 against a limit of 262,144 is a real measurement of a real value,
/// visibly 4% over. A *cumulative work* ceiling charged as it goes cannot say
/// that — it stops the instant the budget is crossed, so `observed` reads a
/// hair over `limit` whether the caller asked for one element too many or
/// fifteen times too many.
///
/// Not a harmless imprecision: measured against a real model (`eval/traces/`),
/// reading it proportionally is exactly what happens. Refused at 0.02% over
/// while deduplicating 100,000 integers, the model retried with 99,999 and then
/// 99,979 and failed both times; it needed a 94% cut and nothing in the refusal
/// pointed there. Reporting where the scan stopped is the answer rather than a
/// hint at it — the budget bought exactly `completed` elements of this data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceProgress {
    /// Units of `unit` fully processed before the budget ran out.
    pub completed: u64,
    /// Units of `unit` the operation was asked for.
    pub total: u64,
    /// What is being counted. `"elements"` for the collection scans, which are
    /// the only operations charged incrementally today.
    pub unit: &'static str,
}

/// The unit the collection scans count in.
pub const PROGRESS_UNIT_ELEMENTS: &str = "elements";

impl ResourceLimit {
    pub fn as_protocol_str(&self) -> &'static str {
        match self {
            ResourceLimit::SourceBytes => "sourceBytes",
            ResourceLimit::NumericLiteralDigits => "numericLiteralDigits",
            ResourceLimit::NumericWork => "numericWork",
            ResourceLimit::CollectionWork => "collectionWork",
            ResourceLimit::BigintBits => "bigintBits",
            ResourceLimit::AlgebraicTerms => "algebraicTerms",
            ResourceLimit::NestingDepth => "nestingDepth",
            ResourceLimit::ExecutionSteps => "executionSteps",
            ResourceLimit::MaterializedElements => "materializedElements",
            ResourceLimit::CallDepth => "callDepth",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCategory {
    StackUnderflow,
    UnknownWord,
    MalformedSource,
    ExecutionLimitExceeded,
    /// A named `RuntimeLimits` ceiling other than the step budget. Separate
    /// from `ExecutionLimitExceeded` so "the program never terminated" and
    /// "one value grew past the declared size ceiling" stop sharing an answer.
    ResourceLimitExceeded,
    /// A `#:contract` declaration the inferred contract contradicts
    /// (`agent::contract`). Raised before anything runs — by `check`,
    /// and by `compute`, which refuses to execute a program whose own
    /// declaration about itself is false — so it has no `AjisaiError`
    /// behind it: no Word ever raises it.
    ContractViolation,
    /// The condition the failing Word's `errorWhen` declares for this state.
    /// Its protocol spelling *is* the declared condition name, so a reader who
    /// asked `word_contract` for the Word gets back the same vocabulary the
    /// failure answers in.
    Declared(&'static str),
}

impl ErrorCategory {
    pub fn as_protocol_str(&self) -> &'static str {
        match self {
            ErrorCategory::StackUnderflow => "stackUnderflow",
            ErrorCategory::UnknownWord => "unknownWord",
            ErrorCategory::MalformedSource => "malformedSource",
            ErrorCategory::ExecutionLimitExceeded => "executionLimitExceeded",
            ErrorCategory::ResourceLimitExceeded => "resourceLimitExceeded",
            ErrorCategory::ContractViolation => "contractViolation",
            ErrorCategory::Declared(condition) => condition,
        }
    }

    /// The spec/outcomes.json error category `err` reports as.
    pub fn from_error(err: &AjisaiError) -> Option<Self> {
        Some(match err {
            AjisaiError::StackUnderflow { .. } => ErrorCategory::StackUnderflow,
            AjisaiError::UnknownWord(_) => ErrorCategory::UnknownWord,
            AjisaiError::MalformedSource(_) => ErrorCategory::MalformedSource,
            AjisaiError::ExecutionLimitExceeded { .. } => ErrorCategory::ExecutionLimitExceeded,
            AjisaiError::ResourceLimitExceeded { .. } => ErrorCategory::ResourceLimitExceeded,
            AjisaiError::DeclaredCondition { condition, .. } => ErrorCategory::Declared(condition),
        })
    }
}

impl NilReason {
    pub fn as_protocol_str(&self) -> &'static str {
        match self {
            NilReason::NotFound => "notFound",
            NilReason::InvalidEncoding => "invalidEncoding",
            NilReason::IndexOutOfBounds => "indexOutOfBounds",
            NilReason::DomainMiss => "domainMiss",
            NilReason::Literal => "literal",
            NilReason::UserDeclared => "userDeclared",
            NilReason::Conflict => "conflict",
        }
    }

    /// Every `NilReason`, so a boundary that must round-trip one can search
    /// this list instead of restating the mapping. `as_protocol_str` stays the
    /// single spelling authority: a new reason is added here and named there,
    /// and `from_protocol_str` follows without another table to update.
    pub const ALL: &'static [NilReason] = &[
        NilReason::NotFound,
        NilReason::InvalidEncoding,
        NilReason::IndexOutOfBounds,
        NilReason::DomainMiss,
        NilReason::Literal,
        NilReason::UserDeclared,
        NilReason::Conflict,
    ];

    /// The reason a protocol string names, or `None` when it names none.
    ///
    /// LANG.VALUES.NIL makes the reason the entire observable content of a
    /// NIL, so a boundary that persists a value and reads it back has to carry
    /// the reason across or it changes the value. The persistence codec and
    /// the value arena both decode through here.
    pub fn from_protocol_str(s: &str) -> Option<NilReason> {
        NilReason::ALL
            .iter()
            .find(|reason| reason.as_protocol_str() == s)
            .copied()
    }
}

#[derive(Debug, Clone)]
pub enum AjisaiError {
    /// A Word was called with fewer operands than its declared arity. `word`
    /// is filled in at dispatch (`attributed_to`), like a declared condition's,
    /// so the message names the Word that was short.
    StackUnderflow {
        word: Option<&'static str>,
    },
    UnknownWord(String),
    /// Program text that does not parse: an unclosed or crossed delimiter, a
    /// delimiter glued to a name, an unclosed string. The fault is in the
    /// writing, not in any value, so it belongs to neither the value-shape nor
    /// the user-logic families.
    MalformedSource(String),
    ExecutionLimitExceeded {
        limit: usize,
    },
    /// A named internal-computation ceiling was crossed: one value grew past
    /// `max_bigint_bits` / `max_algebraic_terms`, a literal past
    /// `max_numeric_literal_digits`, a program past `max_source_bytes`, or the
    /// accumulated work meter past `max_numeric_work`.
    ///
    /// Carries the ceiling's own name, its configured value and — when the
    /// guard measured one — the observed size, so a host can report which of
    /// the limits it declared actually fired and by how much it was missed.
    ResourceLimitExceeded {
        resource: ResourceLimit,
        limit: u64,
        observed: Option<u64>,
        /// How far the refused operation had got, when the operation is one
        /// that is charged as it goes. `None` everywhere else — see
        /// [`ResourceProgress`] for why the distinction is the whole point.
        progress: Option<ResourceProgress>,
    },
    /// A raise the failing Word's own registry entry already names: `condition`
    /// is one of the conditions its `errorWhen` declares, spelled the way
    /// `spec/words.json` spells it.
    ///
    /// Naming the condition at the raise site is what lets the classification
    /// be derived rather than guessed. Every raise site names one — there is
    /// no catch-all variant left to fall back on: an undeclared outcome is a
    /// compile error, not a `custom`/`unknown` at runtime.
    DeclaredCondition {
        condition: &'static str,
        message: String,
        /// The Word the failure belongs to, attached once by the dispatcher
        /// as the error leaves that Word (`execute_generated_word`). The
        /// message itself never spells the Word's name: every declared
        /// message is written "expected …, got …", and `Display` prefixes the
        /// name, so every one reads `WORD: expected …` and no raise site can
        /// forget, misspell, or borrow another Word's name.
        word: Option<&'static str>,
    },
}

impl AjisaiError {
    /// A stack short of operands, not yet attributed to a Word.
    pub const fn stack_underflow() -> Self {
        AjisaiError::StackUnderflow { word: None }
    }

    /// Raise the named condition from the failing Word's `errorWhen`.
    pub fn declared(condition: &'static str, message: impl Into<String>) -> Self {
        AjisaiError::DeclaredCondition {
            condition,
            message: message.into(),
            word: None,
        }
    }

    /// Two operands whose shapes do not align on `axis` (LANG.COLLECTIONS.LIFT).
    /// One condition for every alignment a Word performs — lifting, pairing
    /// keys with values, zipping rows — so a length that differs is
    /// `shapeMismatch` whichever Word noticed it.
    ///
    /// `axis` counts the right-aligned axes of the two shapes, so the lengths
    /// reported are the ones that met there, not `left[axis]` and
    /// `right[axis]`: for `[2, 3]` against `[4]` axis 1 is 3 and 4.
    pub fn shape_mismatch(left: &[usize], right: &[usize], axis: usize) -> Self {
        let rank = left.len().max(right.len());
        AjisaiError::declared(
            "shapeMismatch",
            format!(
                "expected shapes that align, got {:?} and {:?} (axis {} is {} and {}, and neither is 1)",
                left,
                right,
                axis,
                aligned_dim(left, rank, axis),
                aligned_dim(right, rank, axis)
            ),
        )
    }

    /// Two sequences that must pair position by position and do not.
    pub fn length_mismatch(left: usize, right: usize) -> Self {
        AjisaiError::declared(
            "shapeMismatch",
            format!(
                "expected Vectors of the same length, got {} and {}",
                left, right
            ),
        )
    }

    /// Attach the Word a declared failure belongs to, if none is attached
    /// yet. The innermost Word wins: an error raised by `ADD` inside a `MAP`
    /// block is `ADD`'s, and `MAP` passing it on does not relabel it.
    ///
    /// `declaredFailure` is the program's own text (`FAIL`), so it is left as
    /// the program wrote it.
    pub fn attributed_to(self, name: &'static str) -> Self {
        match self {
            AjisaiError::DeclaredCondition {
                condition,
                message,
                word: None,
            } if condition != "declaredFailure" => AjisaiError::DeclaredCondition {
                condition,
                message,
                word: Some(name),
            },
            AjisaiError::StackUnderflow { word: None } => {
                AjisaiError::StackUnderflow { word: Some(name) }
            }
            other => other,
        }
    }
}

impl fmt::Display for AjisaiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AjisaiError::StackUnderflow { word: Some(word) } => {
                write!(f, "{}: stack underflow", word)?;
                write_hint(f, usage_hint(word, "stackUnderflow", None))
            }
            AjisaiError::StackUnderflow { word: None } => write!(f, "stack underflow"),
            AjisaiError::UnknownWord(name) => {
                write!(f, "Unknown word: {}", name)?;
                // A refined diagnosis (a binding out of scope) arrives as a
                // sentence rather than a name, and is its own repair.
                if name.contains(char::is_whitespace) {
                    return Ok(());
                }
                write_hint(f, unknown_word_hint(name))
            }
            AjisaiError::MalformedSource(msg) => write!(f, "{}", msg),
            AjisaiError::ExecutionLimitExceeded { limit } => {
                write!(f, "Execution step limit ({}) exceeded", limit)
            }
            AjisaiError::ResourceLimitExceeded {
                resource,
                limit,
                observed,
                progress,
            } => match observed {
                // The progress clause carries the repair. Without it the
                // sentence reads as a near miss on every incrementally charged
                // ceiling, because that is the only thing `observed` can say
                // there — see `ResourceProgress`.
                Some(observed) => match progress {
                    Some(progress) => write!(
                        f,
                        "{} of {} exceeds the limit of {} after {} of {} {}",
                        resource.as_protocol_str(),
                        observed,
                        limit,
                        progress.completed,
                        progress.total,
                        progress.unit
                    ),
                    None => write!(
                        f,
                        "{} of {} exceeds the limit of {}",
                        resource.as_protocol_str(),
                        observed,
                        limit
                    ),
                },
                None => write!(
                    f,
                    "{} limit ({}) exceeded",
                    resource.as_protocol_str(),
                    limit
                ),
            },
            AjisaiError::DeclaredCondition {
                condition,
                message,
                word: Some(word),
            } => {
                write!(f, "{}: {}", word, message)?;
                write_hint(f, usage_hint(word, condition, reported_domain(message)))
            }
            AjisaiError::DeclaredCondition { message, .. } => write!(f, "{}", message),
        }
    }
}

/// A diagnosis's one-line repair (`diagnostic_hint`), after a dash.
fn write_hint(f: &mut fmt::Formatter<'_>, hint: Option<String>) -> fmt::Result {
    match hint {
        Some(hint) => write!(f, " — {}", hint),
        None => Ok(()),
    }
}

impl std::error::Error for AjisaiError {}
