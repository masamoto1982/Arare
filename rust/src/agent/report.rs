//! Report assembly for a completed execution, and its JSON rendering for the
//! `ajisai agent` envelope.
//!
//! Assembly is I/O-free, so the typed agent API and the terminal CLI observe
//! the same stack, NIL reasons, error, output and resource usage. Rendering
//! serializes the shared value protocol (`types::value_protocol`) and the
//! error (LANG.OBSERVATION.DIAGNOSIS: a category, a message, the Word and the
//! source position) into the camelCase wire format documented in
//! `docs/dev/agent-cli-output-contract.md`.

use super::error_report;
use crate::error::ErrorCategory;
use crate::interpreter::{Interpreter, ResourceUsage};
use crate::semantic::AbsenceMetadata;
use crate::types::value_protocol::{exact_terms, value_to_protocol, ProtocolNode, ProtocolValue};
use crate::types::{Value, ValueData};
use serde_json::{json, Map, Value as Json};

/// Version of the top-level `--json` envelope. Bump only on a breaking
/// change (field removal or rename); purely additive fields keep the same
/// version. See `docs/dev/agent-cli-output-contract.md`.
///
/// 4: `diagnosis`, `aiDiagnostic` and `errorFlowTrace` gave way to one
/// `error` object — `category`, `resource` for a ceiling, `word`, `line`,
/// `column` — the whole of what LANG.OBSERVATION.DIAGNOSIS asks an error to
/// carry beside its message.
///
/// 5: `observationDigest` and `receipt` are gone. No host verified either;
/// a value's digest is the `DIGEST` Word's.
pub(crate) const SCHEMA_VERSION: u64 = 5;

/// What an error report says about the failure beside its message: its
/// category, the ceiling that refused it, the Word it is attributed to
/// (`failure_site`), and the top-level source position that reached it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ErrorInfo {
    pub category: Option<ErrorCategory>,
    pub resource: Option<&'static str>,
    pub word: Option<String>,
    pub position: Option<crate::tokenizer::SourceSpan>,
}

impl ErrorInfo {
    pub(crate) fn of(category: ErrorCategory, word: Option<String>) -> Self {
        ErrorInfo {
            category: Some(category),
            word,
            ..ErrorInfo::default()
        }
    }

    /// The error a run ended in, as `interp` attributes it.
    pub(crate) fn of_run(interp: &Interpreter, err: &crate::error::AjisaiError) -> Self {
        let resource = match err {
            crate::error::AjisaiError::ResourceLimitExceeded { resource, .. } => {
                Some(resource.as_protocol_str())
            }
            _ => None,
        };
        ErrorInfo {
            category: ErrorCategory::from_error(err),
            resource,
            word: interp.failed_word_for(err).map(str::to_string),
            position: interp.current_source_position(),
        }
    }
}

pub(crate) fn error_json(error: &ErrorInfo) -> Json {
    let mut obj = Map::new();
    if let Some(category) = &error.category {
        obj.insert("category".into(), json!(category.as_protocol_str()));
    }
    if let Some(resource) = error.resource {
        obj.insert("resource".into(), json!(resource));
    }
    if let Some(word) = &error.word {
        obj.insert("word".into(), json!(word));
    }
    if let Some(span) = error.position {
        obj.insert("line".into(), json!(span.line));
        obj.insert("column".into(), json!(span.column));
    }
    Json::Object(obj)
}

pub(crate) struct Report {
    pub status: &'static str,
    pub stack: Json,
    /// Human display strings for the stack, bottom to top — the same text
    /// the GUI's Stack projection renders. Not the text `PRINT` writes: a
    /// top-level String is displayed quoted (`'a'`) and printed raw (`a`),
    /// see `types::display::format_for_output`. Carried in the JSON envelope as
    /// `stackDisplay` so agents and the SKILL.md generator can show
    /// "code → expected stack" pairs without re-deriving display rules.
    pub stack_display: Vec<String>,
    pub output: Vec<String>,
    pub message: Option<String>,
    pub error: Option<ErrorInfo>,
    /// What the run spent of the budgets that can refuse it. Read from the
    /// counters the ceilings read, so it cannot disagree with them.
    pub resource_usage: ResourceUsage,
    /// Per-word contract declarations checked against inference
    /// (`check`, P2). `None` unless the user opted in; additive
    /// field. Prebuilt JSON so `report` stays decoupled from the declaration
    /// types.
    pub contract_decls: Option<Json>,
    /// Which stack slots an error report dropped the values of, and why
    /// (`agent::error_stack`). `None` whenever nothing was dropped, which is
    /// every success and every ordinary error.
    pub stack_elided: Option<Json>,
    /// The outcome id (`spec/outcomes.json`) a `compute` run produced —
    /// `value`, `nil:<reason>` or `error:<category>`, from
    /// [`Report::outcome_id`]. `None` for `check`, which never runs, and for
    /// a run the id cannot name; the JSON then carries no `outcome` at all.
    pub outcome: Option<String>,
}

