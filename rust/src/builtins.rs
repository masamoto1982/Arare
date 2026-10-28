//! The Core Words' runtime view: registration into the dictionary, lookup
//! by name, and the host lookup text rendered from the registry entry.
//!
//! Everything here is read from the generated registry
//! (`kernel::generated`, projected from `spec/words.json`); nothing is a
//! second source of Core Word facts.

use crate::coreword_registry::{FieldClosure, Partiality};
use crate::kernel::generated::{
    generated_word, GeneratedWord, OperandRole, VocabularyTier, GENERATED_WORDS,
};
use crate::types::WordDefinition;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[cfg(test)]
mod builtin_word_details_tests;

/// Register every Core Word the specification declares.
///
/// The inventory is the generated registry: `spec/words.json` decides which
/// Words exist, and each is registered with its generated hover title as its
/// description.
pub fn register_builtins<S: std::hash::BuildHasher>(
    dictionary: &mut HashMap<String, Arc<WordDefinition>, S>,
) {
    for word in GENERATED_WORDS {
        dictionary.insert(
            word.name.to_string(),
            Arc::new(WordDefinition {
                body: std::sync::Arc::from([]),
                is_builtin: true,
                description: Some(word.hover_summary.to_string()),
                dependencies: HashSet::new(),
                text_references: HashSet::new(),
                registration_order: 0,
                code: Default::default(),
                generated: Some(word),
            }),
        );
    }
}

/// The registry entry for a Core Word named in any accepted spelling, or
/// `None` for a name the registry does not know.
///
/// There used to be a `BuiltinSpec` here: seven of the entry's fields copied
/// one for one into a second `OnceLock`'d table, scanned linearly, with a
/// test asserting every field equal to the generated one. The entry is the
/// view.
pub fn lookup_builtin_spec(name: &str) -> Option<&'static GeneratedWord> {
    generated_word(&crate::word_name::canonical_word_name(name))
}

/// WASM/GUI tuple shape: `(name, hover_summary, hover_syntax)`.
/// Position 1 (`hover_summary`) is the native button-title text;
/// position 2 (`hover_syntax`) is the inline word-info preview.
/// See three-layer-documentation-model.md §4.
///
/// Consumed only by the wasm bindings (feature = "wasm").
#[cfg_attr(not(feature = "wasm"), allow(dead_code))]
pub fn collect_core_builtin_definitions() -> Vec<(&'static str, &'static str, &'static str)> {
    GENERATED_WORDS
        .iter()
        .map(|word| (word.name, word.hover_summary, word.hover_syntax))
        .collect()
}

/// Render the host lookup text for a Core Word: the base sections (Family /
/// Summary / Stack Effect), the Word's one correct call (Examples), and the sections
/// derived from the LANG.CONTRACT.REGISTRY contract metadata (Failure baseline, Side
/// Effects, Vocabulary) — derived so they can never drift from the
/// registry. See docs/dev/three-layer-documentation-model.md §3.
pub fn lookup_builtin_detail(name: &str) -> String {
    let Some(word) = lookup_builtin_spec(name) else {
        return format!(
            "# {}\n\nNo documentation found for this word.\n",
            crate::word_name::canonical_word_name(name)
        );
    };

    let mut out = render_sections(
        word.name,
        word.family.as_spec_str(),
        word.summary,
        word.stack_effect,
    );

    // One source for what a Word does: the summary above, from
    // `spec/words.json`, carries the prose and the tested examples. The
    // example here is the one correct call a diagnosis quotes.
    out.push('\n');
    out.push_str("Examples:\n");
    push_indented(&mut out, word.hover_syntax, "  ");

    out.push('\n');
    out.push_str("Failure:\n");
    push_indented(&mut out, &derive_failure_text(word), "  ");

    out.push('\n');
    out.push_str("Side Effects:\n");
    push_indented(&mut out, &derive_side_effects_text(word), "  ");

    out.push('\n');
    out.push_str("Vocabulary:\n");
    push_indented(&mut out, &derive_vocabulary_text(word), "  ");

    out
}

