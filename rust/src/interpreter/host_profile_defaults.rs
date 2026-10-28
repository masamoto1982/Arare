//! The default resource ceilings a host gets if it does not inject its own
//! `RuntimeLimits` — i.e. the playground / native-CLI (`ajisai run`) profile.
//! Split out of `runtime_limits.rs` (the 500-line file budget in
//! docs/dev/specification-implementation-rules.md)
//! and re-exported from there (`pub use host_profile_defaults::*;`), so every
//! existing `runtime_limits::DEFAULT_MAX_*` path keeps resolving unchanged.
//!
//! `docs/dev/mcp-host-profiles.md` is the live comparison against the MCP
//! profile (`LOCAL_AGENT_RUNTIME_LIMITS`, `rust/src/agent/api.rs`); the two
//! disagree on purpose (LANG.MACHINE.LIMITS: limits are a host safety control,
//! not language semantics). It also records how the values below were derived
//! (or, for three of them, deliberately *not* derived — see each constant's own
//! doc comment).

use super::RuntimeLimits;
/// Default cap on elements a single generative built-in (`RANGE`, `FILL`,
/// `RESHAPE`, …) may materialize in one call. Each generated `Value` costs a few
/// hundred bytes, so one million elements bounds a call to a few hundred MiB
/// rather than a multi-gigabyte OOM abort.
///
/// Unlike [`DEFAULT_MAX_NUMERIC_WORK`] / [`DEFAULT_MAX_COLLECTION_WORK`] /
/// [`super::interpreter_core::DEFAULT_MAX_EXECUTION_STEPS`], this is **not**
/// derived from [`DEFAULT_HOST_TIME_BUDGET_MS`]: it bounds the size of one
/// value, not accumulated time, so a time budget has nothing to say about it.
/// The right basis would be something like "elements a browser can hold and a
/// human can still make sense of on screen" — nobody has measured that yet, so
/// this stays at its prior value. It was re-checked against the 2026-08-14
/// re-derivation of [`DEFAULT_MAX_COLLECTION_WORK`]: one linear pass over a
/// `DEFAULT_MAX_MATERIALIZED_ELEMENTS`-sized vector still costs well under 2%
/// of the new budget, so lowering the work budget did not strand this ceiling
/// out of reach (`profile_liveness_tests`).
pub const DEFAULT_MAX_MATERIALIZED_ELEMENTS: usize = 1_000_000;

/// Default cap on the byte length of a single source program handed to
/// `execute`, checked before tokenization allocates per-character buffers.
///
/// Was 64 MiB — deliberately generous, but by nobody's measurement: not a
/// program-size observation, just a round number that happened to be far above
/// the perf-benchmark's largest chain (~1.77 MB) without saying why *that*
/// margin was the right one. 16 MiB keeps the same generosity in kind — about
/// 9x the largest known legitimate machine-generated program, so nothing that
/// has ever actually been written against Ajisai gets close to it — while being
/// an amount someone could state a reason for instead of a bare "large enough".
/// Still a judgment call, not a measurement of what a human pastes into a
/// textarea; nobody has done that study either.
///
/// Memory-constrained hosts — **the WASM playground in particular**, which is
/// the host carrying this value and the one with the least headroom — should
/// inject a tighter `max_source_bytes` via `Interpreter::set_runtime_limits`;
/// that is exactly why the limit is a per-interpreter injectable field rather
/// than a global. (The MCP server already does, at 64 KiB.) That advice
/// predates the 2026-08-14 derivation and survives it: a byte ceiling is
/// about the allocation the tokenizer is about to make, which a time budget
/// has nothing to say about.
pub const DEFAULT_MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// Default cap on the digit count of a single numeric literal in source. A
/// 4096-digit integer is astronomically large for any legitimate program,
/// while the ceiling stops a megabyte-long literal from driving an expensive
/// BigInt parse (`Fraction::from_str`) before the value is ever built.
pub const DEFAULT_MAX_NUMERIC_LITERAL_DIGITS: usize = 4_096;

