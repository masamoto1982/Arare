//! Generated Word registry — the runtime's supply of Word contract metadata.
//!
//! [`word_registry`] is generated from `spec/words.json` by
//! `scripts/generate-word-registry.mjs` (CI drift-checks it with
//! `npm run word-registry:check`). It is no longer a parallel projection kept
//! honest by equivalence tests: the runtime *reads* it. Dispatch keys off
//! [`WordId`], the NIL guard reads [`GeneratedWord::nil_policy`], and the
//! Coreword registry takes stack arity, purity and determinism from here, so
//! `spec/words.json` is the only place those facts are written down.
//!
//! The documentation prose (summary, stack effect, hover text) is projected
//! here too, and the GUI and reference consumers read it from these rows
//! directly (`builtins::lookup_builtin_spec`). No canonical fact is written
//! down twice.

mod word_registry;

pub use word_registry::{
    Arity, CostAxis, CostClass, Determinism, Family, FieldClosure, GeneratedWord, NilPolicy,
    OperandRole, Partiality, Purity, ValueDomain, VocabularyTier, WordCost, WordId,
    GENERATED_WORDS, SPECIFICATION_VERSION,
};

/// The declared contract for a Word, by canonical name.
///
/// Built-in Words form a single flat namespace, so this is an exact match on
/// the canonical (post-alias) name. Callers holding a raw source token should
/// canonicalize first.
pub fn generated_word(name: &str) -> Option<&'static GeneratedWord> {
    static BY_NAME: std::sync::OnceLock<
        crate::fast_hash::FastMap<&'static str, &'static GeneratedWord>,
    > = std::sync::OnceLock::new();
    BY_NAME
        .get_or_init(|| {
            GENERATED_WORDS
                .iter()
                .map(|word| (word.name, word))
                .collect()
        })
        .get(name)
        .copied()
}

/// The Core Word a name's text spells, in any letter case — the lookup the
/// compiler makes for every name it meets, so it reads the bytes without
/// folding them into a new string. A name with a byte outside ASCII is folded
/// as `canonical_word_name` folds it, since Unicode case mapping can turn one
/// into an ASCII letter.
pub fn core_word_spelled(bytes: &[u8]) -> Option<&'static GeneratedWord> {
    const WIDTH: usize = 16;
    static BY_KEY: std::sync::OnceLock<crate::fast_hash::FastMap<u128, &'static GeneratedWord>> =
        std::sync::OnceLock::new();
    if !bytes.is_ascii() {
        let text = std::str::from_utf8(bytes).ok()?;
        return generated_word(&crate::word_name::canonical_word_name(text));
    }
    if bytes.len() > WIDTH {
        return None;
    }
    let key = |text: &[u8]| {
        let mut key = [0u8; WIDTH];
        for (k, b) in key.iter_mut().zip(text) {
            *k = b.to_ascii_uppercase();
        }
        u128::from_le_bytes(key)
    };
    BY_KEY
        .get_or_init(|| {
            GENERATED_WORDS
                .iter()
                .map(|word| (key(word.name.as_bytes()), word))
                .collect()
        })
        .get(&key(bytes))
        .copied()
}

/// The declared contract for a Word, by its canonical identity.
///
/// `WordId` is generated from the registry itself, so — unlike the name lookup
/// above — every id has an entry by construction and the result is not an
/// `Option`. The scan is over the same 65-entry table dispatch already walks.
pub fn generated_word_by_id(id: WordId) -> &'static GeneratedWord {
    GENERATED_WORDS
        .iter()
        .find(|word| word.id == id)
        .expect("WordId is generated from GENERATED_WORDS, so every id has an entry")
}

/// The contract vocabularies serialize as the canonical spec strings, so the
/// wire form of a Word's contract is the specification's own spelling rather
/// than a Rust-side renaming of it. `as_spec_str` is generated from the schema
/// enum, so a new admitted value reaches the wire without a second edit here.
macro_rules! serialize_as_spec_str {
    ($($ty:ty),+ $(,)?) => {
        $(impl serde::Serialize for $ty {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_spec_str())
            }
        })+
    };
}

serialize_as_spec_str!(
    Family,
    NilPolicy,
    Partiality,
    FieldClosure,
    Purity,
    Determinism,
    VocabularyTier,
    CostClass
);

#[cfg(test)]
mod tests {
    use super::{generated_word, GENERATED_WORDS};
    use std::collections::BTreeSet;

    #[test]
    fn generated_inventory_matches_the_runtime_builtin_registration() {
        let generated: BTreeSet<&str> = GENERATED_WORDS.iter().map(|word| word.name).collect();
        let interp = crate::interpreter::Interpreter::new();
        let runtime: BTreeSet<&str> = interp.core_vocabulary.keys().map(String::as_str).collect();
        assert_eq!(generated, runtime);
    }

    /// The executor-key equivalence test this module used to carry is gone: it
    /// compared the generated key against a hand-written `BuiltinExecutorKey`,
    /// and that enum no longer exists — the runtime dispatches on `WordId`
    /// directly, so a Word without an executor arm is a compile error rather
    /// than a test failure. What remains worth asserting is that name lookup,
    /// the path both the guard and the registry take, reaches every Word.
    #[test]
    fn every_generated_word_is_reachable_by_name() {
        for word in GENERATED_WORDS {
            let found = generated_word(word.name)
                .unwrap_or_else(|| panic!("{} is not reachable by name", word.name));
            assert_eq!(found.id, word.id);
        }
        assert!(generated_word("__AJISAI_NO_SUCH_WORD__").is_none());
    }
}
