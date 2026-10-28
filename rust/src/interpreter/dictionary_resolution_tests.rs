//! Resolution laws for `crate::interpreter::resolve_word`.
//!
//! LANG.DICTIONARY.RESOLUTION: "The dictionary has two tiers. **Core** holds
//! the canonical Words and is sealed: a Core name cannot be redefined or
//! deleted. **User** holds definitions made by `DEF`. Resolution is a
//! deterministic function of the normalized name and the current dictionary,
//! and User never shadows Core." And: "Those two tiers are the whole
//! dictionary: a name resolves in Core or in User."
//!
//! This file used to test a different dictionary. It had named user
//! dictionaries addressed by `DICT@WORD`, `USER@DICT@WORD` and
//! `DICT@USER@DICT@WORD`; a bare name fell through three stages, and a name
//! held by several dictionaries was collapsed by content identity or reported
//! ambiguous. None of it was reachable from the language — no Word changes the
//! active dictionary, so every `DEF` wrote to the same one — and the clause
//! says there are two tiers. The laws below are what is left once there are.

use crate::interpreter::Interpreter;

fn define(interp: &mut Interpreter, name: &str, definition: &str) {
    let tokens = crate::tokenizer::tokenize(definition)
        .unwrap_or_else(|e| panic!("failed to tokenize {name}: {e}"));
    crate::interpreter::execute_def::op_def_inner(interp, name, &tokens)
        .unwrap_or_else(|e| panic!("failed to define {name}: {e}"));
}

#[tokio::test]
async fn a_core_name_resolves_to_core() {
    let interp = Interpreter::new();
    let (name, def) = interp
        .resolve_word_entry("ADD")
        .expect("ADD is a Core Word");
    assert_eq!(name.as_ref(), "ADD");
    assert!(def.is_builtin);
}

#[tokio::test]
async fn a_user_name_resolves_to_user_by_its_bare_name() {
    let mut interp = Interpreter::new();
    define(&mut interp, "INC", "1 ADD");

    let (name, def) = interp.resolve_word_entry("INC").expect("INC was defined");
    assert_eq!(name.as_ref(), "INC", "a name is the whole address");
    assert!(!def.is_builtin);
}

#[tokio::test]
async fn resolution_is_case_insensitive() {
    let mut interp = Interpreter::new();
    define(&mut interp, "INC", "1 ADD");
    for spelling in ["INC", "inc", "Inc"] {
        assert_eq!(
            interp
                .resolve_word_entry(spelling)
                .map(|(n, _)| n)
                .as_deref(),
            Some("INC"),
            "{spelling} must normalize to INC"
        );
    }
}

#[tokio::test]
async fn user_never_shadows_core() {
    // The seal is enforced at definition time, so the shadowing case is
    // unconstructible rather than merely losing the lookup.
    let mut interp = Interpreter::new();
    let tokens = crate::tokenizer::tokenize("1 ADD").expect("tokenize");
    let result = crate::interpreter::execute_def::op_def_inner(&mut interp, "ADD", &tokens);
    assert!(result.is_err(), "Core is sealed against redefinition");

    let (_, def) = interp.resolve_word_entry("ADD").expect("ADD");
    assert!(def.is_builtin, "ADD still resolves to Core");
}

#[tokio::test]
async fn an_at_sign_is_an_ordinary_name_character() {
    // `DICT@WORD` addressed a tier that no longer exists. It is now just a
    // name: held by nothing until defined, and by that word once it is.
    let mut interp = Interpreter::new();
    define(&mut interp, "INC", "1 ADD");

    for path in [
        "EXAMPLE@INC",
        "USER@EXAMPLE@INC",
        "CORE@ADD",
        "DICT@CORE@ADD",
    ] {
        assert!(
            interp.resolve_word_entry(path).is_none(),
            "{path} resolves to nothing"
        );
    }

    define(&mut interp, "EXAMPLE@INC", "2 ADD");
    let (name, def) = interp
        .resolve_word_entry("example@inc")
        .expect("a defined name resolves, case-folded like any other");
    assert_eq!(name.as_ref(), "EXAMPLE@INC");
    assert!(!def.is_builtin);
}