/// How long a host with nobody waiting on the other end of a call — a user in
/// their own tab or terminal, free to close it or hit abort at any point —
/// should let a computation run before answering with a named reason instead of
/// continuing silently. The single dial every ceiling below is derived from,
/// per `docs/dev/mcp-host-profiles.md`: not a value to tune per-ceiling, a
/// budget of *time* that gets turned into a budget of *units* once per meter,
/// at that meter's measured rate.
///
/// This is a **judgment call, not a measurement** — say so plainly, per that
/// document's §6 requirement, because nobody has usage data on how long an
/// Ajisai learner actually waits before assuming a computation is stuck
/// rather than genuinely large. What is known: the playground runs execution
/// off the main thread in a Worker (`src/workers/execution-worker-manager.ts`)
/// behind an explicit abort control
/// (`ExecutionController.abortExecution`), so the relevant threshold is not
/// "the tab looks frozen" — Nielsen/Miller's classic ~10s attention limit for
/// a UI with *no* feedback and no way out — but "how long a user who chose to
/// run something and can already see it running, and can already stop it, is
/// willing to wait before the ceiling should do it for them." 30 seconds is
/// picked as several multiples of that 10s baseline: long enough that a
/// program a curious learner deliberately made bigger doesn't hit a wall for
/// being merely ambitious (§2's rejected "align down to MCP" option is
/// rejected for exactly this reason — the teaching surface should stay wide),
/// short enough that it is still a real ceiling and not a number nobody will
/// ever reach. Re-derive this from real usage data if it ever becomes
/// available; until then, treat it as what it is.
pub const DEFAULT_HOST_TIME_BUDGET_MS: u64 = 30_000;

/// `numericWork`'s units/ms floor **on WASM**, this container, this build,
/// this session (Node's V8 against `tools/mcp-server/wasm/generated`, which
/// is the same compiled module the browser playground ships — see
/// `scripts/wasm-profile-calibration.mjs`) — the slowest measured rate among
/// the paths only `numericWork` bounds, the same role `dense tensor lanes`
/// plays in `rust/examples/collection_word_calibration.rs`.
///
/// **Not measured on native.** The first attempt at this derivation
/// (2026-08-14) calibrated all three floor rates on `cargo run --release`
/// and applied them to the WASM playground, which is where every one of
/// these ceilings actually runs. Measured back to back on one container, the
/// native→WASM ratio was 2.1x for this meter, 4.0x for `collectionWork` and
/// 0.66x for `executionSteps` — a ratio that swings from "twice as fast" to
/// "a third slower" depending on the meter, so a native calibration cannot
/// make three ceilings bound equal time on the host they are actually
/// applied to (`scripts/wasm-profile-calibration.mjs` and
/// `rust/examples/work_meter_calibration.rs` re-measure both sides).
/// `rust/examples/work_meter_calibration.rs` remains useful for its own
/// purpose — the native floor `ajisai run`/native `ajisai agent` actually
/// runs under, and the ratio work between `numericWork` and `collectionWork`
/// pricing does not depend on host — but it is not this constant's source.
///
/// `dense tensor lanes (100k) x80` was the floor across every run (10,373–
/// 15,591 units/ms observed; this container is shared/virtualized and
/// visibly noisier under WASM than under native, so the minimum — the safe
/// direction, since a lower rate means a *smaller* derived budget — is kept
/// rather than an average). **Container- and engine-specific: re-measure
/// with `scripts/wasm-profile-calibration.mjs` when deploying on different
/// hardware or a different browser engine.**
const NUMERIC_WORK_FLOOR_RATE_UNITS_PER_MS: u64 = 10_373;

/// `collectionWork`'s units/ms floor on WASM — measured the same way as
/// [`NUMERIC_WORK_FLOOR_RATE_UNITS_PER_MS`], by the same script.
///
/// The floor path here, `UNIQUE` over a vector of 4096-digit values, is
/// itself the reason this whole derivation was redone: it used to charge
/// ~470x less per millisecond than any other collection path (248 units/ms
/// against 73,491 for `REVERSE 100k`), a pricing hole in `Fraction::hash`'s
/// `BigInt::gcd` on wide operands with a narrow denominator (see the fix's
/// own commit), not a real bound — deriving a budget from it would have
/// meant shipping a ceiling one specific shape of value could blow through
/// 470x over. After the fix this path charges 47,162–73,457 units/ms across
/// runs, now genuinely the floor (below `REVERSE 100k`'s ~89,000–91,000) but
/// by a normal margin, not a hole. Minimum kept for the same reason as the
/// numeric floor. **Container- and engine-specific: re-measure with
/// `scripts/wasm-profile-calibration.mjs` on deployment hardware.**
const COLLECTION_WORK_FLOOR_RATE_UNITS_PER_MS: u64 = 47_162;

