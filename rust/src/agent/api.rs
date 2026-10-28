//! Typed, source-only boundary for agent hosts.
//!
//! Host adapters should consume this API instead of reproducing interpreter
//! execution and report assembly. It performs no filesystem or terminal I/O.

use super::report::{completed_run_report, ErrorInfo, Report};
use super::{contract, error_report, print_payloads, resolve_words};
use crate::error::ErrorCategory;
use crate::interpreter::runtime_limits::DEFAULT_MAX_NESTING_DEPTH;
use crate::interpreter::{Interpreter, RuntimeLimits};

/// Tighter internal-cost profile for untrusted, agent-generated programs.
pub const LOCAL_AGENT_RUNTIME_LIMITS: RuntimeLimits = RuntimeLimits {
    max_materialized_elements: 100_000,
    max_source_bytes: 64 * 1024,
    max_numeric_literal_digits: 4_096,
    max_numeric_work: 10_000_000,
    // Twice the numeric budget, which is what makes the two bound the same
    // amount of *time* rather than the same number of units: the numeric
    // meter's slowest unbounded path charges 14,465 units/ms and the collection
    // meter's charges 30,800, so 10M and 20M both buy about 0.7 s. Their sum
    // leaves `wallTimeMs` 5,000 a 3.7x margin.
    max_collection_work: 20_000_000,
    max_bigint_bits: 262_144,
    // Not a round number: 512 terms is 3.0% of `responseBytes` in `exactTerms`
    // (4,096 was 26.5% — a quarter of the whole response for one value), and
    // sixteen doublings past the point where the continued fraction stops being
    // readable at all. It is also *live*: the doubling that crosses it charges
    // 2,113,536 units, a fifth of `max_numeric_work`, so this ceiling names
    // itself instead of being pre-empted. At 4,096 it could not — the doubling
    // that would first exceed it costs 16,780,400 against a 10,000,000 budget,
    // so `numericWork` always answered first and this limit was a claim rather
    // than a control. See `profile_liveness_tests`.
    max_algebraic_terms: 512,
    max_nesting_depth: DEFAULT_MAX_NESTING_DEPTH,
};

/// The execution-step budget of the agent profile: the one the MCP server
/// applies (`tools/mcp-server/index.js`, `LIMITS.executionSteps`, which
/// threads it explicitly on every call) and the one `ajisai agent compute`
/// runs under unless `--step-limit` says otherwise. `docs/dev/mcp-host-profiles.md`
/// compares it with the playground's derived budget, which is 120 times
/// larger: a block iteration is one step per element, so under this profile
/// MAP / FILTER / FOLD walk tens of thousands of elements and a vector
/// operation (`V V ADD`) handles the rest.
pub const LOCAL_AGENT_EXECUTION_STEPS: usize = 100_000;

/// Byte budget for a *successful* agent-profile result's `stack` and
/// `stackDisplay`, together, past which the largest slots are elided
/// (`agent::error_stack`) rather than sent.
///
/// A success used to be sent whole or not at all: a 7,000-element vector left
/// on the stack became the host's `responseTooLarge`, and the caller learned
/// that its answer was too big and nothing else — not what was on the stack,
/// not how big, not that the small values beside it were fine. Elided, the
/// same result arrives with every affordable slot in full and the oversized
/// one replaced by a record of what it was, so the caller can see that it
/// left an intermediate value behind and fix that.
///
/// Sized against the MCP host's 1 MiB `responseBytes`, which bounds the
/// response as sent: the envelope twice (structured, and mirrored into a text
/// block where every quote is escaped), measured at 2.2x the envelope. A
/// stack of this many bytes leaves that response under the ceiling with
/// about 6% to spare, and still admits a 5,000-element vector of small
/// integers in full (433 KB). The estimate the budget is compared against is
/// `error_stack::node_wire_bytes`, calibrated on the real rendering.
pub const AGENT_STACK_BUDGET_BYTES: usize = 440 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ComputeOptions {
    pub step_limit: Option<usize>,
    pub runtime_limits: Option<RuntimeLimits>,
    /// Byte budget for a successful result's stack, past which the largest
    /// slots are elided. `None` sends every success whole, whatever its size
    /// — the trusted profile, for a host with no response ceiling.
    pub stack_budget_bytes: Option<usize>,
}

