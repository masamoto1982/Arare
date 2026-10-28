//! Every declared error reads the same way: `WORD: expected …, got …`.
//!
//! The Word's name is attached once, by the dispatcher (`AjisaiError::
//! attributed_to`), and an operand is named by its domain as
//! LANG.VALUES.DISJOINT spells it (`Value::domain_name`) or, for a Scalar, by
//! its value. This sweeps every Word with a fixed arity over one operand from
//! each domain, so a raise site that spells a name by hand, borrows another
//! Word's, or invents its own word for a domain is caught wherever it is.

use crate::error::{AjisaiError, ErrorCategory};
use crate::interpreter::Interpreter;
use crate::kernel::generated::{Arity, GENERATED_WORDS};

/// One operand from each of the seven domains, plus an irrational and a
/// non-integer Scalar, whose messages name the value rather than the domain.
const OPERANDS: [&str; 9] = [
    "1",
    "1/2",
    "2 SQRT",
    "TRUE",
    "'a'",
    "[ 1 ]",
    "[ 'k' ] [ 1 ] RECORD",
    "NIL",
    "[ ADD ] 0 GET",
];

const DOMAINS: [&str; 7] = [
    "Scalar", "Boolean", "String", "Vector", "Record", "NIL", "Symbol",
];

/// Spellings a message used before it named domains one way.
const RETIRED_SPELLINGS: [&str; 14] = [
    "Nil",
    "nil ",
    "non-vector",
    "non-text",
    "non-numeric",
    "non-record",
    "non-truth",
    "non-scalar",
    "another value",
    "another format",
    "other format",
    "code block",
    "Number",
    "fraction",
];

fn combinations(arity: usize) -> Vec<Vec<&'static str>> {
    let mut out: Vec<Vec<&'static str>> = vec![Vec::new()];
    for _ in 0..arity {
        out = out
            .into_iter()
            .flat_map(|prefix| {
                OPERANDS.iter().map(move |operand| {
                    let mut next = prefix.clone();
                    next.push(operand);
                    next
                })
            })
            .collect();
    }
    out
}

fn check(word: &str, source: &str, error: &AjisaiError) -> Result<(), String> {
    let Some(ErrorCategory::Declared(condition)) = ErrorCategory::from_error(error) else {
        return Ok(());
    };
    if condition == "declaredFailure" {
        return Ok(());
    }
    let message = error.to_string();
    let prefix = format!("{word}: ");
    let Some(rest) = message.strip_prefix(&prefix) else {
        return Err(format!(
            "`{source}`: `{message}` does not start with `{prefix}`"
        ));
    };
    if let Some(name) = GENERATED_WORDS
        .iter()
        .map(|w| w.name)
        .find(|name| rest.starts_with(&format!("{name}:")) || rest.starts_with(&format!("{name} ")))
    {
        return Err(format!(
            "`{source}`: `{message}` names {name} a second time"
        ));
    }
    if let Some(spelling) = RETIRED_SPELLINGS.iter().find(|s| rest.contains(*s)) {
        return Err(format!(
            "`{source}`: `{message}` says `{spelling}` for a domain"
        ));
    }
    if let Some(got) = rest.rsplit_once("got ").map(|(_, got)| got) {
        let first: String = got
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect();
        if !first.is_empty() && !DOMAINS.contains(&first.as_str()) && !first.starts_with("sqrt") {
            return Err(format!(
                "`{source}`: `{message}` names `{first}`, not a domain"
            ));
        }
    }
    Ok(())
}

#[tokio::test]
async fn every_declared_error_names_its_word_once_and_its_operand_by_domain() {
    let mut failures = Vec::new();
    for word in GENERATED_WORDS {
        let Arity::Fixed(arity) = word.stack_inputs else {
            continue;
        };
        for operands in combinations(arity as usize) {
            let source = format!("{} {}", operands.join(" "), word.name);
            let mut interp = Interpreter::new();
            if let Err(error) = interp.execute(&source).await {
                if let Err(failure) = check(word.name, &source, &error) {
                    failures.push(failure);
                }
            }
        }
    }
    failures.sort();
    failures.dedup();
    assert!(
        failures.is_empty(),
        "{} message(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A `shapeMismatch` between shapes of different ranks names the lengths that
/// met on the right-aligned axis. It used to index each shape by the aligned
/// axis itself, so `[2, 3]` against `[4]` read "axis 1 is 3 and 1, and
/// neither is 1".
#[tokio::test]
async fn a_shape_mismatch_across_ranks_names_the_aligned_lengths() {
    let mut interp = Interpreter::new();
    let error = interp
        .execute("[ [ 1 2 3 ] [ 4 5 6 ] ] [ 1 2 3 4 ] ADD")
        .await
        .expect_err("[2, 3] and [4] do not align");
    let message = error.to_string();
    assert!(
        message.contains("axis 1 is 3 and 4, and neither is 1"),
        "{message}"
    );
}

#[test]
fn a_shape_mismatch_past_the_rank_reports_rather_than_panics() {
    let message = crate::error::AjisaiError::shape_mismatch(&[2], &[3], 1).to_string();
    assert!(message.contains("axis 1 is 1 and 1"), "{message}");
}