impl Report {
    /// The outcome id this report names (`spec/outcomes.json`).
    ///
    /// `status` separates a value from an error but folds a reasoned absence
    /// into `ok`, so LANG.FAILURE's three results were reconstructable only by
    /// reading the top stack node's `semantics.absence.reason`. An error is its
    /// category, a NIL on top is its reason, anything else — an empty stack
    /// included — is a value. A report this cannot classify (a NIL with no
    /// reason, an error naming no category) has no id rather than a guessed
    /// one.
    pub(crate) fn outcome_id(&self) -> Option<String> {
        match self.status {
            "error" => self
                .error
                .as_ref()
                .and_then(|error| error.category.as_ref())
                .map(|category| format!("error:{}", category.as_protocol_str())),
            "ok" => match self.stack.as_array().and_then(|stack| stack.last()) {
                Some(top) if top["type"] == "nil" => top["semantics"]["absence"]["reason"]
                    .as_str()
                    .filter(|reason| !reason.is_empty())
                    .map(|reason| format!("nil:{reason}")),
                _ => Some("value".to_string()),
            },
            _ => None,
        }
    }

    pub(crate) fn to_json(&self) -> Json {
        self.document(self.stack.clone())
    }

    /// [`Report::to_json`], moving the stack's JSON into the document instead
    /// of copying it: the stack is most of a large report, and `json!` would
    /// rebuild it node by node through `serde_json::to_value`.
    pub(crate) fn into_json(mut self) -> Json {
        let stack = std::mem::take(&mut self.stack);
        self.document(stack)
    }

    fn document(&self, stack: Json) -> Json {
        let mut doc = json!({
            "schemaVersion": SCHEMA_VERSION,
            "specificationVersion": crate::kernel::generated::SPECIFICATION_VERSION,
            "status": self.status,
            "stackDisplay": self.stack_display,
            "output": self.output,
            "message": self.message,
            "error": self.error.as_ref().map(error_json),
            "resourceUsage": resource_usage_json(&self.resource_usage),
            "contractDecls": self.contract_decls,
            "stackElided": self.stack_elided,
        });
        doc["stack"] = stack;
        if let Some(outcome) = &self.outcome {
            doc["outcome"] = json!(outcome);
        }
        doc
    }
}

pub(crate) fn stack_json(interp: &Interpreter) -> Json {
    let nodes: Vec<Json> = interp
        .get_stack()
        .iter()
        .map(|value| protocol_node_json(&value_to_protocol(value)))
        .collect();
    Json::Array(nodes)
}

fn absence_json(absence: &AbsenceMetadata) -> Json {
    let mut obj = Map::new();
    if let Some(reason) = &absence.reason {
        obj.insert("reason".into(), json!(reason.as_protocol_str()));
    }
    if let Some(detail) = &absence.detail {
        obj.insert("detail".into(), json!(detail.as_str()));
    }
    Json::Object(obj)
}

/// What the run spent of the budgets that could have refused it, in the keys
/// the host declares those budgets under.
///
/// Every key here names a
/// `mcp.limits` key and carries the same number the ceiling compared against,
/// so an agent can subtract one from the other and know what it has left; no
/// key here is an internal routing counter, and no counter there is a budget.
pub(crate) fn resource_usage_json(usage: &ResourceUsage) -> Json {
    let mut usage_json = json!({
        "executionSteps": usage.execution_steps,
        "numericWork": usage.numeric_work,
        "collectionWork": usage.collection_work,
    });
    // A per-result ceiling has no running total, so what it reports is the
    // largest result it measured — and only when there was one: a run that
    // made no irrational has nothing to say about algebraic terms.
    if usage.algebraic_terms > 0 {
        usage_json["algebraicTerms"] = json!(usage.algebraic_terms);
    }
    usage_json
}