impl ComputeOptions {
    /// The agent profile: [`LOCAL_AGENT_RUNTIME_LIMITS`] and
    /// [`LOCAL_AGENT_EXECUTION_STEPS`], with `step_limit` overriding the
    /// step budget when given, and [`AGENT_STACK_BUDGET_BYTES`] bounding
    /// what a success sends. The profile used to leave the step budget at
    /// the interpreter's derived default when none was given, so `ajisai
    /// agent compute` ran 12,180,000 steps where the MCP server, which names
    /// the same profile, ran 100,000.
    pub const fn agent(step_limit: Option<usize>) -> Self {
        ComputeOptions {
            step_limit: Some(match step_limit {
                Some(limit) => limit,
                None => LOCAL_AGENT_EXECUTION_STEPS,
            }),
            runtime_limits: Some(LOCAL_AGENT_RUNTIME_LIMITS),
            stack_budget_bytes: Some(AGENT_STACK_BUDGET_BYTES),
        }
    }

    /// The agent profile's ceilings as `ajisai://limits` names them, for a host that wants to show them beside its own.
    pub fn agent_limit_profile() -> serde_json::Value {
        let options = Self::agent(None);
        crate::interpreter::limit_profile::to_json(
            &LOCAL_AGENT_RUNTIME_LIMITS,
            options.step_limit.unwrap_or(LOCAL_AGENT_EXECUTION_STEPS),
        )
    }

    /// Put `interp` under these ceilings; an absent one keeps the
    /// interpreter's own default.
    pub(crate) fn apply(&self, interp: &mut Interpreter) {
        if let Some(limits) = self.runtime_limits {
            interp.set_runtime_limits(limits);
        }
        if let Some(limit) = self.step_limit {
            interp.set_max_execution_steps(limit);
        }
    }

    /// A fresh interpreter under these ceilings.
    pub(crate) fn interpreter(&self) -> Interpreter {
        let mut interp = Interpreter::new();
        self.apply(&mut interp);
        interp
    }
}

pub struct AgentResponse {
    report: Report,
}

pub struct ContractResponse {
    /// The inferred contracts, or the source-form error that stopped
    /// inference before it began — the same report `check` gives.
    result: Result<serde_json::Value, Box<AgentResponse>>,
}

impl ContractResponse {
    /// The agent envelope around the inferred contracts, or the error report.
    pub fn to_json(&self) -> serde_json::Value {
        match &self.result {
            Ok(contracts) => serde_json::json!({
                "schemaVersion": super::report::SCHEMA_VERSION,
                "specificationVersion": crate::kernel::generated::SPECIFICATION_VERSION,
                "status": "ok",
                "contracts": contracts,
            }),
            Err(report) => report.to_json(),
        }
    }

    pub fn exit_code(&self) -> i32 {
        match &self.result {
            Ok(_) => 0,
            Err(report) => report.exit_code(),
        }
    }
}

impl AgentResponse {
    /// A `compute` report, carrying the outcome id it names.
    fn computed(mut report: Report) -> Self {
        report.outcome = report.outcome_id();
        AgentResponse { report }
    }

