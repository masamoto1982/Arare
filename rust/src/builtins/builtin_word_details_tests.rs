//! LOOKUP rendering checks, and structural consistency checks for built-in
//! `hover_syntax` examples (structural-constraint ledger items 9 and 10; see
//! `docs/dev/structural-constraint-ledger.md`). Kept in a sibling file so
//! `builtins.rs` stays within the file-size budget in
//! docs/dev/specification-implementation-rules.md.
//!
//! These convert three invariants from authoring convention into a build-time
//! guarantee: a `hover_syntax` example must be a well-formed snippet (item 9),
//! every word it names must be a real word (item 10), and every *concrete*
//! example must actually run (item 10b).
//!
//! The declared-effect guard below is here for the same reason: it keeps the
//! LOOKUP "Side Effects" prose total over what `spec/words.json` declares.

use super::{effect_sentence, lookup_builtin_detail, lookup_builtin_spec};
use crate::interpreter::Interpreter;
use crate::kernel::generated::GENERATED_WORDS;
use crate::tokenizer::tokenize;

/// The effect names are the specification's, not the runtime's, so a Word
/// gaining an effect in `spec/words.json` reaches the LOOKUP prose without any
/// Rust edit — and would print the raw protocol name if nobody wrote a sentence
/// for it. Requiring a sentence for every declared effect makes that a test
/// failure instead of a reader-visible `consoleWrite`.
#[test]
fn every_declared_effect_has_a_user_facing_sentence() {
    let mut checked = 0;
    for word in GENERATED_WORDS {
        for effect in word.effects {
            checked += 1;
            assert!(
                effect_sentence(effect).is_some(),
                "{} declares effect `{}` with no user-facing sentence",
                word.name,
                effect
            );
        }
    }
    assert!(checked > 0, "no Word declares an effect; the guard is idle");
}
#[test]
fn every_hover_syntax_is_a_well_formed_snippet() {
    // Ledger item 9. A `hover_syntax` is a runnable example, so requiring it to
    // tokenize makes well-formedness a build-time guarantee. Only tokenization
    // is sound to require of all of them — some are deliberate modifier fragments
    // (`. ADD`); symbol resolution is the sibling check below (item 10).
    for spec in GENERATED_WORDS {
        if spec.hover_syntax.is_empty() {
            continue;
        }
        assert!(
            tokenize(spec.hover_syntax).is_ok(),
            "{}: hover_syntax `{}` does not tokenize (malformed doc example)",
            spec.name,
            spec.hover_syntax
        );
    }
}
#[tokio::test]
async fn every_hover_syntax_calls_its_word_and_runs() {
    // Ledger items 10 and 10b. A `hover_syntax` is also the "one correct call"
    // a diagnosis quotes, so it must be one: it ends in the Word's own name
    // and it runs on a fresh interpreter. FAIL's one correct call is the
    // ERROR it exists to raise.
    let mut ran = 0u32;
    for spec in GENERATED_WORDS {
        if spec.hover_syntax.is_empty() {
            continue;
        }
        assert_eq!(
            spec.hover_syntax.split_whitespace().last(),
            Some(spec.name),
            "{}: hover_syntax `{}` does not end in the Word's canonical name",
            spec.name,
            spec.hover_syntax
        );
        let mut interp = Interpreter::new();
        let outcome = interp.execute(spec.hover_syntax).await;
        if spec.name == "FAIL" {
            let err = outcome.expect_err("FAIL's hover_syntax must raise");
            assert_eq!(
                crate::error::ErrorCategory::from_error(&err)
                    .expect("a program ERROR has a category")
                    .as_protocol_str(),
                "declaredFailure"
            );
        } else {
            assert!(
                outcome.is_ok(),
                "{}: hover_syntax `{}` does not run: {:?}",
                spec.name,
                spec.hover_syntax,
                outcome.err()
            );
        }
        ran += 1;
    }
    assert!(ran >= 70, "only {ran} hover_syntax examples ran");
}

/// Parse the `(consumes, produces)` arity from a `stack_effect` prose string,
/// or `None` when the prose is not in the machine-checkable subset (so the
/// caller abstains rather than risk a false mismatch). The DSL is `LHS -> RHS`,
/// where each side is a sequence of items: a bracketed group `[ … ]` / `{ … }`
/// counts as one stack slot, an empty group `[]` counts as zero, and a variadic
/// (`...`), annotated (`(…)`), or multi-arrow prose form abstains.
fn parse_stack_effect_arity(stack_effect: &str) -> Option<(u16, u16)> {
    if stack_effect == "no values popped or pushed" {
        return Some((0, 0));
    }
    let sides: Vec<&str> = stack_effect.split(" -> ").collect();
    if sides.len() != 2 {
        return None; // no single arrow: prose or a control-directive description
    }
    for side in &sides {
        if side.contains("...") || side.contains('(') {
            return None; // variadic or annotated: not a fixed arity
        }
    }
    Some((count_stack_items(sides[0])?, count_stack_items(sides[1])?))
}

