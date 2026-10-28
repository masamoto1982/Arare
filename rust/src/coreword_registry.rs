//! The Coreword registry: what the runtime knows about each Core Word.
//!
//! The contract half of that — stack arity, NIL policy, purity, determinism —
//! is **not** written here. It is read from `kernel::generated`, projected from
//! `spec/words.json`, and this module joins it with the runtime-local facts
//! derived from it (the flow-mass contract the analyzers read). It adds no
//! classification of its own: a label the registry does not declare is one a
//! reader cannot check against the specification.
//!
//! Two of the vocabularies that used to be declared in this file were narrower
//! than the canonical ones and mislabelled Words as a result: the hand-written
//! `NilPolicy` had 5 of the specification's 7 values, and `WordPurity` had 3 of
//! its 4 with `conditional` inexpressible, so every higher-order Word was
//! recorded as `pure`. Determinism was a `bool` where the specification
//! distinguishes `deterministic` / `stateRelative` / `hostRelative`. All three
//! now come from the generated enums, where a value the canon admits is a
//! variant by construction.

use crate::kernel::generated::{Arity, GENERATED_WORDS};
#[cfg(test)]
use std::collections::HashSet;

pub use crate::kernel::generated::{
    Determinism, FieldClosure, GeneratedWord, NilPolicy, Partiality, Purity,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorewordMetadata {
    pub name: String,
    pub family: String,
    /// Declared in `spec/words.json`.
    pub purity: Purity,
    /// Declared in `spec/words.json`, in the specification's own spelling.
    pub effects: Vec<String>,
    /// Declared in `spec/words.json`. Was a `bool`, which could not express
    /// the specification's distinction between a Word that reads runtime state
    /// (`stateRelative`) and one that reads the host (`hostRelative`).
    pub determinism: Determinism,
    pub partiality: Partiality,
    /// Declared in `spec/words.json`: whether the Word can answer a point over
    /// zero from operands that hold none (LANG.CONTRACT.FIELD).
    pub field: FieldClosure,
    /// Declared in `spec/words.json`.
    pub nil_policy: NilPolicy,
    /// Static flow-mass contract: arity / production (LANG.STACK.CONSUMPTION).
    /// Derived from the declared stack arity (LANG.MACHINE.WORD).
    pub mass: MassContract,
}

/// The registry is built by walking the *generated* inventory and joining each
/// Word's prose entry, rather than the reverse. `spec/words.json` decides which
/// Words exist; a prose entry without a declared Word is not a Word.
fn build_builtin_word_registry() -> Vec<CorewordMetadata> {
    GENERATED_WORDS.iter().map(core_word_metadata).collect()
}

/// The complete built-in word registry. Built once on first access and
/// cached for the process lifetime.
pub fn get_builtin_word_registry() -> &'static [CorewordMetadata] {
    static REGISTRY: std::sync::OnceLock<Vec<CorewordMetadata>> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(build_builtin_word_registry)
}

/// Metadata lookup by bare word name.
///
/// Built-in words form a single flat namespace, so lookup is an exact match on
/// the upper-cased name.
pub fn get_coreword_metadata(name: &str) -> Option<CorewordMetadata> {
    let upper = name.to_uppercase();
    get_builtin_word_registry()
        .iter()
        .find(|m| m.name == upper)
        .cloned()
}

/// Validates that no two registry entries share a `name`. Built-in words form
/// a single flat namespace, so a repeated name is always a genuine duplicate.
/// Used internally by tests.
#[cfg(test)]
fn collect_duplicate_entries(registry: &[CorewordMetadata]) -> Vec<String> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut dupes: Vec<String> = Vec::new();
    for word in registry {
        if !seen.insert(word.name.as_str()) {
            dupes.push(word.name.clone());
        }
    }
    dupes
}

/// A declared Word's registry row: its generated declaration joined with the
/// flow-mass contract derived from it.
fn core_word_metadata(word: &GeneratedWord) -> CorewordMetadata {
    CorewordMetadata {
        name: word.name.to_string(),
        family: word.family.as_spec_str().to_string(),
        purity: word.purity,
        effects: word.effects.iter().map(|e| e.to_string()).collect(),
        determinism: word.determinism,
        partiality: word.partiality,
        field: word.field,
        nil_policy: word.nil_policy,
        mass: mass_from_arity(word),
    }
}

// ── Static Core Word flow-mass contracts ─────────────────────────────────
//
// Invariant: flow mass is derived only from the generated stack arity; dynamic
// and control arities never acquire a guessed fixed contract.

