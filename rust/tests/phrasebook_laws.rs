//! The phrasebook (`docs/phrasebook/phrasebook.ajisai`) says what it does.
//!
//! Every `#:contract` line in it is verified by the pre-execution check
//! (LANG.CONTRACT.CHECK), every `#= program → stack` line, run after the
//! whole file under the agent profile, leaves exactly that stack, and every
//! `#:digest NAME HEX` line is the content identity NAME has once the file
//! has run (LANG.DICTIONARY.MUTATION). The MCP server's `words` argument and
//! its `prelude` read those digests, so a stale one would mislabel a Word. The file is
//! User Words over the Core vocabulary and nothing else, so a change to a Core
//! Word that breaks a phrase breaks this suite rather than a reader's program.

use ajisai_core::agent::api::{check, compute, ComputeOptions};

const PHRASEBOOK: &str = include_str!("../../docs/phrasebook/phrasebook.ajisai");

/// The `#=` examples, as (program, displayed stack).
fn examples() -> Vec<(&'static str, &'static str)> {
    let examples: Vec<_> = PHRASEBOOK
        .lines()
        .filter_map(|line| line.strip_prefix("#= "))
        .map(|example| {
            example
                .split_once(" → ")
                .unwrap_or_else(|| panic!("`#= {example}` has no ` → `"))
        })
        .collect();
    assert!(!examples.is_empty(), "the phrasebook holds no examples");
    examples
}

#[test]
fn every_declaration_in_the_phrasebook_is_verified() {
    let report = check(PHRASEBOOK).to_json();
    assert_eq!(report["status"], "ok", "{report}");
    let decls = &report["contractDecls"];
    assert_eq!(decls["outcome"], "value", "{decls}");
    let declared = PHRASEBOOK
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
async fn every_example_in_the_phrasebook_leaves_the_stack_it_names() {
    for (program, expected) in examples() {
        let source = format!("{PHRASEBOOK}\n{program}\n");
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
async fn the_phrasebook_alone_leaves_nothing_behind() {
    // Running the file to load its Words must not leave values a program
    // after it would read as operands.
    let report = compute(PHRASEBOOK, ComputeOptions::agent(None))
        .await
        .to_json();
    assert_eq!(report["status"], "ok", "{report}");
    assert_eq!(report["stackDisplay"], serde_json::json!([]), "{report}");
}

#[tokio::test]
async fn every_digest_line_is_the_identity_of_the_word_it_names() {
    let digests: Vec<(&str, &str)> = PHRASEBOOK
        .lines()
        .filter_map(|line| line.strip_prefix("#:digest "))
        .map(|rest| rest.split_once(' ').expect("`#:digest NAME HEX`"))
        .collect();
    let defined = PHRASEBOOK
        .lines()
        .filter(|line| line.ends_with("' DEF"))
        .count();
    assert_eq!(digests.len(), defined, "one #:digest line per definition");
    let reads: Vec<String> = digests
        .iter()
        .map(|(name, _)| format!("[ {name} ] 0 GET DIGEST"))
        .collect();
    let source = format!("{PHRASEBOOK}\n{}\n", reads.join(" "));
    let report = compute(&source, ComputeOptions::agent(None))
        .await
        .to_json();
    assert_eq!(report["status"], "ok", "{report}");
    let shown = report["stackDisplay"].as_array().expect("stackDisplay");
    for ((name, recorded), actual) in digests.iter().zip(shown) {
        let actual = actual
            .as_str()
            .expect("a display string")
            .trim_matches('\'');
        assert_eq!(*recorded, actual, "#:digest {name} is stale");
    }
}