#[tokio::test]
async fn an_unknown_name_does_not_resolve() {
    let interp = Interpreter::new();
    assert!(interp.resolve_word_entry("NO-SUCH-WORD").is_none());
}

#[tokio::test]
async fn a_redefinition_replaces_the_user_entry() {
    let mut interp = Interpreter::new();
    define(&mut interp, "INC", "1 ADD");
    define(&mut interp, "INC", "2 ADD");

    interp.execute("5 INC").await.expect("INC runs");
    assert_eq!(
        format!("{}", interp.get_stack().last().expect("a result")),
        "7/1",
        "the second definition is the one that resolves"
    );
}

// ── the resolve cache must never outlive the dictionary it described ──────
//
// The engine caches a name's `Arc<WordDefinition>` under the dictionary
// epoch (`Interpreter::user_word`); every change to `user_words` moves the
// epoch, so a hit is the definition in force.
//
// So: the vocabulary is the one record of what a name means, and these pin
// what the cache must not be able to do to that. A stale definition served
// after a redefinition or a deletion would be a silently wrong answer, which
// is what LANG.FAILURE.TRICHOTOMY exists to rule out.

#[tokio::test]
async fn a_redefinition_is_not_served_from_the_cache() {
    let mut interp = Interpreter::new();
    define(&mut interp, "INC", "1 ADD");

    // Resolve once so the first definition is certainly cached.
    interp.execute("5 INC").await.expect("INC runs");
    assert_eq!(
        format!("{}", interp.get_stack().last().expect("a result")),
        "6/1"
    );
    interp.update_stack(Vec::new());

    define(&mut interp, "INC", "10 ADD");
    interp.execute("5 INC").await.expect("INC runs again");
    assert_eq!(
        format!("{}", interp.get_stack().last().expect("a result")),
        "15/1",
        "the cache must not serve the definition the redefinition replaced"
    );
}

/// Repeatedly, because a cache that is right once and wrong on the third
/// pass is the failure mode an epoch check exists to prevent.
#[tokio::test]
async fn every_redefinition_in_a_chain_is_the_one_that_resolves() {
    let mut interp = Interpreter::new();
    for (addend, expected) in [(1, "6/1"), (2, "7/1"), (3, "8/1"), (100, "105/1")] {
        define(&mut interp, "INC", &format!("{addend} ADD"));
        interp.execute("5 INC").await.expect("INC runs");
        assert_eq!(
            format!("{}", interp.get_stack().last().expect("a result")),
            expected,
            "after redefining INC to `{addend} ADD`"
        );
        interp.update_stack(Vec::new());
    }
}

#[tokio::test]
async fn a_deleted_word_is_not_served_from_the_cache() {
    let mut interp = Interpreter::new();
    define(&mut interp, "GONE", "1 ADD");

    interp.execute("5 GONE").await.expect("GONE runs");
    interp.update_stack(Vec::new());
    assert!(
        interp.resolve_word_entry("GONE").is_some(),
        "GONE resolves before it is deleted"
    );

    interp.execute("'GONE' DEL").await.expect("DEL runs");
    interp.update_stack(Vec::new());

    assert!(
        interp.resolve_word_entry("GONE").is_none(),
        "GONE must not resolve after deletion"
    );
    let error = interp
        .execute("5 GONE")
        .await
        .expect_err("a deleted word must not run from the cache");
    assert!(
        format!("{error:?}").contains("UnknownWord"),
        "a deleted word must be unknown, got: {error:?}"
    );
}