/// Default cap on accumulated internal numeric work units charged through the
/// work meter (algebraic products, reciprocal recursion, precision doubling).
///
/// Derived, not chosen: [`DEFAULT_HOST_TIME_BUDGET_MS`] ×
/// [`NUMERIC_WORK_FLOOR_RATE_UNITS_PER_MS`]. At the floor rate this spends
/// the full time budget; every other unbounded path is faster per unit and
/// so exhausts this budget in *less* than [`DEFAULT_HOST_TIME_BUDGET_MS`],
/// never more — the same safety direction
/// `LOCAL_AGENT_RUNTIME_LIMITS.max_numeric_work` uses for the MCP profile.
///
/// This number moved twice on 2026-08-14, for two different reasons, and both
/// are worth keeping straight. First, from the pre-derivation 1,000,000,000
/// (never itself derived — "a post-hoc interpretation, not a derived value")
/// down to 139,800,000, calibrated on **native** release. Then, once review
/// found native was not a valid proxy for the WASM engine every ceiling here
/// actually governs, up to the value computed here — WASM's `numericWork` floor
/// measured faster than native's on this container. Neither number was wrong to
/// compute; the second calibration was wrong to apply to the playground.
/// Re-measure with `scripts/wasm-profile-calibration.mjs` on a different
/// deployment target or browser engine rather than adjusting this by hand.
pub const DEFAULT_MAX_NUMERIC_WORK: u64 =
    DEFAULT_HOST_TIME_BUDGET_MS * NUMERIC_WORK_FLOOR_RATE_UNITS_PER_MS;

/// `executionSteps`' steps/ms floor on WASM — the same idea as
/// [`NUMERIC_WORK_FLOOR_RATE_UNITS_PER_MS`] above, but for word dispatch
/// rather than operand size. `executionSteps` prices raw word count and
/// cannot see what a word does internally (the other two meters already
/// price that), so the only path it alone has to bound is a long loop of the
/// *cheapest* word — the dispatch-bound analogue of `dense tensor lanes`.
///
/// Measured by `scripts/wasm-profile-calibration.mjs`, which tries two
/// shapes and keeps the slower: a flat loop of machine-word additions (785
/// steps/ms observed) and a loop of user-word calls, which is dearer because
/// every iteration pays a dictionary lookup and a frame push, not just an
/// arithmetic op (406–419 steps/ms observed). The dearer shape was a
/// self-calling trampoline when those figures were taken; a self-call is no
/// longer definable (LANG.DICTIONARY.ACYCLIC), so the script now pays the
/// same per-iteration cost through a `MAP` and the figures stand until
/// re-measured. The trampoline sets
/// the floor, same as the numeric and collection meters: minimum of several
/// runs kept for the same conservative reason. **Container- and
/// engine-specific: re-measure with the same script on deployment
/// hardware.**
const EXECUTION_STEPS_FLOOR_RATE_STEPS_PER_MS: u64 = 406;

/// Default cap on words executed in one `execute` call — `executionSteps`.
/// [`super::interpreter_core::DEFAULT_MAX_EXECUTION_STEPS`] re-exports this
/// value.
///
/// Derived the same way as the two work budgets:
/// [`DEFAULT_HOST_TIME_BUDGET_MS`] ×
/// [`EXECUTION_STEPS_FLOOR_RATE_STEPS_PER_MS`]. Previously 100,000, shared
/// verbatim with the MCP profile's own `executionSteps` — not because the two
/// hosts were declared to agree, but because nobody had threaded a
/// playground-specific value through ("a long loop of cheap Words hits the same
/// wall on its own tab as it does through MCP"). The MCP profile still passes
/// its own `executionSteps EQ 100_000` explicitly at every call site
/// (`tools/mcp-server/index.js` `LIMITS.executionSteps`, threaded through
/// `--step-limit` / `stepLimit`), so raising this default only widens the
/// playground and native-CLI budget; MCP's is untouched.
///
/// Like the two work budgets, this was first calibrated on native and moved
/// again once that was found not to be a valid proxy for WASM (see
/// [`DEFAULT_MAX_NUMERIC_WORK`]'s doc comment) — down, this time: WASM's
/// dispatch floor measured slower than native's on this container.
///
/// A budget this large is not something a fast test should try to exhaust by
/// actually running it — unlike the two work meters, which a single wide
/// operand can reach in microseconds, reaching a step *count* costs real wall
/// time roughly proportional to dispatch speed, by construction. Tests that
/// need to observe `ExecutionLimitExceeded` inject a small limit instead
/// (`Interpreter::set_max_execution_steps`), the same pattern already used
/// throughout `collection_meter_tests.rs` and `arithmetic_meter_tests.rs` for
/// the same reason.
pub const DEFAULT_MAX_EXECUTION_STEPS: usize =
    (DEFAULT_HOST_TIME_BUDGET_MS * EXECUTION_STEPS_FLOOR_RATE_STEPS_PER_MS) as usize;

