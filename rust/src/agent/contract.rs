//! `#:contract` declarations and the reports built on contract inference
//! (LANG.CONTRACT.CHECK): `check`, the pre-execution check every
//! host runs, and `infer-contracts`.
//!
//! A declaration is a comment line to the interpreter —
//! `#:contract DOUBLE inputs=1 outputs=1 field=closed` — naming a Word the
//! same source defines. `inputs` and `outputs` must equal the inferred counts
//! and `field=closed` is a bound an inferred `leaving` violates; a key left out
//! is not checked. Each declaration has one of three outcomes, the trichotomy
//! at check time: verified, cannot verify (the inference gave the weaker
//! answer), or violated.

use super::report::{ErrorInfo, Report};
use crate::coreword_registry::FieldClosure;
use crate::error::ErrorCategory;
use crate::interpreter::word_contract::{ContractFlow, WordContract};
use crate::interpreter::Interpreter;
use crate::types::Token;
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
struct ContractDecl {
    pub name: String,
    pub inputs: Option<u16>,
    pub outputs: Option<u16>,
    pub field: Option<FieldClosure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Error,
    Note,
}

pub(crate) struct DeclFinding {
    pub severity: Severity,
    pub message: String,
}

/// One declaration's outcome: `value` (verified), `nil` (cannot verify) or
/// `error` (violated), ordered so the worst of several is their maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CheckOutcome {
    Value,
    Nil,
    Error,
}

impl CheckOutcome {
    fn as_str(self) -> &'static str {
        match self {
            CheckOutcome::Value => "value",
            CheckOutcome::Nil => "nil",
            CheckOutcome::Error => "error",
        }
    }
}

pub(crate) struct ContractDeclCheck {
    pub findings: Vec<DeclFinding>,
    pub violated: bool,
    pub decl_outcomes: Vec<(String, CheckOutcome)>,
}

impl ContractDeclCheck {
    pub(crate) fn to_json(&self) -> serde_json::Value {
        let worst = self
            .decl_outcomes
            .iter()
            .map(|(_, outcome)| *outcome)
            .chain(self.violated.then_some(CheckOutcome::Error))
            .max()
            .unwrap_or(CheckOutcome::Value);
        serde_json::json!({
            "findings": self.findings.iter().map(|f| serde_json::json!({
                "severity": match f.severity { Severity::Error => "error", Severity::Note => "note" },
                "message": f.message,
            })).collect::<Vec<_>>(),
            "outcome": worst.as_str(),
            "declarations": self.decl_outcomes.iter().map(|(word, outcome)| serde_json::json!({
                "word": word,
                "outcome": outcome.as_str(),
            })).collect::<Vec<_>>(),
        })
    }
}

/// Every `#:contract` line in `source`, and a message for each malformed one.
fn parse_contract_directives(source: &str) -> (Vec<ContractDecl>, Vec<String>) {
    let mut decls = Vec::new();
    let mut errors = Vec::new();
    for line in source.lines() {
        let Some(body) = line.trim_start().strip_prefix("#:contract") else {
            continue;
        };
        let mut terms = body.split_whitespace();
        let Some(name) = terms.next() else {
            errors.push("empty `#:contract` directive (expected a word name)".to_string());
            continue;
        };
        let mut decl = ContractDecl {
            name: name.to_uppercase(),
            ..ContractDecl::default()
        };
        match terms.try_for_each(|term| parse_term(term, &mut decl)) {
            Ok(()) => decls.push(decl),
            Err(e) => errors.push(format!("`#:contract {name}`: {e}")),
        }
    }
    (decls, errors)
}

fn parse_term(term: &str, decl: &mut ContractDecl) -> Result<(), String> {
    let unknown = || {
        format!(
            "unknown term `{term}` (expected `inputs=N`, `outputs=N` or `field=closed|leaving`)"
        )
    };
    let (key, value) = term.split_once('=').ok_or_else(unknown)?;
    let count = || {
        value
            .parse::<u16>()
            .map_err(|_| format!("`{key}` is a non-negative integer, got `{value}`"))
    };
    match key {
        "inputs" => decl.inputs = Some(count()?),
        "outputs" => decl.outputs = Some(count()?),
        "field" => {
            decl.field = Some(
                FieldClosure::from_spec_str(value)
                    .ok_or_else(|| format!("`field` is `closed` or `leaving`, got `{value}`"))?,
            )
        }
        _ => return Err(unknown()),
    }
    Ok(())
}