/// Static mass contract: a word's flow-mass relationship. `consumes` operands
/// are read and removed, and `produces` results are pushed
/// (LANG.STACK.CONSUMPTION). This is the machine-readable form of the "arity /
/// consumption / production / bifurcation" declaration; the NIL-projection part
/// of LANG.MACHINE.WORD is carried by `nil_policy`.
///
/// `Dynamic` marks a data-dependent arity (e.g. `COLLECT`'s count-driven gather
/// or runtime-shaped vector ops) that is not statically pinned; the static
/// mass-conservation validator abstains on `Dynamic` words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MassContract {
    Fixed { consumes: u8, produces: u8 },
    Dynamic,
}

/// The mass contract implied by a Word's declared stack arity.
///
/// `MassContract` is the analyzers' vocabulary — they need one bit, "is this
/// arity statically pinned". An arity that is not pinned is `variable`: it is
/// decided by the data. The `control` shape, for a directive that was not a
/// stack operation at all, went with the one Word that had it (`OR-NIL`).
fn mass_from_arity(word: &GeneratedWord) -> MassContract {
    match (word.stack_inputs, word.stack_outputs) {
        (Arity::Fixed(consumes), Arity::Fixed(produces)) => {
            MassContract::Fixed { consumes, produces }
        }
        _ => MassContract::Dynamic,
    }
}

/// The canonical mass contract for a Coreword, keyed by its canonical name.
/// Unknown or non-core names conservatively return `Dynamic`.
pub fn mass_contract(name: &str) -> MassContract {
    let canonical = crate::word_name::canonical_word_name(name);
    crate::kernel::generated::generated_word(&canonical)
        .map(mass_from_arity)
        .unwrap_or(MassContract::Dynamic)
}

/// Fold a surface Word name to its dictionary key: names resolve
/// case-insensitively (LANG.DICTIONARY.RESOLUTION), and a Word has exactly one
/// name, so case is the only thing folded.
///
/// This is called on every word dispatch, so it allocates only when folding is
/// actually required: an already-uppercase ASCII name (`MAP`, `LENGTH`, most
/// User Words) is its own key and is borrowed unchanged. The borrow is gated on
/// `is_ascii()` so it never diverges from Unicode `to_uppercase` for exotic
/// input.
pub fn canonical_word_name(name: &str) -> std::borrow::Cow<'_, str> {
    if name.is_ascii() && !name.bytes().any(|b| b.is_ascii_lowercase()) {
        return std::borrow::Cow::Borrowed(name);
    }
    std::borrow::Cow::Owned(name.to_uppercase())
}

#[cfg(test)]
mod tests {
    /// Every enum the registry generates spells each Word's value exactly as
    /// `spec/words.json` writes it, so a renamed or reordered schema value
    /// cannot drift from the Rust spelling unnoticed.
    #[test]
    fn generated_spellings_are_the_specifications() {
        let spec: serde_json::Value =
            serde_json::from_str(include_str!("../../spec/words.json")).unwrap();
        for entry in spec["entries"].as_array().unwrap() {
            let name = entry["name"].as_str().unwrap();
            let word = crate::kernel::generated::generated_word(name).unwrap();
            for (key, spelled) in [
                ("partiality", word.partiality.as_spec_str()),
                ("determinism", word.determinism.as_spec_str()),
                ("vocabularyTier", word.vocabulary_tier.as_spec_str()),
                ("purity", word.purity.as_spec_str()),
                ("field", word.field.as_spec_str()),
            ] {
                assert_eq!(entry[key], spelled, "{name}.{key}");
            }
            for (axis, cost) in [
                ("steps", word.cost.steps),
                ("numeric", word.cost.numeric),
                ("collection", word.cost.collection),
            ] {
                assert_eq!(
                    entry["cost"][axis]["class"],
                    cost.class.as_spec_str(),
                    "{name}.cost.{axis}"
                );
            }
        }
    }

    use super::{
        collect_duplicate_entries, get_builtin_word_registry, get_coreword_metadata, Determinism,
        NilPolicy, Partiality, Purity,
    };

    // Verification of declared contracts and registry uniqueness.

    #[test]
    fn aq_ver_contract_a_every_word_has_contract_metadata() {
        let registry = get_builtin_word_registry();
        for word in registry {
            assert!(
                matches!(
                    word.partiality,
                    Partiality::Total | Partiality::Partial | Partiality::Projecting
                ),
                "{} must declare partiality",
                word.name
            );
            // The NIL policy's admissible values are the schema's, generated
            // into the enum, so an invalid one is unrepresentable rather than
            // merely untested — which is the whole reason the list this
            // assertion used to spell out went stale.
            assert!(
                !word.nil_policy.as_spec_str().is_empty(),
                "{} must declare nil_policy",
                word.name
            );
        }
    }