    pub fn exit_code(&self) -> i32 {
        if self.report.status == "ok" {
            0
        } else {
            1
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        self.report.to_json()
    }

    /// [`AgentResponse::to_json`], consuming the response so the stack's JSON
    /// is moved into the envelope rather than copied.
    pub fn into_json(self) -> serde_json::Value {
        self.report.into_json()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn report(&self) -> &Report {
        &self.report
    }
}

/// Execute one source document and return the same structured observation the
/// CLI emits, without creating a file or writing stdout/stderr.
pub async fn compute(source: &str, options: ComputeOptions) -> AgentResponse {
    // The pre-execution check (LANG.CONTRACT.CHECK) runs here as well as in
    // `check`: a program that declares a contract for its own Word is
    // checked against it *before anything runs*, and a declaration inference
    // disproves stops the run — the program is wrong about itself, and
    // executing it would answer a value as if it were not. The result is the
    // same report `check` gives, with `contractDecls` carrying every finding.
    // Programs without a directive are untouched, which is every program
    // that does not opt in.
    let mut interp = options.interpreter();
    let decls = contract::declared_contract_check(source);
    if let Some(check) = &decls {
        if check.violated {
            return AgentResponse::computed(contract::violation_report(&interp, check));
        }
    }
    let result = interp.execute(source).await;
    let output = print_payloads(&interp);
    let mut report = completed_run_report(&interp, result, output, options.stack_budget_bytes);
    // A verified (or unverifiable) declaration is reported too, so the caller
    // sees the check happened and what it decided; a source with no
    // directive carries no `contractDecls` at all.
    report.contract_decls = decls.as_ref().map(|check| check.to_json());
    AgentResponse::computed(report)
}

/// The source-form gate every execution-free operation shares: a source that
/// does not tokenize, or whose vector delimiters do not balance, is
/// `malformedSource` before anything else can be said about it. `Err` carries
/// the finished error report.
///
/// Both refusals come from the one call: the tokenizer's structural phase
/// (`spec/grammar.json`, structuralValidation) runs on every `tokenize`
/// result, so an unbalanced bracket is a tokenize failure here exactly as it
/// is to `compute` — one phase, one message, for every tool.
///
/// `check` had this gate and `infer_contracts` did not, so `[ 1 2` was a
/// `malformedSource` error to `compute` and `check` and an `ok`
/// with no contracts to inference — three tools, two answers about one source.
fn tokens_of_well_formed(
    interp: &Interpreter,
    source: &str,
) -> Result<Vec<crate::types::Token>, Box<AgentResponse>> {
    let message = match crate::tokenizer::tokenize(source) {
        Ok(tokens) => return Ok(tokens),
        Err(message) => message,
    };
    // The same category `run` reports for an unbalanced bracket.
    Err(Box::new(AgentResponse {
        report: error_report(
            interp,
            ErrorInfo::of(ErrorCategory::MalformedSource, None),
            message,
            Vec::new(),
        ),
    }))
}

/// Validate source without executing it and return the standard report shape:
/// every name resolves, every `#:contract` declaration holds, and every block
/// written before MAP, FILTER, FOLD or SCAN can leave the one value its Word
/// requires (`block_arity`).
pub fn check(source: &str) -> AgentResponse {
    let interp = Interpreter::new();
    let tokens = match tokens_of_well_formed(&interp, source) {
        Ok(tokens) => tokens,
        Err(report) => return *report,
    };
    let resolved = resolve_words(&interp, &tokens);
    let unknown = &resolved.unknown;
    if let Some(first) = unknown.first() {
        let mut message = format!("Unknown words: {}", unknown.join(", "));
        // The same repair the runtime's Unknown Word diagnosis appends, one
        // per name that has one.
        let hints: Vec<String> = unknown
            .iter()
            .filter(|name| !resolved.bound_elsewhere.contains(name))
            .filter_map(|name| {
                crate::diagnostic_hint::unknown_word_hint(name)
                    .map(|hint| format!("{name} — {hint}"))
            })
            .collect();
        if !hints.is_empty() {
            message.push_str(&format!(". {}", hints.join("; ")));
        }
        if !resolved.bound_elsewhere.is_empty() {
            message.push_str(&format!(
                ". {} is bound in another frame: a binding is reachable in the frame that made it \
                 and in the blocks written there, never inside a Word it calls — pass the value \
                 as an operand instead",
                resolved.bound_elsewhere.join(", ")
            ));
        }
        return AgentResponse {
            report: error_report(
                &interp,
                ErrorInfo::of(ErrorCategory::UnknownWord, Some(first.clone())),
                message,
                Vec::new(),
            ),
        };
    }

    let contract_decls = contract::check_contract_decls(source);
    if contract_decls.violated {
        // The same shape every other error has — `message` and
        // `error.category: contractViolation` — so the violation is where
        // every other failure is, not only in `contractDecls.findings`.
        return AgentResponse {
            report: contract::violation_report(&interp, &contract_decls),
        };
    }
    if let Some(violation) = super::block_arity::first_block_arity_violation(source) {
        let category = match violation.category {
            "stackUnderflow" => ErrorCategory::StackUnderflow,
            declared => ErrorCategory::Declared(declared),
        };
        let mut error = ErrorInfo::of(category, Some(violation.word.to_string()));
        error.position = violation.position;
        return AgentResponse {
            report: error_report(&interp, error, violation.message, Vec::new()),
        };
    }
    AgentResponse {
        report: Report {
            status: "ok",
            stack: serde_json::Value::Array(Vec::new()),
            stack_display: Vec::new(),
            output: Vec::new(),
            message: None,
            error: None,
            resource_usage: crate::interpreter::ResourceUsage::default(),
            contract_decls: Some(contract_decls.to_json()),
            stack_elided: None,
            outcome: None,
        },
    }
}

/// Infer user-Word contracts without executing definitions or top-level code.
pub fn infer_contracts(source: &str) -> ContractResponse {
    if let Err(report) = tokens_of_well_formed(&Interpreter::new(), source) {
        return ContractResponse {
            result: Err(report),
        };
    }
    ContractResponse {
        result: Ok(contract::contracts_json(source)),
    }
}
