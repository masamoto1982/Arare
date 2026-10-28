//! Test suite for `crate::word_name` canonicalization.

use crate::interpreter::Interpreter;

/// A Word has exactly one name. The symbol spellings `ADD SUB MUL DIV EQ LT GT` were
/// once aliases of `ADD SUB MUL DIV EQ LT GT`; they were deleted so the
/// vocabulary has one spelling per concept, and are ordinary names now:
/// canonicalization leaves them unchanged and, undefined, they are unknown
/// words like any other.
#[tokio::test]
async fn former_symbol_aliases_are_ordinary_names() {
    use crate::word_name::canonical_word_name;
    for symbol in ["+", "-", "*", "/", "=", "<", ">"] {
        assert_eq!(canonical_word_name(symbol), symbol);
        let mut interp = Interpreter::new();
        let err = interp
            .execute(&format!("1 2 {symbol}"))
            .await
            .expect_err("an undefined name is an unknown word");
        assert!(
            err.to_string().contains("Unknown word"),
            "`{symbol}`: {err}"
        );
    }
}

/// `OR-NIL` has no symbol or legacy-name sugar: `^` and `VENT` (the former
/// canonical spelling, retired with the water-metaphor decoupling) are
/// ordinary, unrecognized names now — canonicalization leaves them
/// unchanged rather than folding them onto `OR-NIL`.
#[tokio::test]
async fn caret_and_legacy_vent_spelling_are_no_longer_aliases() {
    use crate::word_name::canonical_word_name;
    assert_eq!(canonical_word_name("^"), "^");
    assert_eq!(canonical_word_name("VENT"), "VENT");
}

/// `?` is the host's spelling of a lookup, not a Word alias, so canonicalization
/// leaves it alone. It used to fold to `LOOKUP`; if it still did, a User Word
/// named `?` would resolve to a Word that no longer exists.
#[tokio::test]
async fn the_lookup_mark_is_not_a_word_alias() {
    use crate::word_name::canonical_word_name;
    assert_eq!(canonical_word_name("?"), "?");
}

/// Lexical / structural surface forms are documented as named concepts but are
/// never runtime words: `canonical_word_name` must not return their
/// concept names. (See `crate::surface_forms`.)
#[tokio::test]
async fn surface_form_concepts_are_not_runtime_canonicalizations() {
    use crate::surface_forms::lookup_surface_form;
    use crate::word_name::canonical_word_name;

    assert_eq!(lookup_surface_form("#").unwrap().concept, "COMMENT-LINE");
    assert_eq!(lookup_surface_form("[").unwrap().concept, "BEGIN-VECTOR");

    assert_ne!(canonical_word_name("#"), "COMMENT-LINE");
    assert_ne!(canonical_word_name("["), "BEGIN-VECTOR");
    assert_ne!(canonical_word_name("]"), "END-VECTOR");
    assert_ne!(canonical_word_name("'"), "STRING-QUOTE");
}

/// 手3 (dispatch de-allocation): canonicalization must not allocate on the
/// dominant dispatch case — an already-uppercase word borrows the input slice.
/// Only a name that genuinely needs case folding takes the owned path.
#[test]
fn canonicalize_borrows_without_allocating_on_hot_paths() {
    use crate::word_name::canonical_word_name;
    use std::borrow::Cow;

    // Already-uppercase word → input borrowed unchanged.
    for word in ["MAP", "LENGTH", "TIME@NOW", "USER-WORD", "+"] {
        let canon = canonical_word_name(word);
        assert!(
            matches!(canon, Cow::Borrowed(_)),
            "uppercase word {word} must borrow"
        );
        assert_eq!(canon, word);
    }

    // Mixed/lowercase requires folding → owned, and folds correctly.
    let folded = canonical_word_name("map");
    assert!(matches!(folded, Cow::Owned(_)), "lowercase must fold owned");
    assert_eq!(folded, "MAP");
}

/// Canonicalization at the edges: an empty name, a non-ASCII one, a name
/// that starts with a symbol, and one that is alphanumeric-with-a-digit.
#[test]
fn canonicalization_is_exact_at_its_edges() {
    use crate::word_name::canonical_word_name;

    for (name, expected) in [
        ("", ""),
        ("ADD", "ADD"),
        ("add", "ADD"),
        ("A1", "A1"),
        ("1ADD", "1ADD"),
        ("+ADD", "+ADD"),
        ("<X", "<X"),
        ("日本語", "日本語"),
        ("TIME@NOW", "TIME@NOW"),
    ] {
        assert_eq!(
            canonical_word_name(name),
            expected,
            "`{name}` must canonicalize to `{expected}`"
        );
    }
}