    /// Division is total (LANG.VALUES.EXACT): a quotient by zero is a point
    /// over zero, a number like any other, so `DIV` declares
    /// no projection and passes a NIL operand through exactly as `ADD` does.
    /// `SQRT` is the arithmetic Word that still projects — a negative
    /// radicand has no square root in the field — and it both passes a NIL
    /// through and projects, which is what `passthroughThenProject` says.
    #[test]
    fn aq_ver_contract_b_arithmetic_division_is_total_and_sqrt_projects() {
        let div = get_coreword_metadata("DIV").expect("DIV must be in registry");
        assert_eq!(div.partiality, Partiality::Total);
        assert_eq!(div.nil_policy, NilPolicy::Passthrough);

        let sqrt = get_coreword_metadata("SQRT").expect("SQRT must be in registry");
        assert_eq!(sqrt.partiality, Partiality::Projecting);
        assert_eq!(sqrt.nil_policy, NilPolicy::PassthroughThenProject);

        let add = get_coreword_metadata("ADD").expect("ADD must be in registry");
        assert_eq!(add.partiality, Partiality::Total);
        assert_eq!(add.nil_policy, NilPolicy::Passthrough);
    }

    #[test]
    fn aq_ver_contract_f_equality_rounding_and_arithmetic_words_are_total() {
        // LANG.CONTRACT.REGISTRY / LANG.VALUES.EXACT: equality, integer rounding
        // and the four arithmetic Words decide over every number the language
        // holds — the field, the two points over zero and `0/0` — so they have
        // no projection to declare. They pass a NIL operand through
        // (LANG.FAILURE.PASSTHROUGH) and are otherwise total. Order is the one
        // exception: `0/0` has no place in the order, so `LT`/`GT` project.
        for name in &["EQ", "FLOOR", "ROUND", "ADD", "SUB", "MUL", "DIV"] {
            let meta = get_coreword_metadata(name)
                .unwrap_or_else(|| panic!("{} must be in registry", name));
            assert_eq!(
                meta.partiality,
                Partiality::Total,
                "{} must be Total (LANG.VALUES.EXACT)",
                name
            );
            assert_eq!(
                meta.nil_policy,
                NilPolicy::Passthrough,
                "{} must be Passthrough (LANG.FAILURE.PASSTHROUGH)",
                name
            );
        }
    }

    #[test]
    fn aq_ver_contract_i_nil_diagnostic_accessors_consume_nil() {
        // LANG.VALUES.NIL / LANG.OBSERVATION.DIAGNOSIS: the two absence accessors inspect a
        // NIL rather than propagate it, so their nil_policy is ConsumesNil (the
        // OR-NIL-family "inspect or branch on NIL" classification). They are pure,
        // observations that consume what they read, like every
        // Word (LANG.STACK.CONSUMPTION), so their mass contract is a pinned 1 -> 1.
        for name in &["NIL?", "NIL-REASON"] {
            let meta = get_coreword_metadata(name)
                .unwrap_or_else(|| panic!("{} must be in registry", name));
            assert_eq!(
                meta.nil_policy,
                NilPolicy::ConsumeNil,
                "{} must be consumeNil (LANG.VALUES.NIL)",
                name
            );
            assert_eq!(
                meta.purity,
                Purity::Pure,
                "{} must be Pure (LANG.OBSERVATION.DIAGNOSIS)",
                name
            );
            // Neither raises on any operand. `NIL?` always answers a truth;
            // `NIL-REASON` answers NIL(domainMiss) for a value that is not a NIL,
            // which is a projection, so it is `projecting`.
            let partiality = if *name == "NIL?" {
                Partiality::Total
            } else {
                Partiality::Projecting
            };
            assert_eq!(meta.partiality, partiality, "{}", name);
            // The declared arity is 1 in, 1 out: the inspected value is
            // consumed and the answer takes its place. A program that needs the
            // value afterwards names it with `BIND`.
            assert_eq!(
                meta.mass,
                super::MassContract::Fixed {
                    consumes: 1,
                    produces: 1
                },
                "{} declares a pinned 1 -> 1 arity",
                name
            );
        }
    }