/// Where the Word sits in the public Core, read from the generated registry.
/// Core is one flat sealed dictionary, so this states a design classification
/// and never a namespace: a Standard Word is reached by its plain name exactly
/// as a Semantic Kernel Word is, and carries the same contract detail.
fn derive_vocabulary_text(word: &GeneratedWord) -> String {
    match (word.vocabulary_tier, word.standard_kind) {
        (VocabularyTier::Kernel, _) => {
            "Core Word, Semantic Kernel: it builds or observes a value domain,\nor is the one explicit operation for its capability.".to_string()
        }
        (VocabularyTier::Standard, Some(kind)) => format!(
            "Core Word, Standard vocabulary ({kind}): one canonical contract for\na frequent concept, on the same terms as a Kernel Word."
        ),
        (VocabularyTier::Standard, None) => "Core Word, Standard vocabulary.".to_string(),
    }
}

/// Failure baseline derived from the LANG.CONTRACT.REGISTRY contract metadata. The wording
/// follows the NIL Projection Rule: well-formed operations that cannot
/// produce a value project onto NIL with a reason, while malformed usage
/// raises an error.
///
/// The NIL sentence is derived from the *declared* policy in
/// `spec/words.json`, so what a reader is told about NIL and what the dispatch
/// guard enforces are the same fact read twice, not two claims that can drift.
fn derive_failure_text(word: &GeneratedWord) -> String {
    let mut lines: Vec<&str> = Vec::new();
    match word.partiality {
        Partiality::Total => lines.push(
            "Total: an operand of the kind it reads always produces a result;\nan operand of another kind raises the error its contract names.",
        ),
        Partiality::Projecting => lines.push(
            "Well-formed input that cannot produce a value yields a NIL\nwith a reason; an operand of the wrong kind raises an error.",
        ),
        Partiality::Partial => lines.push(
            "May raise even on operands of the right kind: the block it runs,\nor the dictionary it changes, can refuse.",
        ),
    }
    if word.field == FieldClosure::Leaving {
        lines.push(
            "Leaves the field: from operands holding no point over zero it can\nanswer 1/0 or 0/0, where the field laws stop (LANG.CONTRACT.FIELD).",
        );
    }
    // One line per role the Word has (LANG.FAILURE.PASSTHROUGH), so the
    // hover says what a NIL does in each operand, not a single summary that
    // is true of some of them.
    let has = |role: OperandRole| word.operand_roles.contains(&role);
    if has(OperandRole::Data) || has(OperandRole::Leaf) {
        lines.push("A NIL data operand passes through as the result, keeping its reason.");
    }
    if has(OperandRole::Element) {
        lines.push(
            "A NIL it only carries (a stored, bound or compared value) is an ordinary value.",
        );
    }
    if has(OperandRole::Control) {
        lines.push("A NIL where a block, name or message belongs is an error.");
    }
    if has(OperandRole::Leaf) || has(OperandRole::Truth) {
        lines.push("A Vector or Record where one value is read applies the word to each element.");
    }
    if has(OperandRole::Truth) {
        lines.push(
            "A dominating definite operand (FALSE for AND) absorbs a NIL operand into that definite result; otherwise a NIL operand yields NIL as UNKNOWN.",
        );
    }
    lines.join("\n")
}

/// Side Effects derived from the LANG.CONTRACT.REGISTRY `effects` list declared in
/// `spec/words.json`. Each declared effect maps to one user-facing sentence;
/// `effect_sentence` returns `None` for a name it does not know, which
/// `builtin_word_details_tests.rs` turns into a failure rather than letting the
/// raw protocol name reach a reader.
fn derive_side_effects_text(word: &GeneratedWord) -> String {
    if word.effects.is_empty() {
        return "None.".to_string();
    }
    let mut sentences: Vec<&str> = Vec::new();
    for effect in word.effects {
        let sentence = effect_sentence(effect).unwrap_or(effect);
        if !sentences.contains(&sentence) {
            sentences.push(sentence);
        }
    }
    sentences.join("\n")
}

/// The user-facing sentence for a declared effect name.
pub(crate) fn effect_sentence(effect: &str) -> Option<&'static str> {
    match effect {
        "consoleWrite" => Some("Writes to the output area."),
        "dictionaryWrite" => Some("Modifies the dictionary."),
        "dictionaryDelete" => Some("Removes a word from the dictionary."),
        _ => None,
    }
}