/// Count top-level stack items in one side of a `stack_effect`. A new item
/// begins at each token seen at bracket depth 0; an empty group contributes
/// nothing. Unbalanced brackets abstain (`None`).
///
/// The empty group has two spellings — `[]` and the spaced `[ ]` — and only
/// the first was recognized, so `[ x ] -> [ ]` read as one output instead of
/// none. Nothing caught it while `PRINT` carried a `Dynamic` mass and was
/// skipped; the declared 1 -> 0 arity engaged the check and exposed it.
fn count_stack_items(side: &str) -> Option<u16> {
    let side = side.replace("[ ]", "[]").replace("{ }", "{}");
    let mut depth = 0i32;
    let mut count = 0u16;
    for token in side.split_whitespace() {
        if token == "[]" || token == "{}" {
            continue; // an empty group produces/consumes nothing
        }
        if depth == 0 {
            count += 1;
        }
        for ch in token.chars() {
            match ch {
                '[' | '{' => depth += 1,
                ']' | '}' => depth -= 1,
                _ => {}
            }
        }
        if depth < 0 {
            return None;
        }
    }
    (depth == 0).then_some(count)
}

#[test]
fn fixed_stack_effect_prose_matches_the_machine_mass() {
    // Structural-constraint ledger item 11 (convention -> structure): the
    // human-facing `stack_effect` prose and the machine `mass` contract (SPEC
    // LANG.MACHINE.WORD) are two descriptions of one word's arity that could drift. For
    // every word with a `Fixed` mass, the arity parsed from the prose must equal
    // the mass. The parser abstains (skips) on any prose outside its
    // machine-checkable subset, so this never raises a false mismatch; it only
    // fires when the two descriptions provably disagree.
    let mut compared = 0u32;
    for spec in GENERATED_WORDS {
        let crate::coreword_registry::MassContract::Fixed {
            consumes: mass_consumes,
            produces: mass_produces,
        } = crate::coreword_registry::mass_contract(spec.name)
        else {
            continue; // Dynamic mass: no fixed arity to check against
        };
        let Some((prose_consumes, prose_produces)) = parse_stack_effect_arity(spec.stack_effect)
        else {
            continue; // prose outside the machine-checkable subset: abstain
        };
        compared += 1;
        assert_eq!(
            (prose_consumes, prose_produces),
            (u16::from(mass_consumes), u16::from(mass_produces)),
            "{}: stack_effect `{}` reads as arity ({}, {}) but mass is ({}, {})",
            spec.name,
            spec.stack_effect,
            prose_consumes,
            prose_produces,
            mass_consumes,
            mass_produces
        );
    }
    // Guard against the check silently going vacuous (e.g. if the parser starts
    // abstaining on everything): a healthy share of the fixed-mass words must
    // actually be compared. There are ~25 today; require a conservative floor.
    assert!(
        compared >= 20,
        "stack_effect/mass cross-check only compared {compared} words; \
         the prose parser may have regressed into abstaining"
    );
}
/// The rendered stack `code` leaves on a fresh interpreter, or `None` if it
/// raised.
async fn run_render(code: &str) -> Option<Vec<String>> {
    let mut interp = Interpreter::new();
    interp.execute(code).await.ok()?;
    Some(crate::types::display::render_stack(interp.get_stack()))
}

/// Every "`code` is `value`" a summary states, as `(code, value)`.
fn stated_examples(summary: &str) -> Vec<(&str, &str)> {
    let parts: Vec<&str> = summary.split('`').collect();
    (1..parts.len().saturating_sub(2))
        .step_by(2)
        .filter(|&k| parts[k + 1] == " is ")
        .map(|k| (parts[k], parts[k + 2]))
        .collect()
}

#[tokio::test]
async fn every_summary_example_holds() {
    // A summary is the one prose source for what a Word does (LOOKUP, hover,
    // SKILL.md, the MCP quickstart all render it), so what it states is
    // executed: every "`code` is `value`" in it must leave exactly the stack
    // `value` leaves, and every Word whose call leaves a value states at
    // least one. PRINT and FAIL leave none — one writes, the other raises.
    for word in GENERATED_WORDS {
        let examples = stated_examples(word.summary);
        if !matches!(word.name, "PRINT" | "FAIL") {
            assert!(
                !examples.is_empty(),
                "{}: the summary states no `code` is `value` example",
                word.name
            );
        }
        for (code, value) in examples {
            let actual = run_render(code).await;
            let expected = run_render(value).await;
            assert!(
                actual.is_some() && actual == expected,
                "{}: the summary says `{code}` is `{value}`, but they leave {actual:?} and {expected:?}",
                word.name
            );
        }
    }
}

