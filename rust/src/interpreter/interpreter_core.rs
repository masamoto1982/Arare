use crate::error::Result;
use crate::types::{Name, Stack, Value, WordDefinition};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// How many User Word lookups `word_cache` keeps.
pub(crate) const WORD_CACHE: usize = 8;

pub(crate) type WordCacheEntry = (u64, Name, Option<Arc<WordDefinition>>);

/// Re-exports `runtime_limits::DEFAULT_MAX_EXECUTION_STEPS` (see its doc
/// comment for the host-time-budget derivation).
pub const DEFAULT_MAX_EXECUTION_STEPS: usize = super::runtime_limits::DEFAULT_MAX_EXECUTION_STEPS;

/// The `callDepth` ceiling: how many User Word calls and block evaluations may
/// be active at once. Hit before Rust's native call stack
/// runs out and panics the WASM module to an unrecoverable trap. The value is
/// set well above any reasonable hand-written nesting (the existing deep
/// non-recursive test uses 5 frames) but well below the Rust stack budget on
/// WASM and on the 2 MiB default native test-thread stack — every user-word
/// recursion expands to several Rust frames (`call_user_word` → `run_code`
/// → `run_op_slow` → `dispatch_word_beyond_core`), so the empirical
/// safe ceiling is roughly 256 levels in debug builds. `DEFAULT_MAX_EXECUTION_STEPS`
/// is still the primary backstop for runaway computation generally; this
/// guard turns deep nesting into `resourceLimitExceeded` naming `callDepth`
/// regardless of that step budget's value — a stack overflow is a depth
/// problem, not a count problem. What reaches it is a long acyclic call
/// chain, with blocks nested inside it; a block that runs a copy of itself
/// is refused long before, as `nestedDataBlock` (`in_block_frame`,
/// LANG.SOURCE.CODE), so the guard bounds depth and never decides whether a
/// run ends. A Word that
/// evaluates a block nests the same frames, so it counts one level too
/// (`in_block_frame`): the depth is that of calls and blocks together. Fixed
/// by the engine rather than by a host profile, and published in every
/// profile as `callDepth` (`host_profile_defaults::entries`).
pub const MAX_CALL_DEPTH: usize = 256;

/// What this run spent of the budgets that can refuse it.
///
/// Every field here is the *same* value the corresponding ceiling compares
/// against — read from the counter the check reads, not from a parallel copy —
/// and every field's name is a key of the host's declared limit profile. An
/// agent that plans against `mcp.limits` can therefore subtract.
///
/// Deliberately only the two that exist. `bigintBits` and `algebraicTerms` are
/// checked per result and never accumulated, so there is no peak to report;
/// inventing fields for them would mean reporting a number nothing measured,
/// which is the defect this type was introduced to fix.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResourceUsage {
    /// Words executed, against `executionSteps`.
    pub execution_steps: u64,
    /// Internal arithmetic work charged, against `numericWork`.
    pub numeric_work: u64,
    /// Element operations charged inside collection Words, against
    /// `collectionWork`.
    pub collection_work: u64,
    /// The most algebraic terms any one result carried, against
    /// `algebraicTerms`: the largest number that per-result ceiling compared.
    /// 0 when no result was an irrational, and then not reported.
    pub algebraic_terms: u64,
}