/// The `[ ... ] 'NAME' DEF` definitions written at the top level of `tokens`:
/// name, body, and the index of the `DEF`.
pub(super) fn collect_top_level_defs(tokens: &[Token]) -> Vec<(String, Vec<Token>, usize)> {
    let mut defs = Vec::new();
    let mut depth = 0i32;
    let mut open_at = None;
    for (idx, token) in tokens.iter().enumerate() {
        match token {
            Token::VectorStart => {
                if depth == 0 {
                    open_at = Some(idx);
                }
                depth += 1;
            }
            Token::VectorEnd => {
                depth -= 1;
                if depth != 0 {
                    continue;
                }
                let Some(open) = open_at.take() else { continue };
                if let (Some(Token::String(name)), true) =
                    (tokens.get(idx + 1), is_def_symbol(tokens.get(idx + 2)))
                {
                    defs.push((name.to_string(), tokens[open + 1..idx].to_vec(), idx + 2));
                }
            }
            _ => {}
        }
    }
    defs
}

pub(super) fn is_def_symbol(token: Option<&Token>) -> bool {
    matches!(token, Some(Token::Symbol(s)) if crate::word_name::canonical_word_name(s) == "DEF")
}

/// Names the check cannot pin to one definition without running the source:
/// a Word defined twice, or by a `DEF` it does not read (`every` when that
/// `DEF`'s name is itself computed).
#[derive(Debug, Default)]
pub(super) struct UnsettledNames {
    names: HashSet<String>,
    every: bool,
}

impl UnsettledNames {
    fn contains(&self, name: &str) -> bool {
        self.every || self.names.contains(&name.to_uppercase())
    }

    fn of(tokens: &[Token], defs: &[(String, Vec<Token>, usize)]) -> Self {
        let mut unsettled = UnsettledNames::default();
        let mut seen = HashSet::new();
        for (name, _, _) in defs {
            let upper = name.to_uppercase();
            if !seen.insert(upper.clone()) {
                unsettled.names.insert(upper);
            }
        }
        let read: HashSet<usize> = defs.iter().map(|(_, _, at)| *at).collect();
        for (idx, token) in tokens.iter().enumerate() {
            if read.contains(&idx) || !is_def_symbol(Some(token)) {
                continue;
            }
            match idx.checked_sub(1).map(|prev| &tokens[prev]) {
                Some(Token::String(name)) => {
                    unsettled.names.insert(name.to_uppercase());
                }
                _ => unsettled.every = true,
            }
        }
        unsettled
    }
}

/// A fresh interpreter holding the source's top-level definitions, made by
/// `DEF` itself and nothing else run, with the names it defined.
pub(super) fn build_definitions_interpreter(
    source: &str,
) -> (Interpreter, Vec<String>, UnsettledNames) {
    let mut interp = Interpreter::new();
    let mut names = Vec::new();
    let mut unsettled = UnsettledNames::default();
    if let Ok(tokens) = crate::tokenizer::tokenize(source) {
        let defs = collect_top_level_defs(&tokens);
        unsettled = UnsettledNames::of(&tokens, &defs);
        for (name, body, _) in defs {
            if crate::interpreter::execute_def::op_def_inner(&mut interp, &name, &body).is_ok() {
                let upper = name.to_uppercase();
                if !names.contains(&upper) {
                    names.push(upper);
                }
            }
        }
        interp.output_buffer.clear();
    }
    (interp, names, unsettled)
}

pub(crate) fn check_contract_decls(source: &str) -> ContractDeclCheck {
    let (decls, parse_errors) = parse_contract_directives(source);
    let mut findings: Vec<DeclFinding> = parse_errors
        .into_iter()
        .map(|message| DeclFinding {
            severity: Severity::Error,
            message,
        })
        .collect();
    let (interp, _, unsettled) = build_definitions_interpreter(source);
    let mut decl_outcomes = Vec::with_capacity(decls.len());
    for decl in &decls {
        let before = findings.len();
        if unsettled.contains(&decl.name) {
            findings.push(DeclFinding {
                severity: Severity::Note,
                message: format!(
                    "`#:contract {}`: the word is defined more than once, or by a `DEF` the \
                     check does not read, so the check cannot tell which definition a call \
                     runs (unverified).",
                    decl.name
                ),
            });
        } else {
            check_one(&interp, decl, &mut findings);
        }
        let new = &findings[before..];
        let outcome = if new.iter().any(|f| f.severity == Severity::Error) {
            CheckOutcome::Error
        } else if new.is_empty() {
            CheckOutcome::Value
        } else {
            CheckOutcome::Nil
        };
        decl_outcomes.push((decl.name.clone(), outcome));
    }
    ContractDeclCheck {
        violated: findings.iter().any(|f| f.severity == Severity::Error),
        findings,
        decl_outcomes,
    }
}