// ── a dispatch asks for a name it already has ─────────────────────────────
//
// A dispatch canonicalizes the name once, and every question
// it then asks is about *that* name. Two of those questions used to rebuild
// it first: the binding lookup re-uppercased a name canonicalization had
// already uppercased, and resolution answered with a freshly allocated
// `String` copy of the name it was handed. Both ran once per word dispatch,
// which is the most-travelled path the interpreter has, and neither could
// produce anything but the name already in hand.
//
// `canonical_word_name` returns one of two things, and both are
// uppercase: the input unchanged when it is ASCII with no lowercase byte
// (uppercasing that is the identity), or an owned `to_uppercase()`. Resolution looks a
// name up in Core and then in User and answers with the key it looked up —
// when the dictionary had named tiers and `DICT@WORD` paths it could answer
// with a different, qualified name, and that is what the returned copy was
// for; with two tiers there is no other answer to give.
//
// These pin the observable consequences, so the allocations cannot come back
// as the only thing holding a spelling or a name together.

/// The binding lookup reads the canonical name directly, so a name written
/// in any case must still find the binding `BIND` stored under its
/// uppercase. Lowercase is the case that would break if the canonical name
/// were *not* already uppercase.
#[tokio::test]
async fn a_binding_is_reached_through_every_spelling_of_its_name() {
    for spelling in ["value", "VALUE", "Value", "vALUe"] {
        let mut interp = Interpreter::new();
        interp
            .execute(&format!("7 'value' BIND {spelling}"))
            .await
            .unwrap_or_else(|e| panic!("binding read as `{spelling}` must run: {e:?}"));
        assert_eq!(
            format!("{}", interp.get_stack().last().expect("a result")),
            "7/1",
            "a binding must be reachable as `{spelling}`"
        );
    }
}

/// Resolution answers with the canonical name, whatever case it was asked
/// with. This is the invariant that lets the
/// resolved name be *shared* rather than copied: there is nothing in it that
/// the caller's own canonical name does not already say.
#[tokio::test]
async fn a_resolution_answers_with_the_canonical_name() {
    let mut interp = Interpreter::new();
    define(&mut interp, "INC", "1 ADD");

    for (asked, canonical) in [
        ("add", "ADD"),
        ("ADD", "ADD"),
        ("Add", "ADD"),
        ("inc", "INC"),
        ("INC", "INC"),
    ] {
        let (name, _) = interp
            .resolve_word_entry(asked)
            .unwrap_or_else(|| panic!("`{asked}` resolves"));
        assert_eq!(
            name.as_ref(),
            canonical,
            "`{asked}` must resolve under the name `{canonical}`"
        );

        // And the caching route must agree with the read-only one, since a
        // hit returns a *shared* name where a miss builds one.
        let (cached_name, _) = interp
            .resolve_word_entry(asked)
            .unwrap_or_else(|| panic!("`{asked}` resolves through the cache"));
        assert_eq!(
            cached_name.as_ref(),
            canonical,
            "`{asked}` must resolve under `{canonical}` on a cache hit too"
        );
    }
}

/// A session reset is documented as clearing every trace of the previous
/// program, and the resolve cache is such a trace. It is also the one trace
/// a reset used to leave behind *at a matching epoch*: every other way the
/// dictionary changes goes through `bump_dictionary_epoch`, which clears the
/// cache as it moves the epoch, while a reset moves neither.
#[tokio::test]
async fn a_session_reset_leaves_no_resolution_behind() {
    let mut interp = Interpreter::new();
    define(&mut interp, "INC", "1 ADD");

    // Run it so the resolution is certainly cached.
    interp.execute("5 INC").await.expect("INC runs");
    interp.update_stack(Vec::new());

    interp.execute_reset().expect("reset succeeds");

    assert!(
        interp.resolve_word_entry("INC").is_none(),
        "a user word must not resolve after a reset"
    );
    let error = interp
        .execute("5 INC")
        .await
        .expect_err("a word cleared by a reset must not run");
    assert!(
        format!("{error:?}").contains("UnknownWord"),
        "INC must be unknown after a reset, got: {error:?}"
    );

    // And Core still works, from the freshly registered vocabulary.
    interp.execute("2 3 ADD").await.expect("ADD still runs");
    assert_eq!(
        format!("{}", interp.get_stack().last().expect("a result")),
        "5/1",
        "Core must resolve against the vocabulary the reset re-registered"
    );
}