pub struct Interpreter {
    pub(crate) stack: Stack,
    pub(crate) core_vocabulary: crate::fast_hash::FastMap<String, Arc<WordDefinition>>,
    pub(crate) user_words: crate::fast_hash::FastMap<String, Arc<WordDefinition>>,
    pub(crate) dependents: HashMap<String, HashSet<String>>,
    pub(crate) output_buffer: String,
    /// Structured, ordered host effects produced during execution. This is the
    /// language-independent observation channel for the conformance suite
    /// (`tests/conformance/`): two implementations agree iff they emit the same
    /// effect sequence. The legacy `output_buffer` string protocol is still emitted in
    /// parallel so existing front-ends keep working.
    pub(crate) host_effects: Vec<super::HostEffect>,
    /// Host boundary for clocks, entropy, capability checks, and effect sinks.
    /// Core execution must not call platform APIs directly; Hosted words route
    /// boundary access through this trait object so conformance can inject a
    /// deterministic or restricted host.
    pub(crate) host_env: Arc<dyn super::HostEnv>,
    /// Local name bindings, innermost frame last (`super::bindings`): the
    /// first `binding_depth` are the open frames, and the rest are emptied
    /// tables kept for the next frames to open. Not a second dictionary:
    /// each scope belongs to one frame of LANG.SOURCE.FRAME and dies with
    /// it, so a name here is never reachable from another Word.
    pub(crate) binding_scopes: Vec<super::bindings::BindingScope>,
    pub(crate) binding_depth: usize,
    /// The slots of every open frame's locals (`BindingScope::slots`).
    pub(crate) locals: Vec<Option<Value>>,
    /// The dictionary changes this top-level `execute` has made so far, in
    /// order — `Defined word:` / `Deleted word:` without the prose.
    ///
    /// A host that treats a failed run as committing nothing needs to say so:
    /// the run still printed a success line for every `DEF` it reached, and
    /// leaving those standing while discarding the Words is a log that claims
    /// something the session does not contain. The tester who found this lost
    /// seven definitions and only noticed when `LOOKUP` answered
    /// `Unknown word`. Cleared at the start of each top-level run.
    pub(crate) dictionary_changes_this_run: Vec<String>,
    pub(crate) disable_no_change_check: bool,
    /// User-word call depth. Incremented on entry to a user-word body in
    /// `call_user_word`, and around a Word that evaluates a block
    /// (`in_block_frame`), decremented on exit. Compared against
    /// `MAX_CALL_DEPTH` to prevent deep nesting from blowing the
    /// Rust call stack and trapping the WASM module.
    pub(crate) call_depth: usize,
    /// The block literals each running code frame has pushed so far, flat,
    /// with where each frame's own begin (`code_literal_bases`). A block a
    /// Word runs is written in the code running it when it equals one of the
    /// current frame's, or a block inside one (`engine.rs`,
    /// LANG.DICTIONARY.ACYCLIC).
    pub(crate) code_literals: Vec<Value>,
    pub(crate) code_literal_bases: Vec<usize>,
    /// Whether a block built from data — not written in the code that runs
    /// it — is running now. At most one may be (LANG.DICTIONARY.ACYCLIC).
    pub(crate) data_block_running: bool,
    pub(crate) execution_step_count: usize,
    pub(crate) max_execution_steps: usize,
    /// Unified internal-computation-cost ceilings (CS5). The step budget above
    /// prices word count; these price the per-word work it cannot see
    /// (materialization, source/literal size, and — via the work meter —
    /// algebraic/BigInt blow-up). Child runtimes inherit a copy.
    pub(crate) runtime_limits: super::runtime_limits::RuntimeLimits,
    /// Cumulative internal numeric work charged this `execute` (CS5 work meter).
    /// Charged before each expensive exact-arithmetic operation so a runaway
    /// algebraic computation fails at `runtime_limits.max_numeric_work` rather
    /// than running for minutes. Reset per top-level `execute`.
    pub(crate) numeric_work_used: u64,
    /// Cumulative collection work charged this `execute`. A collection Word
    /// loops inside Rust and costs one execution step however many elements it
    /// touches, so this is the only counter that can see it. Reset per
    /// top-level `execute`, like the numeric meter beside it.
    pub(crate) collection_work_used: u64,
    /// The largest algebraic term count a result reached this `execute`, as
    /// `check_result_size` and `POW`'s size check measured it. A `Cell`
    /// because those checks read the interpreter immutably. Reset per
    /// top-level `execute`.
    pub(crate) algebraic_terms_peak: std::cell::Cell<u64>,

    pub(crate) next_registration_order: u64,

    pub(crate) dictionary_epoch: u64,

    /// The Word this run's failure is attributed to (`failure_site`).
    pub(crate) failed_word: Option<super::failure_site::FailedWord>,
    /// The last User Word lookups by name, each with the dictionary epoch it
    /// was made at (`Interpreter::user_word`).
    pub(crate) word_cache: [WordCacheEntry; WORD_CACHE],

    /// Content identity of each user word, keyed by name. Derived state:
    /// recomputed whenever the user-word graph changes.
    pub(crate) word_identities: HashMap<String, String>,

    /// Content store for definition bodies, keyed by content key. Textually
    /// identical bodies share a single `Arc<[Token]>`, so restoring or copying
    /// a word group does not duplicate its code in memory.
    pub(crate) body_store: HashMap<String, std::sync::Arc<[crate::types::Token]>>,

    /// When set, `recompute_word_identities` is a no-op. Bulk operations (e.g.
    /// restoring or importing many words) set this for the duration of the
    /// batch and recompute once at the end, avoiding O(N^2) identity hashing.
    pub(crate) defer_identity_recompute: bool,

    /// A `DEF` body's own written tokens, captured lexically when a literal
    /// precedes `<name> DEF` (`engine.rs`, `execute_def.rs`).
    pub(crate) pending_def_body_tokens: Option<Vec<crate::types::Token>>,