fn check_one(interp: &Interpreter, decl: &ContractDecl, findings: &mut Vec<DeclFinding>) {
    let Some(contract) = interp.infer_word_contract(&decl.name) else {
        findings.push(DeclFinding {
            severity: Severity::Error,
            message: format!("`#:contract {}`: no such word is defined.", decl.name),
        });
        return;
    };
    let mut differs = |declared: String, inferred: String, proven: bool| {
        findings.push(DeclFinding {
            severity: if proven {
                Severity::Error
            } else {
                Severity::Note
            },
            message: format!(
                "`#:contract {}`: declared `{declared}` but inferred `{inferred}`{}.",
                decl.name,
                if proven { "" } else { " (unverified)" }
            ),
        });
    };
    let counts = match contract.flow {
        ContractFlow::Fixed { consumes, produces } => Some((consumes, produces)),
        ContractFlow::Dynamic => None,
    };
    for (key, declared, inferred) in [
        ("inputs", decl.inputs, counts.map(|c| c.0)),
        ("outputs", decl.outputs, counts.map(|c| c.1)),
    ] {
        match (declared, inferred) {
            (Some(d), Some(i)) if d != i => {
                differs(format!("{key}={d}"), format!("{key}={i}"), true)
            }
            (Some(d), None) => differs(format!("{key}={d}"), format!("{key}=variable"), false),
            _ => {}
        }
    }
    if decl.field == Some(FieldClosure::Closed) && contract.field == FieldClosure::Leaving {
        let inferred = match contract.field_exits.as_slice() {
            [] => "field=leaving".to_string(),
            exits => format!("field=leaving` at `{}", exits.join("`, `")),
        };
        differs("field=closed".to_string(), inferred, !contract.field_unread);
    }
}

/// The paste-ready directive the inference supports.
fn suggested_directive(name: &str, contract: &WordContract) -> String {
    let mut line = format!("#:contract {name}");
    if let ContractFlow::Fixed { consumes, produces } = contract.flow {
        line.push_str(&format!(" inputs={consumes} outputs={produces}"));
    }
    line.push_str(&format!(" field={}", contract.field.as_spec_str()));
    line
}

/// The inferred contract of every Word `source` defines at its top level, as
/// `infer-contracts` reports it.
pub(crate) fn contracts_json(source: &str) -> serde_json::Value {
    let (interp, names, _) = build_definitions_interpreter(source);
    let count = |n: Option<u16>| n.map_or_else(|| serde_json::json!("variable"), |n| n.into());
    serde_json::Value::Array(
        names
            .iter()
            .filter_map(|name| {
                let contract = interp.infer_word_contract(name)?;
                let counts = match contract.flow {
                    ContractFlow::Fixed { consumes, produces } => (Some(consumes), Some(produces)),
                    ContractFlow::Dynamic => (None, None),
                };
                Some(serde_json::json!({
                    "name": name,
                    "inputs": count(counts.0),
                    "outputs": count(counts.1),
                    "field": contract.field.as_spec_str(),
                    "fieldExits": contract.field_exits,
                    "suggested": suggested_directive(name, &contract),
                }))
            })
            .collect(),
    )
}

/// The declaration check for a program that opted in — any `#:contract`
/// line, well-formed or not — or `None` for one that wrote no directive:
/// `compute` attaches `contractDecls` exactly when there is a declaration to
/// report on. LANG.CONTRACT.CHECK is a pre-execution check, so every host
/// operation that would run a program checks it first.
pub(crate) fn declared_contract_check(source: &str) -> Option<ContractDeclCheck> {
    let declares = source
        .lines()
        .any(|line| line.trim_start().starts_with("#:contract"));
    declares.then(|| check_contract_decls(source))
}

/// The error report for a violated declaration: the one shape every other
/// error has (LANG.FAILURE.TRICHOTOMY), with the declaration findings beside
/// it as `contractDecls`.
pub(crate) fn violation_report(interp: &Interpreter, check: &ContractDeclCheck) -> Report {
    let (message, word) = violation_message(check);
    let mut report = super::error_report(
        interp,
        ErrorInfo::of(ErrorCategory::ContractViolation, word),
        message,
        Vec::new(),
    );
    report.contract_decls = Some(check.to_json());
    report
}

/// The message a violated declaration answers with, and the Word it names,
/// shared by every host that refuses to run it: the agent report above and
/// the playground's `execute`.
pub(crate) fn violation_message(check: &ContractDeclCheck) -> (String, Option<String>) {
    let violations: Vec<&DeclFinding> = check
        .findings
        .iter()
        .filter(|finding| finding.severity == Severity::Error)
        .collect();
    // The Word the first violated declaration names, when it names one that
    // is defined: the locus of the failure. A malformed directive or a
    // declaration for an undefined Word has no Word to point at.
    let word = check
        .decl_outcomes
        .iter()
        .find(|(_, outcome)| matches!(outcome, CheckOutcome::Error))
        .map(|(word, _)| word.clone());
    let message = format!(
        "Contract declaration violated: {}",
        violations
            .iter()
            .map(|finding| finding.message.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    );
    (message, word)
}