    /// The mass contract is the declared stack arity, read through the
    /// analyzers' coarser vocabulary. This used to assert that the adapter
    /// returned what the hand-written table said; now that there is nothing to
    /// disagree with, what is worth asserting is the projection itself — a
    /// pinned arity survives, and only the two data-dependent markers collapse.
    #[test]
    fn aq_ver_contract_f_mass_contract_projects_the_declared_arity() {
        use crate::kernel::generated::{Arity, GENERATED_WORDS};

        let mut pinned = 0_usize;
        for word in GENERATED_WORDS {
            let expected = match (word.stack_inputs, word.stack_outputs) {
                (Arity::Fixed(consumes), Arity::Fixed(produces)) => {
                    pinned += 1;
                    super::MassContract::Fixed { consumes, produces }
                }
                _ => super::MassContract::Dynamic,
            };
            assert_eq!(
                super::mass_contract(word.name),
                expected,
                "{}: mass_contract must project the declared arity",
                word.name
            );
        }
        assert!(
            pinned >= 53,
            "only {pinned} Words have a pinned arity; the projection has collapsed"
        );
    }

    /// A name that is not a Word — a retired symbol spelling included — has no
    /// contract to reach.
    #[test]
    fn aq_ver_contract_f2_mass_contract_of_a_non_word_is_dynamic() {
        assert_eq!(super::mass_contract("+"), super::MassContract::Dynamic);
        assert_eq!(
            super::mass_contract("__AJISAI_NO_SUCH_WORD__"),
            super::MassContract::Dynamic
        );
    }

    #[test]
    fn aq_ver_listing_a_no_two_entries_share_a_name() {
        let registry = get_builtin_word_registry();
        let dupes = collect_duplicate_entries(registry);
        assert!(
            dupes.is_empty(),
            "built-in word names must be unique (duplicates: {:?})",
            dupes
        );
    }

    // AQ-VER-007 — Coreword purity integrity tests.
    //
    // These tests are linked from `docs/quality/TRACEABILITY_MATRIX.md`
    // to AQ-REQ-007 ("Built-in word purity classification is self-consistent
    // with the effects and determinism each Word declares"). Test names are prefixed with their
    // verification ID so that a `cargo test aq_ver_007` invocation runs
    // the full coreword-registry coverage subset.

    #[test]
    fn aq_ver_007_a_metadata_exists_for_all_builtin_words() {
        let registry = get_builtin_word_registry();
        assert!(!registry.is_empty(), "registry must not be empty");
        for word in registry {
            assert!(!word.name.is_empty(), "name must not be empty");
            assert!(!word.family.is_empty(), "{} has empty family", word.name);
            // Purity is generated from the schema's enum, so "is this a valid
            // class" is a type-level fact now. What still needs asserting is
            // that the declaration reached the registry.
            assert!(
                !word.purity.as_spec_str().is_empty(),
                "{} has no declared purity",
                word.name
            );
        }
    }

    /// A `pure` Word declares no effects.
    ///
    /// Determinism is not asserted here: purity and determinism are separate
    /// axes in the specification, and a pure Word may still be `stateRelative`.
    #[test]
    fn aq_ver_007_b_pure_words_declare_no_effects() {
        let registry = get_builtin_word_registry();
        for word in registry.iter().filter(|w| w.purity == Purity::Pure) {
            assert!(
                word.effects.is_empty(),
                "{} pure words must have no effects",
                word.name
            );
        }
    }

    /// The `conditional` class the hand-written vocabulary could not express:
    /// a Word whose purity is that of the block it is given. It contributes no
    /// effects of its own, so it must declare none — but it is never
    /// `deterministic`, because what it runs is decided at runtime.
    #[test]
    fn aq_ver_007_b2_conditional_words_borrow_their_purity_from_their_block() {
        let conditional: Vec<&str> = get_builtin_word_registry()
            .iter()
            .filter(|w| w.purity == Purity::Conditional)
            .map(|w| w.name.as_str())
            .collect();
        assert_eq!(
            conditional,
            vec!["MAP", "FILTER", "FOLD", "SCAN", "EXEC"],
            "the conditional Words are the higher-order ones plus EXEC"
        );
        for word in get_builtin_word_registry()
            .iter()
            .filter(|w| w.purity == Purity::Conditional)
        {
            assert!(
                word.effects.is_empty(),
                "{} contributes no effects of its own",
                word.name
            );
            assert!(
                word.determinism != Determinism::Deterministic,
                "{} runs a block chosen at runtime, so it is not deterministic",
                word.name
            );
        }
    }

    #[test]
    fn aq_ver_007_c_effectful_words_declare_effects() {
        let registry = get_builtin_word_registry();
        for word in registry.iter().filter(|w| w.purity == Purity::Effectful) {
            assert!(
                !word.effects.is_empty(),
                "{} effectful words must declare effects",
                word.name
            );
        }
    }
}