    /// `#:contract NAME ...` directive text scanned out of the raw source at
    /// `execute()` time, keyed by the upper-cased word name. `#:contract` is
    /// tooling-only — an ordinary comment to the interpreter (SPEC), stripped
    /// by the tokenizer before any Word ever sees it — so this is the one
    /// place its text still exists once the Word it names actually runs
    /// `DEF`. `op_def` consumes an entry here as that Word's `description`, a
    /// host affordance (the Dictionary panel's hover) rather than a language
    /// effect: it changes nothing about what the program computes.
    pub(crate) pending_word_descriptions: std::collections::HashMap<String, String>,
    // ── Source positions ──────────────────────────────────────────────────
    /// Where each token of the program currently running was written,
    /// index-aligned with its token stream. Empty for any entry point that did
    /// not come from source text.
    pub(crate) source_spans: Vec<crate::tokenizer::SourceSpan>,
    /// The top-level token a failure was reached from (`engine::run_program`),
    /// which is what turns "Stack underflow" into "Stack underflow at line 4,
    /// column 12".
    pub(crate) current_source_span: Option<crate::tokenizer::SourceSpan>,

    /// When true (default), StackTop scalar-scalar arithmetic and comparison can
    /// bypass the tensor broadcast wrapper for bare scalars and same-shape
    /// singleton tensor/vector wrappers. Disable via
    /// `AJISAI_NO_SCALAR_FASTPATH` for A/B measurement.
    pub(crate) scalar_fastpath_enabled: bool,

    /// When true (default), a `MAP`/`FOLD`/`SCAN` block of straight-line
    /// rational arithmetic runs fused (`fused_block`), with the interpreted
    /// walk's charges. Disable via `AJISAI_NO_FUSED_BLOCK` for A/B comparison.
    pub(crate) fused_block_enabled: bool,

