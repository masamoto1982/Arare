//! The showcase (`docs/showcase/showcase.ajisai`) says what it does.
//!
//! The file sets each program beside what binary floating point would answer,
//! so a claim in it that the engine stopped keeping would be a false claim
//! about the language. Every `#:contract` line in it is verified by the
//! pre-execution check (LANG.CONTRACT.CHECK); every `#= program → stack` line,
//! run after the whole file under the agent profile, leaves exactly that
//! stack; and the refusal the file describes in prose — `MEAN` declared
//! `field=closed` — is refused, at the `DIV` it names.

use ajisai_core::agent::api::{check, compute, ComputeOptions};

const SHOWCASE: &str = include_str!("../../docs/showcase/showcase.ajisai");

/// The `#=` examples, as (program, displayed stack).
fn examples() -> Vec<(&'static str, &'static str)> {
    let examples: Vec<_> = SHOWCASE
        .lines()
        .filter_map(|line| line.strip_prefix("#= "))
        .map(|example| {
            example
                .split_once(" → ")
                .unwrap_or_else(|| panic!("`#= {example}` has no ` → `"))
        })
        .collect();
    assert!(!examples.is_empty(), "the showcase holds no examples");
    examples
}

#[test]
fn every_declaration_in_the_showcase_is_verified() {
    let report = check(SHOWCASE).to_json();
    assert_eq!(report["status"], "ok", "{report}");
    let decls = &report["contractDecls"];
    assert_eq!(decls["outcome"], "value", "{decls}");
    let declared = SHOWCASE
        .lines()
        .filter(|l| l.starts_with("#:contract "))
        .count();
    let verified = decls["declarations"]
        .as_array()
        .expect("declarations")
        .iter()
        .filter(|d| d["outcome"] == "value")
        .count();
    assert_eq!(verified, declared, "{decls}");
}

#[tokio::test]
async fn every_example_in_the_showcase_leaves_the_stack_it_names() {
    for (program, expected) in examples() {
        let source = format!("{SHOWCASE}\n{program}\n");
        let report = compute(&source, ComputeOptions::agent(None))
            .await
            .to_json();
        assert_eq!(report["status"], "ok", "`{program}`: {report}");
        let shown: Vec<&str> = report["stackDisplay"]
            .as_array()
            .expect("stackDisplay")
            .iter()
            .map(|v| v.as_str().expect("a display string"))
            .collect();
        assert_eq!(shown.join(" "), expected, "`{program}`");
    }
}

#[tokio::test]
async fn the_showcase_alone_leaves_nothing_behind() {
    let report = compute(SHOWCASE, ComputeOptions::agent(None))
        .await
        .to_json();
    assert_eq!(report["status"], "ok", "{report}");
    assert_eq!(report["stackDisplay"], serde_json::json!([]), "{report}");
}

#[test]
fn mean_declared_closed_is_refused_at_its_div() {
    let leaving = "#:contract MEAN inputs=1 outputs=1 field=leaving";
    assert!(
        SHOWCASE.contains(leaving),
        "the showcase declares MEAN leaving"
    );
    let closed = SHOWCASE.replace(leaving, "#:contract MEAN inputs=1 outputs=1 field=closed");
    let report = check(&closed).to_json();
    let decls = &report["contractDecls"];
    assert_eq!(decls["outcome"], "error", "{decls}");
    let mean = decls["declarations"]
        .as_array()
        .expect("declarations")
        .iter()
        .find(|d| d["word"] == "MEAN")
        .expect("MEAN is declared");
    assert_eq!(mean["outcome"], "error", "{mean}");
    assert!(
        decls.to_string().contains("DIV"),
        "the refusal names the DIV MEAN leaves the field at: {decls}"
    );
}