/// Default cap on accumulated collection work units.
///
/// Derived the same way as [`DEFAULT_MAX_NUMERIC_WORK`]:
/// [`DEFAULT_HOST_TIME_BUDGET_MS`] ×
/// [`COLLECTION_WORK_FLOOR_RATE_UNITS_PER_MS`]. **No longer twice the numeric
/// budget**, and not a fixed ratio to it at all. It used to be defined as
/// `2 MUL DEFAULT_MAX_NUMERIC_WORK`, a relationship that was itself derived
/// once — the two floor rates measured 14,465 and 30,800 units/ms on the
/// reference container (`rust/examples/collection_word_calibration.rs`), a
/// ratio of ~2.1 that the `2×` constant captured — but a ratio between
/// two independently-measured rates is not a rule, and it moved: the
/// 2026-08-14 de-quadraticization follow-up predicted it might once the scan
/// family stopped being a linear scan, and the same day's `Fraction::hash`
/// fix moved it again. Each budget is derived independently from its own
/// measured floor; they only need to agree on the *time* they bound, which
/// they do by construction (both use the same [`DEFAULT_HOST_TIME_BUDGET_MS`]).
pub const DEFAULT_MAX_COLLECTION_WORK: u64 =
    DEFAULT_HOST_TIME_BUDGET_MS * COLLECTION_WORK_FLOOR_RATE_UNITS_PER_MS;

/// Default cap on the bit length of a BigInt arithmetic result. ~300k decimal
/// digits — generous for exact rationals, but bounded so a doubling cascade
/// cannot blow up to gigabytes.
///
/// Like [`DEFAULT_MAX_MATERIALIZED_ELEMENTS`], this bounds the size of one
/// value rather than accumulated time, so it is not derived from
/// [`DEFAULT_HOST_TIME_BUDGET_MS`] the way the two work budgets are — but it
/// still has to stay *reachable* inside [`DEFAULT_MAX_NUMERIC_WORK`], or it is
/// a claim rather than a control (`profile_liveness_tests`, and the account of
/// exactly this failure mode in `docs/dev/mcp-host-profiles.md`'s "what each
/// MCP-declared limit is pinned by" section). Re-checked against the
/// WASM-calibrated `DEFAULT_MAX_NUMERIC_WORK`: the widening chain that first
/// crosses 1,000,000 bits (74 repeated multiplications by a 4096-digit
/// constant) charges 122,321,640 units — a fixed cost, deterministic from the
/// pricing formula and independent of which host measured the floor rate —
/// which is ~39% of the current 311,190,000 budget (was ~87% under the
/// native-calibrated 139,800,000 this replaced; still comparable to the margin
/// `bigintBits` runs at under the MCP profile, 86%, because an N-limb integer
/// costs about N²/4 limb-operations by construction and the two ceilings are
/// the closest pair for exactly that reason). Left unchanged because it still
/// fires by name with the numeric-work ceiling as backstop, not because a
/// size-based criterion for it has been written down: it is honestly
/// unmeasured.
pub const DEFAULT_MAX_BIGINT_BITS: u64 = 1_000_000;