fn render_sections(name: &str, family: &str, summary: &str, stack_effect: &str) -> String {
    let mut out = format!("# {}\n\n", name);

    out.push_str("Family:\n");
    push_indented(&mut out, family, "  ");
    out.push('\n');

    out.push_str("Summary:\n");
    push_indented(&mut out, summary, "  ");
    out.push('\n');

    out.push_str("Stack Effect:\n");
    push_indented(&mut out, stack_effect, "  ");

    out
}

fn push_indented(out: &mut String, body: &str, indent: &str) {
    for line in body.split('\n') {
        out.push_str(indent);
        out.push_str(line);
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::lookup_builtin_detail;

    #[test]
    fn builtin_specs_are_not_named_by_symbols() {
        let forbidden = [
            "+", "-", "*", "/", "%", "=", "<", "<=", ">", ">=", "<>", ".", "..", ",", ",,", "~",
            "!", "'", "|", "?", "^",
        ];

        for word in crate::kernel::generated::GENERATED_WORDS {
            assert!(
                !forbidden.contains(&word.name),
                "the Core vocabulary must not contain a symbol/helper word: {}",
                word.name
            );
        }
    }

    #[test]
    fn builtin_specs_contain_canonical_core_words() {
        let required = ["ADD", "SUB", "MUL", "DIV", "EQ", "LT", "GT", "SQRT", "SORT"];

        for name in required {
            assert!(
                super::lookup_builtin_spec(name).is_some(),
                "missing canonical core word: {}",
                name
            );
        }
    }

    #[test]
    fn builtin_specs_have_required_lookup_content() {
        for spec in crate::kernel::generated::GENERATED_WORDS {
            assert!(!spec.summary.is_empty(), "{} missing summary", spec.name);
            assert!(
                !spec.family.as_spec_str().is_empty(),
                "{} missing family",
                spec.name
            );
            assert!(
                !spec.stack_effect.is_empty(),
                "{} missing stack_effect",
                spec.name
            );
        }
    }

    #[test]
    fn builtin_specs_stack_effect_grammar() {
        for spec in crate::kernel::generated::GENERATED_WORDS {
            let s = spec.stack_effect;
            let is_literal_no_op =
                s == "no values popped or pushed" || s == "operands preserved; result pushed";
            if is_literal_no_op {
                continue;
            }
            assert!(
                s.contains("->"),
                "{} stack_effect missing '->' arrow: {:?}",
                spec.name,
                s
            );
        }
    }

    #[test]
    fn builtin_specs_lookup_text_is_utf8_plain_text() {
        let check = |label: &str, name: &str, text: &str| {
            assert!(
                !text.chars().any(|c| c.is_control() && c != '\n'),
                "{} field of {} must be UTF-8 plain text without control characters; got: {:?}",
                label,
                name,
                text
            );
        };
        for spec in crate::kernel::generated::GENERATED_WORDS {
            check("summary", spec.name, spec.summary);
            check("stack_effect", spec.name, spec.stack_effect);
            check("family", spec.name, spec.family.as_spec_str());
        }
    }

    /// The Failure section used to promise `SORT` "always produces a result",
    /// which is false of a vector holding a string. Totality is stated over
    /// the kind of operand the Word reads, which is what makes it true.
    #[test]
    fn totality_is_stated_over_the_kind_the_word_reads() {
        let text = lookup_builtin_detail("REVERSE");
        assert!(
            text.contains("Total: an operand of the kind it reads always produces a result"),
            "{text}"
        );
    }

    /// A raw Rust escape once reached a reader: SQRT's Role said
    /// `the multiquadratic \u{221a}d` verbatim.
    #[test]
    fn no_lookup_text_leaks_a_rust_unicode_escape() {
        for word in crate::kernel::generated::GENERATED_WORDS {
            let text = lookup_builtin_detail(word.name);
            assert!(
                !text.contains("\\u{"),
                "{}: LOOKUP text carries a raw escape:\n{text}",
                word.name
            );
        }
    }
}