// ── LOOKUP rendering ─────────────────────────────────────────────────────

const REQUIRED_SECTIONS: &[&str] = &["Family:", "Summary:", "Stack Effect:"];

/// Sections every builtin now renders, authored entry or not: the
/// derived template (three-layer model §3.4) on top of the four base
/// sections.
const DERIVED_SECTIONS: &[&str] = &["Examples:", "Failure:", "Side Effects:", "Vocabulary:"];

#[test]
fn every_builtin_renders_the_derived_sections() {
    for spec in GENERATED_WORDS {
        let body = lookup_builtin_detail(spec.name);
        for section in REQUIRED_SECTIONS.iter().chain(DERIVED_SECTIONS) {
            assert!(
                body.contains(section),
                "{} LOOKUP body missing section {}: full body =\n{}",
                spec.name,
                section,
                body
            );
        }
    }
}

/// `LOOKUP` is a reading surface for the vocabulary, so it says which half
/// of the public Core a Word belongs to — and says it in terms that keep
/// Core one flat dictionary.
#[test]
fn lookup_reports_the_vocabulary_tier() {
    let kernel = lookup_builtin_detail("FOLD");
    assert!(
        kernel.contains("Vocabulary:") && kernel.contains("Semantic Kernel"),
        "FOLD LOOKUP body must name the Semantic Kernel:\n{}",
        kernel
    );
    let standard = lookup_builtin_detail("FILTER");
    assert!(
        standard.contains("Standard vocabulary (operational)"),
        "FILTER LOOKUP body must name its Standard kind:\n{}",
        standard
    );
    for word in GENERATED_WORDS {
        let body = lookup_builtin_detail(word.name);
        assert!(
            body.contains("Vocabulary:"),
            "{} LOOKUP body has no Vocabulary section",
            word.name
        );
    }
}

#[test]
fn nil_projection_rule_words_describe_nil_not_only_errors() {
    // GET / DIV / NUM describe their NIL cases separately from contract
    // errors.
    for word in ["GET", "DIV", "NUM"] {
        let body = lookup_builtin_detail(word);
        assert!(
            body.contains("NIL"),
            "{} LOOKUP body must describe its NIL case:\n{}",
            word,
            body
        );
    }
}

#[test]
fn word_without_authored_entry_falls_back_to_hover_example() {
    let body = lookup_builtin_detail("ROUND");
    let spec = lookup_builtin_spec("ROUND").expect("ROUND spec");
    assert!(
        body.contains(spec.hover_syntax),
        "ROUND Examples should reuse hover_syntax until authored:\n{}",
        body
    );
}

#[test]
fn lookup_for_add_contains_four_required_sections() {
    let body = lookup_builtin_detail("ADD");
    assert!(body.contains("# ADD"), "ADD header missing:\n{}", body);
    for section in REQUIRED_SECTIONS {
        assert!(
            body.contains(section),
            "ADD LOOKUP body missing section {}: full body =\n{}",
            section,
            body
        );
    }
}

#[test]
fn every_builtin_lookup_contains_all_four_sections() {
    for spec in GENERATED_WORDS {
        let body = lookup_builtin_detail(spec.name);
        for section in REQUIRED_SECTIONS {
            assert!(
                body.contains(section),
                "{} LOOKUP body missing section {}:\n{}",
                spec.name,
                section,
                body
            );
        }
    }
}

#[test]
fn word_header_is_the_bare_name() {
    // The header carries the name alone: no label the registry does not
    // declare. PRINT used to read `(experimental)` for having an effect.
    for name in ["ADD", "PRINT", "DEF"] {
        let body = lookup_builtin_detail(name);
        assert!(
            body.contains(&format!("# {name}\n")),
            "{name} header must be bare:\n{body}"
        );
    }
}

#[test]
fn comparison_words_have_uniform_stack_effect() {
    // All six comparison primitives must use the same stack-effect
    // notation so the four-section template is consistent across the
    // comparison category.
    const EXPECTED: &str = "[ a ] [ b ] -> [ TRUE | FALSE ]";
    for name in &["EQ", "LT", "GT"] {
        let spec =
            lookup_builtin_spec(name).unwrap_or_else(|| panic!("{} must be a Core Word", name));
        assert_eq!(
            spec.stack_effect, EXPECTED,
            "{} stack_effect deviates from the comparison-word standard",
            name
        );
    }
}

#[test]
fn lookup_output_is_utf8_plain_text() {
    for name in ["ADD", "MAP", "LOOKUP", "DEF", "TOP", "PRINT"] {
        let body = lookup_builtin_detail(name);
        assert!(
            !body.chars().any(|c| c.is_control() && c != '\n'),
            "LOOKUP body for {} must be UTF-8 plain text without control characters:\n{}",
            name,
            body
        );
    }
}