/// Default cap on the number of algebraic terms a single algebraic (normal-form)
/// value may carry.
///
/// Lowered from 100,000 on 2026-08-14. That value predates
/// [`DEFAULT_MAX_NUMERIC_WORK`]'s re-derivation and stopped being live once the
/// numeric-work budget shrank to match this container's measured floor rate:
/// the doubling cascade that first exceeds 100,000 terms (131,072 terms, at
/// factor 17) charges 520,099,280 units — over the 311,190,000 budget this
/// constant is checked against now, and further still over the 139,800,000 the
/// native-calibrated first attempt would have shipped — so `numericWork` would
/// always answer first and the term ceiling would never fire in its life,
/// exactly the failure `profile_liveness_tests` exists to catch (see the
/// module-level docs on that file, and the identical MCP-side history:
/// `max_algebraic_terms` was 4,096 there for the same reason before being
/// lowered to 512). 10,000 is chosen the same way 512 was for the MCP profile:
/// the cascade that first crosses it (16,384 terms, at factor 14) charges
/// 50,335,424 units — a fixed, host-independent cost — ~16% of the current work
/// budget (was ~36% under the smaller native-calibrated one; either way
/// comfortably reachable with room to spare, not cut close the way `bigintBits`
/// is). Left at 10,000 rather than raised back toward 100,000 now that the
/// budget has more room: the value was never meant to track the budget's exact
/// size, only to stay live under it, and chasing a bigger number each time the
/// budget moves is churn without a principle behind it. Like
/// `DEFAULT_MAX_BIGINT_BITS`, this is not itself derived from a stated
/// size-legibility criterion; it is chosen to be *live*, which is a weaker and
/// more urgent property than being *right*.
pub const DEFAULT_MAX_ALGEBRAIC_TERMS: usize = 10_000;

/// Default cap on how many containers deep one value may nest
/// (`Value::nesting`), however it came to be: a literal, a decoded JSON text,
/// or a value Words built. Every walk over a value — comparing, rendering,
/// hashing, encoding, broadcasting, and the recursive `Drop` of a nested
/// `Arc<Vec<Value>>` — descends one native frame per level, so a value a few
/// thousand levels deep overflowed the native stack and aborted the process
/// (an unrecoverable trap in the WASM playground) instead of failing with a
/// diagnosable error.
///
/// Not derived from the time budget: it bounds the shape of one value, not
/// accumulated work. It matches `MAX_CALL_DEPTH`, which is vetted
/// against the same WASM stack envelope — one level of a value walk is
/// lighter than a User Word call, which expands to several native frames — and
/// is ~20x the deepest hand-written nesting in the corpus. The literal ceiling
/// was already this figure; the JSON decoder's was 512, and a program could
/// build past either with no ceiling at all.
pub const DEFAULT_MAX_NESTING_DEPTH: usize = 256;

// The one place the resource-ceiling set is enumerated, for the two hosts
// that publish it (`wasm_interpreter_bindings::agent_host_profile` and the
// agent profile): `entries` destructures `RuntimeLimits` exhaustively, so a
// new ceiling stops this file compiling until it is named and placed.
/// The number of ceilings in a limit profile: `RuntimeLimits`' own fields plus
/// the execution-step budget, which lives beside rather than inside it (see
/// `RuntimeLimits`' doc for why), and the call-depth ceiling, which the engine
/// fixes for every host (`interpreter_core::MAX_CALL_DEPTH`).
pub(crate) const LIMIT_PROFILE_CEILINGS: usize = 10;

/// Every ceiling of one run as `(published name, value)`, under the wire
/// names every host publishes.
pub(crate) fn entries(
    limits: &RuntimeLimits,
    step_limit: usize,
) -> [(&'static str, u64); LIMIT_PROFILE_CEILINGS] {
    // Exhaustive on purpose — see this module's doc. Do not replace with
    // field access, and do not add `..`.
    let RuntimeLimits {
        max_materialized_elements,
        max_source_bytes,
        max_numeric_literal_digits,
        max_numeric_work,
        max_collection_work,
        max_bigint_bits,
        max_algebraic_terms,
        max_nesting_depth,
    } = *limits;
    [
        ("executionSteps", step_limit as u64),
        ("materializedElements", max_materialized_elements as u64),
        ("sourceBytes", max_source_bytes as u64),
        ("numericLiteralDigits", max_numeric_literal_digits as u64),
        ("numericWork", max_numeric_work),
        ("collectionWork", max_collection_work),
        ("bigintBits", max_bigint_bits),
        ("algebraicTerms", max_algebraic_terms as u64),
        ("nestingDepth", max_nesting_depth as u64),
        ("callDepth", super::interpreter_core::MAX_CALL_DEPTH as u64),
    ]
}

/// The ceilings as a JSON object, under the wire names of [`entries`].
pub(crate) fn to_json(limits: &RuntimeLimits, step_limit: usize) -> serde_json::Value {
    serde_json::Value::Object(
        entries(limits, step_limit)
            .into_iter()
            .map(|(name, value)| (name.to_string(), serde_json::json!(value)))
            .collect(),
    )
}

#[cfg(test)]
#[path = "limit_profile_tests.rs"]
mod limit_profile_tests;