    /// When true (default), element-wise arithmetic, `LT`/`GT` and
    /// `FLOOR`/`ROUND` on one-dimensional dense Tensors run on the columns
    /// (`dense_kernels`). Disable via `AJISAI_NO_DENSE_KERNELS` for A/B
    /// comparison.
    pub(crate) dense_kernels_enabled: bool,
    /// Whether a truth column may be on the stack (`truth_columns`).
    pub(crate) packed_truths: std::cell::Cell<bool>,
    /// Compiled scalar calls answered without the dispatch (`quickened`).
    pub(crate) quickening_enabled: bool,
    /// Code operands compiled recently (`higher_order::BlockCache`).
    pub(crate) block_cache: super::higher_order::BlockCache,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self::with_host(super::default_host_env())
    }

    pub fn with_host(host_env: Arc<dyn super::HostEnv>) -> Self {
        let mut interpreter = Interpreter {
            stack: Stack::new(),
            core_vocabulary: Default::default(),
            user_words: Default::default(),
            dependents: HashMap::new(),
            output_buffer: String::new(),
            host_effects: Vec::new(),
            host_env,
            binding_scopes: vec![super::bindings::BindingScope::root()],
            binding_depth: 1,
            locals: Vec::new(),
            dictionary_changes_this_run: Vec::new(),
            disable_no_change_check: true,
            call_depth: 0,
            code_literals: Vec::new(),
            code_literal_bases: Vec::new(),
            data_block_running: false,
            execution_step_count: 0,
            max_execution_steps: DEFAULT_MAX_EXECUTION_STEPS,
            runtime_limits: super::runtime_limits::RuntimeLimits::default(),
            numeric_work_used: 0,
            collection_work_used: 0,
            algebraic_terms_peak: std::cell::Cell::new(0),
            next_registration_order: 1,
            dictionary_epoch: 0,
            failed_word: None,
            word_cache: std::array::from_fn(|_| (u64::MAX, Name::new(""), None)),

            // Elastic Engine
            word_identities: HashMap::new(),
            body_store: HashMap::new(),
            defer_identity_recompute: false,
            pending_def_body_tokens: None,
            pending_word_descriptions: HashMap::new(),
            source_spans: Vec::new(),
            current_source_span: None,
            scalar_fastpath_enabled: std::env::var("AJISAI_NO_SCALAR_FASTPATH").is_err(),
            fused_block_enabled: std::env::var("AJISAI_NO_FUSED_BLOCK").is_err(),
            dense_kernels_enabled: std::env::var("AJISAI_NO_DENSE_KERNELS").is_err(),
            packed_truths: Default::default(),
            quickening_enabled: std::env::var("AJISAI_NO_QUICKEN").is_err(),
            block_cache: Default::default(),
        };
        crate::builtins::register_builtins(&mut interpreter.core_vocabulary);
        interpreter
    }

    pub(crate) fn bump_dictionary_epoch(&mut self) {
        self.dictionary_epoch += 1;
    }

    /// One Word dispatch against the execution-step ceiling.
    ///
    /// The step is what a Word *costs*, not what any one route happens to count,
    /// so every route that dispatches a Word calls this.
    pub(crate) fn charge_execution_step(&mut self) -> crate::error::Result<()> {
        self.execution_step_count += 1;
        if self.execution_step_count > self.max_execution_steps {
            return Err(crate::error::AjisaiError::ExecutionLimitExceeded {
                limit: self.max_execution_steps,
            });
        }
        Ok(())
    }

    /// Hold every value written to the stack since the last check to the
    /// nesting ceiling (LANG.MACHINE.LIMITS).
    ///
    /// Run after each Word, so a Word that builds a value too deep fails as
    /// itself, and once more when a run ends, for the literals pushed after
    /// the last Word. A literal is bounded by its own builder and a Word adds
    /// only a few levels to operands that already passed, so nothing between
    /// two checks can grow deep enough to overflow a walk. Each value's
    /// nesting is kept on the value, and only the slots written since the last
    /// check are read, so the check costs what the Word wrote.
    #[inline]
    pub(crate) fn check_fresh_nesting(&mut self) -> crate::error::Result<()> {
        let start = self.stack.take_fresh_start();
        let deepest = self.stack.as_slice()[start..]
            .iter()
            .map(Value::nesting)
            .max()
            .unwrap_or(0);
        self.runtime_limits.check_nesting_depth(deepest as usize)
    }

    pub(crate) fn normalize_symbol<'a>(symbol: &'a str) -> std::borrow::Cow<'a, str> {
        if symbol.as_bytes().iter().any(|b| b.is_ascii_lowercase()) {
            std::borrow::Cow::Owned(symbol.to_uppercase())
        } else {
            std::borrow::Cow::Borrowed(symbol)
        }
    }

    pub(crate) fn next_registration_order(&mut self) -> u64 {
        let order = self.next_registration_order;
        self.next_registration_order += 1;
        order
    }

    pub fn collect_output(&mut self) -> String {
        std::mem::take(&mut self.output_buffer)
    }

    /// The ordered sequence of structured host effects produced so far. This is
    /// the language-independent observation channel used by the conformance
    /// suite, distinct from the human-readable `output_buffer`.
    pub fn host_effects(&self) -> &[super::HostEffect] {
        &self.host_effects
    }

    /// Take the structured host effects produced so far, leaving the log
    /// empty. A host that holds one interpreter across many runs (the
    /// Playground's WASM instance) reads each run's output once, and without
    /// draining, every `PRINT` payload of the session stayed in memory for as
    /// long as the interpreter lived. `execute` itself never clears the log,
    /// because the REPL reads it across lines by index (`cli::repl`).
    pub fn take_host_effects(&mut self) -> Vec<super::HostEffect> {
        std::mem::take(&mut self.host_effects)
    }

    pub(crate) fn emit_host_effect(&mut self, effect: super::HostEffect) {
        self.host_env.emit_effect(&effect);
        self.host_effects.push(effect);
    }

    /// Effect schema: request construction → effect append.
    ///
    /// Output is the only effect that reaches a host (LANG.EFFECTS.OUTPUT), so
    /// there is no capability to gate on: the builder constructs the structured payload
    /// (and may update the legacy output channel kept for adapters), and the
    /// resulting `HostEffect` is appended to the effect log in request order.
    pub(crate) fn run_effect_schema<F>(&mut self, build_effect: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<super::HostEffect>,
    {
        let effect = build_effect(self)?;
        self.emit_host_effect(effect);
        Ok(())
    }

    pub fn get_stack(&self) -> &Stack {
        &self.stack
    }

    /// The Words the current top-level run has defined or deleted, in order.
    /// A host that discards a failed run's state reports these so its own
    /// success lines are corrected rather than left standing.
    pub fn dictionary_changes_this_run(&self) -> &[String] {
        &self.dictionary_changes_this_run
    }
    /// Where in the source the interpreter is, as of the last top-level token
    /// it began. `None` for an entry point that did not come from source text.
    pub fn current_source_position(&self) -> Option<crate::tokenizer::SourceSpan> {
        self.current_source_span
    }

    /// Enable or disable the D1 scalar-scalar arithmetic/comparison fast path.
    /// In-process equivalent of `AJISAI_NO_SCALAR_FASTPATH`; it affects
    /// subsequent primitive executions immediately.
    pub fn set_scalar_fastpath_enabled(&mut self, enabled: bool) {
        self.scalar_fastpath_enabled = enabled;
    }

    /// Enable or disable fused higher-order blocks. In-process equivalent of
    /// `AJISAI_NO_FUSED_BLOCK`.
    pub fn set_fused_block_enabled(&mut self, enabled: bool) {
        self.fused_block_enabled = enabled;
    }

    /// Enable or disable quickened scalar calls. In-process equivalent of
    /// `AJISAI_NO_QUICKEN`.
    pub fn set_quickening_enabled(&mut self, enabled: bool) {
        self.quickening_enabled = enabled;
    }

    /// Enable or disable the dense column kernels. In-process equivalent of
    /// `AJISAI_NO_DENSE_KERNELS`.
    pub fn set_dense_kernels_enabled(&mut self, enabled: bool) {
        self.dense_kernels_enabled = enabled;
    }

    /// Override the execution step budget (water level). Raising it lets a
    /// benchmark drive a tail-recursive loop far past the default
    /// `DEFAULT_MAX_EXECUTION_STEPS` to observe O(1)-native-stack iteration.
    /// The step budget in force. A host that publishes its limit profile has
    /// to be able to read the value it is actually running under, not restate
    /// the default and hope no one changed it.
    pub fn max_execution_steps(&self) -> usize {
        self.max_execution_steps
    }

    pub fn set_max_execution_steps(&mut self, steps: usize) {
        self.max_execution_steps = steps;
    }

    /// The unified internal-computation-cost ceilings (CS5) in force.
    pub fn runtime_limits(&self) -> &super::runtime_limits::RuntimeLimits {
        &self.runtime_limits
    }

    /// Internal numeric work charged by this `execute`, in limb-multiply units.
    ///
    /// The meter's price is only meaningful if the rate it charges at can be
    /// *measured* rather than asserted, and a rate needs both halves: the units
    /// and the wall clock. `examples/work_meter_calibration.rs` reads this one.
    pub fn numeric_work_used(&self) -> u64 {
        self.numeric_work_used
    }

    /// Collection work charged by this `execute`, in element-operation units.
    ///
    /// Read by `examples/collection_word_calibration.rs` for the same reason
    /// the numeric counter is read by its own calibration: a price is only
    /// meaningful against a measured rate.
    pub fn collection_work_used(&self) -> u64 {
        self.collection_work_used
    }

    /// What this run spent of the budgets that can refuse it.
    ///
    /// Read from the counters the ceilings themselves read. There is no second
    /// copy to fall out of step, because reporting a resource and enforcing it
    /// are two readings of one number — which is the whole content of this
    /// method, and the reason it exists.
    pub fn resource_usage(&self) -> ResourceUsage {
        ResourceUsage {
            execution_steps: self.execution_step_count as u64,
            numeric_work: self.numeric_work_used,
            collection_work: self.collection_work_used,
            algebraic_terms: self.algebraic_terms_peak.get(),
        }
    }

    /// Override the internal-computation-cost ceilings. Used by tests to inject
    /// small limits that fire a guard without allocating anything huge, and by
    /// hosts that need a tighter or looser envelope. Child runtimes spawned
    /// afterwards inherit the new limits.
    /// Record a result's algebraic term count toward the run's peak.
    pub(crate) fn note_algebraic_terms(&self, terms: usize) {
        let terms = terms as u64;
        if terms > self.algebraic_terms_peak.get() {
            self.algebraic_terms_peak.set(terms);
        }
    }

    pub fn set_runtime_limits(&mut self, limits: super::runtime_limits::RuntimeLimits) {
        self.runtime_limits = limits;
    }

    /// Charge `units` of internal numeric work to the CS5 work meter and fail
    /// (diagnosably, before the expensive computation runs) if the cumulative
    /// total crosses `runtime_limits.max_numeric_work`. Saturating so the
    /// counter itself can never overflow. Reports itself by name through
    /// `ResourceLimitExceeded`, so an agent can tell an exhausted work meter
    /// from an exhausted step budget without reading the message.
    pub(crate) fn charge_numeric_work(&mut self, units: u64) -> Result<()> {
        self.numeric_work_used = self.numeric_work_used.saturating_add(units);
        if self.numeric_work_used > self.runtime_limits.max_numeric_work {
            return Err(crate::error::AjisaiError::ResourceLimitExceeded {
                resource: crate::error::ResourceLimit::NumericWork,
                limit: self.runtime_limits.max_numeric_work,
                observed: Some(self.numeric_work_used),
                progress: None,
            });
        }
        Ok(())
    }

    pub fn update_stack(&mut self, stack: impl Into<Stack>) {
        self.stack = stack.into();
    }
}