/// JSON rendering of a `ProtocolNode`: `{ type, value, semantics? }`.
pub(crate) fn protocol_node_json(node: &ProtocolNode) -> Json {
    let mut obj = Map::new();
    obj.insert("semantics".into(), semantics_json(&node.semantics));
    obj.insert("type".into(), json!(node.type_str));
    let value = match &node.value {
        ProtocolValue::Null => Json::Null,
        ProtocolValue::Bool(b) => json!(b),
        ProtocolValue::Text(s) => json!(s),
        ProtocolValue::Number {
            numerator,
            denominator,
        } => json!({ "numerator": numerator, "denominator": denominator }),
        ProtocolValue::Children(kids) => Json::Array(kids.iter().map(protocol_node_json).collect()),
        ProtocolValue::Record { keys, values } => json!({
            "keys": keys.iter().map(protocol_node_json).collect::<Vec<_>>(),
            "values": values.iter().map(protocol_node_json).collect::<Vec<_>>(),
        }),
    };
    obj.insert("value".into(), value);
    Json::Object(obj)
}

/// JSON rendering of the per-value `semantics` block — the one rendering,
/// which the GUI's stack view shares.
pub(crate) fn semantics_json(value: &Value) -> Json {
    let mut obj = Map::new();
    if let Some(truth) = value.truth_value() {
        obj.insert("truthValue".into(), json!(truth));
    }
    if let Some(absence) = value.normalized_absence_metadata() {
        obj.insert("absence".into(), absence_json(&absence));
    }
    if matches!(value.data, ValueData::ExactScalar(_)) {
        obj.insert("approximate".into(), json!(true));
    }
    // The terms a consumer computes with. The rendering a reader takes in is
    // the stack display, which writes these same terms (`sqrt(2)`).
    if let Some(terms) = exact_terms(value) {
        obj.insert(
            "exactTerms".into(),
            Json::Array(
                terms
                    .into_iter()
                    .map(|term| {
                        json!({
                            "numerator": term.numerator,
                            "denominator": term.denominator,
                            "radicand": term.radicand,
                        })
                    })
                    .collect(),
            ),
        );
    }
    Json::Object(obj)
}

/// Assemble the report for a completed execution without performing host I/O.
/// The typed agent API and the terminal renderer share this boundary.
///
/// `stack_budget` is the byte budget a successful result's stack may take
/// before its largest slots are elided (`agent::error_stack`); `None` sends
/// the stack whole.
pub(crate) fn completed_run_report(
    interp: &Interpreter,
    result: crate::error::Result<()>,
    output: Vec<String>,
    stack_budget: Option<usize>,
) -> Report {
    match result {
        Ok(()) => {
            let resource_usage = interp.resource_usage();
            // The answer is the stack, so a success is sent whole wherever
            // the host can take it; under a budget, the slots that do not
            // fit are replaced by a record of what they were rather than
            // the whole result being refused (`agent::error_stack`).
            let residue = match stack_budget {
                Some(budget) => super::error_stack::elided_value_stack(interp, budget),
                None => super::error_stack::whole_stack(interp),
            };
            Report {
                status: "ok",
                stack: residue.stack,
                stack_display: residue.stack_display,
                output,
                message: None,
                error: None,
                resource_usage,
                contract_decls: None,
                stack_elided: residue.elided,
                outcome: None,
            }
        }
        Err(err) => {
            let error = ErrorInfo::of_run(interp, &err);
            error_report(interp, error, err.to_string(), output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::exact::ExactReal;
    use crate::types::fraction::Fraction;

    #[test]
    fn cli_keeps_algebraic_normal_form_beside_approximation() {
        let sqrt_two = ExactReal::from_sqrt_rational(Fraction::new(2.into(), 1.into()))
            .expect("sqrt(2) is in the supported algebraic domain");
        let value = Value::from_exact_real(sqrt_two);
        let semantics = semantics_json(&value);

        assert_eq!(semantics["approximate"], true);
        assert_eq!(semantics["exactTerms"][0]["numerator"], "1");
        assert_eq!(semantics["exactTerms"][0]["denominator"], "1");
        assert_eq!(semantics["exactTerms"][0]["radicand"], "2");
        // The rendering is the stack display's, not a second field here.
        assert!(semantics.get("exactDisplay").is_none());
    }
}
