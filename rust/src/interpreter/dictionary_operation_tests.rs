//! Test suite for interpreter dictionary operations.

use crate::interpreter::host_lookup::{resolve_host_lookup, HostLookup};
use crate::interpreter::Interpreter;

/// The host's lookup of `name`, or a panic naming what came back instead.
///
/// Looking a Word up is a host query rather than a Word, so these tests ask
/// the same way the editor does: no `execute`, no stack, no output.
fn host_lookup(interp: &Interpreter, name: &str) -> HostLookup {
    resolve_host_lookup(interp, name).unwrap_or_else(|err| panic!("lookup of {name} failed: {err}"))
}

#[tokio::test]
async fn test_cannot_override_builtin_word() {
    let mut interp = Interpreter::new();
    interp.execute("").await.unwrap();
    let result = interp.execute("[ [ 1 ] ADD ] 'GET' DEF").await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Cannot redefine Core Word"),
        "Expected error message to contain 'Cannot redefine Core Word', got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_can_override_user_word() {
    let mut interp = Interpreter::new();
    interp.execute("").await.unwrap();

    let result1 = interp.execute("[ [ 2 ] MUL ] 'DOUBLE' DEF").await;
    assert!(result1.is_ok(), "First definition should succeed");

    let result2 = interp.execute("[ [ 3 ] MUL ] 'DOUBLE' DEF").await;
    assert!(result2.is_ok(), "Overriding user word should succeed");

    let result3 = interp.execute("[ 5 ] DOUBLE").await;
    assert!(result3.is_ok(), "Executing redefined word should succeed");

    assert_eq!(interp.stack.len(), 1, "Stack should have one element");
    if let Some(val) = interp.stack.last() {
        assert!(val.is_vector(), "Expected vector result");
        assert_eq!(val.len(), 1, "Result should have one element");
        let only = val.child(0).expect("len==1 implies child(0) exists");
        {
            if let Some(f) = only.as_scalar() {
                assert_eq!(f.to_i64().unwrap(), 15, "Result should be 15");
            } else {
                panic!("Expected scalar inside vector");
            }
        }
    }
}

/// A redefinition rebinds the one User entry, and a caller follows it: the
/// caller resolves the name when it runs, so it sees the new body.
#[tokio::test]
async fn test_redefinition_rebinds_the_single_user_entry() {
    let mut interp = Interpreter::new();

    interp.execute("[ [ 1 ] ] 'SAY' DEF").await.unwrap();
    interp.execute("[ SAY ] 'GREET' DEF").await.unwrap();
    assert!(
        interp
            .dependents
            .get("SAY")
            .is_some_and(|d| d.contains("GREET")),
        "GREET depends on SAY"
    );

    // SAY is locked while GREET calls it; redefine it through the only
    // route there is.
    interp.execute("'GREET' DEL").await.unwrap();
    interp.execute("[ [ 2 ] ] 'SAY' DEF").await.unwrap();
    interp.execute("[ SAY ] 'GREET' DEF").await.unwrap();

    interp.execute("GREET").await.unwrap();
    assert_eq!(
        format!("{}", interp.get_stack().last().expect("a result")),
        "[ 2/1 ]"
    );
}

/// Identical content yields one identity regardless of name (the basis for
/// automatic deduplication on import).
#[tokio::test]
async fn test_identical_content_shares_identity() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ] 'LEAF' DEF").await.unwrap();
    interp.execute("[ [ 1 ] ] 'LEED' DEF").await.unwrap();
    interp.execute("[ [ 2 ] ] 'OTHER' DEF").await.unwrap();
    interp.rebuild_dependencies().unwrap();

    let a_leaf = interp.word_identity("LEAF").cloned();
    let b_leed = interp.word_identity("LEED").cloned();
    let b_other = interp.word_identity("OTHER").cloned();
    assert!(a_leaf.is_some(), "identity should be computed");
    assert_eq!(a_leaf, b_leed, "identical bodies must share an identity");
    assert_ne!(a_leaf, b_other, "different bodies must differ");
}

/// A word's identity depends on the content of its dependency, not on the
/// dependency's name. Two words that call differently-named but identical
/// helpers share an identity.
#[tokio::test]
async fn test_identity_is_name_independent() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ] 'LEAF' DEF").await.unwrap();
    interp.execute("[ LEAF ] 'USE-LEAF' DEF").await.unwrap();
    interp.execute("[ [ 1 ] ] 'LEED' DEF").await.unwrap();
    interp.execute("[ LEED ] 'USE-LEED' DEF").await.unwrap();

    let a_use = interp.word_identity("USE-LEAF").cloned();
    let b_use = interp.word_identity("USE-LEED").cloned();
    assert!(a_use.is_some());
    assert_eq!(
        a_use, b_use,
        "callers of identical helpers must share an identity regardless of the helper's name"
    );
}

// `test_recursive_identity_is_stable` pinned that a self-recursive word's
// identity hashed its self-cycle reproducibly. LANG.DICTIONARY.ACYCLIC's DEF-time
// acyclicity check now refuses `[ REC ] 'REC' DEF` outright, so no word's
// dependency graph can contain a cycle for `word_identity`'s cycle-hashing
// path to see; that path (Section 8.6) is unreachable but kept rather than
// torn out, matching this codebase's convention for a retired path (see
// e.g. `NilReason::EmptySequence`).

/// A name resolves when the body runs, so a word that names one defined
/// later calls it from then on: the later definition makes the existing
/// word depend on it, and the caller's identity changes with what it
/// calls (LANG.DICTIONARY.MUTATION). Dependencies used to be fixed at
/// definition time, which let `DEL` delete a word another still called
/// and gave two callers of different words one identity.
#[tokio::test]
async fn a_later_definition_of_a_named_word_becomes_a_dependency() {
    let mut interp = Interpreter::new();
    interp.execute("[ MISSING ] 'CALLER' DEF").await.unwrap();
    let before = interp
        .word_identity("CALLER")
        .cloned()
        .expect("identity should be computed for caller");
    assert!(interp.collect_dependents("MISSING").is_empty());

    interp.execute("[ [ 1 ] ] 'MISSING' DEF").await.unwrap();

    assert_eq!(
        interp.collect_dependents("MISSING"),
        ["CALLER".to_string()].into_iter().collect(),
        "the existing caller depends on the word it names once that word exists"
    );
    assert!(
        interp
            .user_words
            .get("CALLER")
            .is_some_and(|def| def.dependencies.contains("MISSING")),
        "the caller's own dependency set records it too"
    );
    let after = interp
        .word_identity("CALLER")
        .cloned()
        .expect("identity should remain computed for caller");
    assert_ne!(
        before, after,
        "the caller's identity now includes what it calls"
    );

    let refused = interp.execute("'MISSING' DEL").await;
    assert!(
        refused.is_err(),
        "a word another word calls is not deletable, however the two were ordered"
    );
    assert!(interp.user_words.contains_key("MISSING"));
    interp
        .execute("CALLER")
        .await
        .expect("the caller still runs");
}

/// Content store: textually identical bodies defined under different names
/// share a single stored body in memory rather than being duplicated.
#[tokio::test]
async fn test_identical_bodies_share_one_stored_body() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ] 'LEAF' DEF").await.unwrap();
    interp.execute("[ [ 1 ] ] 'TWIN' DEF").await.unwrap();
    interp.execute("[ [ 2 ] ] 'OTHER' DEF").await.unwrap();

    let a_leaf = interp.user_words["LEAF"].body.clone();
    let b_twin = interp.user_words["TWIN"].body.clone();
    let b_other = interp.user_words["OTHER"].body.clone();

    assert!(
        std::sync::Arc::ptr_eq(&a_leaf, &b_twin),
        "identical bodies must share one interned stored body"
    );
    assert!(
        !std::sync::Arc::ptr_eq(&a_leaf, &b_other),
        "different bodies must not share a stored body"
    );
}

/// Deferring identity recomputation during a bulk operation skips per-word
/// recomputes; a single recompute at the end (here via rebuild) restores
/// correctness. This is what makes import O(N) rather than O(N^2).
#[tokio::test]
async fn test_deferred_identity_recompute() {
    let mut interp = Interpreter::new();

    interp.defer_identity_recompute = true;
    interp.execute("[ [ 1 ] ] 'LEAF' DEF").await.unwrap();
    assert!(
        interp.word_identity("LEAF").is_none(),
        "identity recompute should be deferred"
    );

    interp.defer_identity_recompute = false;
    interp.rebuild_dependencies().unwrap();
    assert!(
        interp.word_identity("LEAF").is_some(),
        "identity should be computed once after the batch"
    );
}

/// Section 8.6 content store: bodies orphaned by a redefine are reclaimed,
/// while bodies still shared by a live definition are kept.
#[tokio::test]
async fn test_body_store_gc() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ] 'X' DEF").await.unwrap();
    assert_eq!(interp.body_store.len(), 1);

    // An identical body under another name shares one store entry.
    interp.execute("[ [ 1 ] ] 'Y' DEF").await.unwrap();
    assert_eq!(
        interp.body_store.len(),
        1,
        "identical bodies share one entry"
    );

    // Redefining X keeps [1] (still used by Y) and adds [9].
    interp.execute("[ [ 9 ] ] 'X' DEF").await.unwrap();
    assert_eq!(interp.body_store.len(), 2, "shared [1] kept, [9] added");

    // Redefining Y away orphans [1]; it is reclaimed, leaving [9] and [8].
    interp.execute("[ [ 8 ] ] 'Y' DEF").await.unwrap();
    assert_eq!(interp.body_store.len(), 2, "orphaned [1] reclaimed");
}

#[tokio::test]
async fn test_cannot_override_other_builtin_words() {
    let mut interp = Interpreter::new();

    let builtin_words = vec!["TAKE", "REVERSE", "MAP", "FILTER", "PRINT"];

    for word in builtin_words {
        let code = format!("[ [ 1 ] ADD ] '{}' DEF", word);
        let result = interp.execute(&code).await;
        assert!(
            result.is_err(),
            "Should not be able to override builtin word: {}",
            word
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Cannot redefine Core Word"),
            "Expected error for {}, got: {}",
            word,
            err_msg
        );
    }
}

#[tokio::test]
async fn test_lookup_builtin_renders_four_section_template() {
    let interp = Interpreter::new();
    // A Core Word's entry is reference text, not editable source, so it
    // comes back as documentation — the host shows it rather than loading
    // it over whatever is in the editor.
    let loaded = match host_lookup(&interp, "GET") {
        HostLookup::Documentation(text) => text,
        HostLookup::Definition(text) => {
            panic!("a Core Word must not be offered as a definition to load: {text}")
        }
    };
    for section in ["# GET", "Family:", "Summary:", "Stack Effect:"] {
        assert!(
            loaded.contains(section),
            "a Core Word's entry must include '{}' section, got: {}",
            section,
            loaded
        );
    }
    assert!(
        !loaded.contains("] DEF"),
        "a Core Word's entry should not produce a DEF expression, got: {}",
        loaded
    );
}

#[tokio::test]
async fn test_lookup_user_word_loads_def_source() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 2 ] MUL ] 'DOUBLE' DEF").await.unwrap();
    let _ = interp.collect_output();
    let loaded = match host_lookup(&interp, "DOUBLE") {
        HostLookup::Definition(text) => text,
        HostLookup::Documentation(text) => {
            panic!("a User Word must come back as editable source: {text}")
        }
    };
    assert!(
        loaded.contains("DEF") && loaded.contains("'DOUBLE'"),
        "a User Word's lookup should reconstruct DEF source, got: {}",
        loaded
    );
    assert!(
        !loaded.contains("placeholder"),
        "a User Word's lookup should not load placeholder text, got: {}",
        loaded
    );
}

fn restore_example_words(interp: &mut Interpreter, example_words: &[(&str, &str, &str)]) {
    use crate::tokenizer;

    for (name, definition, _description) in example_words {
        let tokens = tokenizer::tokenize(definition)
            .unwrap_or_else(|e| panic!("Failed to tokenize {}: {}", name, e));

        crate::interpreter::execute_def::op_def_inner(interp, name, &tokens)
            .unwrap_or_else(|e| panic!("Failed to define {}: {}", name, e));
    }

    interp
        .rebuild_dependencies()
        .expect("Failed to rebuild dependencies");
}

#[tokio::test]
async fn test_del_sample_user_words() {
    let mut interp = Interpreter::new();

    let example_words = vec![
        ("C4", "264", "純正律 C4"),
        ("D4", "C4 9 MUL 8 DIV", "純正律 D4"),
        ("E4", "C4 5 MUL 4 DIV", "純正律 E4"),
    ];
    restore_example_words(&mut interp, &example_words);

    assert!(interp.user_words.contains_key("C4"));
    assert!(interp.user_words.contains_key("D4"));
    assert!(interp.user_words.contains_key("E4"));

    let result = interp.execute("'D4' DEL").await;
    assert!(result.is_ok(), "Should delete D4: {:?}", result.err());
    assert!(!interp.user_words.contains_key("D4"));

    let result = interp.execute("'C4' DEL").await;
    assert!(result.is_err(), "Should not delete C4 (has dependents)");

    // There is no force modifier: DEL fails while any definition still
    // depends on the target (LANG.DICTIONARY.MUTATION). Removing the last
    // dependent is what makes the delete legal.
    interp
        .execute("'E4' DEL")
        .await
        .expect("Should delete the remaining dependent E4");
    let result = interp.execute("'C4' DEL").await;
    assert!(
        result.is_ok(),
        "Should delete C4 once nothing depends on it: {:?}",
        result.err()
    );
    assert!(!interp.user_words.contains_key("C4"));
}

/// `DICT@WORD` used to address a named dictionary. With two tiers a
/// Word's name is its whole address, and `@` is an ordinary name character
/// (LANG.SOURCE.LEXICAL allocates none): `EXAMPLE@D4` is a name that
/// nothing holds until something defines it, and then it is that word.
#[tokio::test]
async fn test_an_at_sign_makes_an_ordinary_name() {
    let mut interp = Interpreter::new();

    let example_words = vec![("D4", "264", "test word")];
    restore_example_words(&mut interp, &example_words);

    let result = interp.execute("'EXAMPLE@D4' DEL").await;
    let message = result.expect_err("nothing holds that name").to_string();
    assert!(message.contains("not defined"), "{message}");
    assert!(
        interp.user_words.contains_key("D4"),
        "the word is untouched"
    );

    interp
        .execute("[ 7 ] 'EXAMPLE@D4' DEF EXAMPLE@D4")
        .await
        .unwrap();
    assert_eq!(
        format!("{}", interp.get_stack().last().expect("a result")),
        "7/1"
    );
    interp.execute("'EXAMPLE@D4' DEL 'D4' DEL").await.unwrap();
    assert!(!interp.user_words.contains_key("EXAMPLE@D4"));
    assert!(!interp.user_words.contains_key("D4"));
}

#[tokio::test]
async fn test_execute_restored_example_words() {
    let mut interp = Interpreter::new();

    let example_words = vec![
        ("C4", "264", "純正律 C4"),
        ("D4", "C4 9 MUL 8 DIV", "純正律 D4"),
    ];
    restore_example_words(&mut interp, &example_words);

    let result = interp.execute("C4").await;
    assert!(
        result.is_ok(),
        "Executing C4 should succeed: {:?}",
        result.err()
    );
    assert_eq!(interp.stack.len(), 1);

    let result = interp.execute("D4").await;
    assert!(
        result.is_ok(),
        "Executing D4 should succeed: {:?}",
        result.err()
    );
    assert_eq!(interp.stack.len(), 2);
}
#[tokio::test]
async fn test_example_words_scalar_output() {
    let mut interp = Interpreter::new();

    let example_words = vec![
        ("C4", "264", "純正律 C4"),
        ("D4", "C4 9 MUL 8 DIV", "純正律 D4"),
    ];
    restore_example_words(&mut interp, &example_words);
    let _ = interp.collect_output();

    interp.execute("C4").await.unwrap();
    assert_eq!(interp.stack.len(), 1);
    if let Some(val) = interp.stack.last() {
        assert!(
            val.as_scalar().is_some(),
            "C4 should push a scalar, not a vector"
        );
        assert_eq!(val.as_scalar().unwrap().to_i64().unwrap(), 264);
    }

    interp.execute("D4").await.unwrap();
    assert_eq!(interp.stack.len(), 2);
    if let Some(val) = interp.stack.last() {
        assert!(
            val.as_scalar().is_some(),
            "D4 should push a scalar, not a vector"
        );
        assert_eq!(val.as_scalar().unwrap().to_i64().unwrap(), 297);
    }
}

#[tokio::test]
async fn test_builtin_symbols_remain_strings_in_vector() {
    let mut interp = Interpreter::new();

    let result = interp.execute("[ [ 2 ] MUL ] 'DOUBLE' DEF").await;
    assert!(
        result.is_ok(),
        "Code block DEF should work: {:?}",
        result.err()
    );

    let result = interp.execute("[ 5 ] DOUBLE").await;
    assert!(
        result.is_ok(),
        "Executing DOUBLE should succeed: {:?}",
        result.err()
    );
    if let Some(val) = interp.stack.last() {
        assert!(val.is_vector(), "Expected vector result");
        assert_eq!(val.len(), 1);
        let only = val.child(0).expect("len==1 implies child(0) exists");
        assert_eq!(only.as_scalar().unwrap().to_i64().unwrap(), 10);
    }
}
#[tokio::test]
async fn test_def_with_vector_duality() {
    let mut interp = Interpreter::new();

    let result = interp.execute("[ [ 2 ] MUL ] 'DOUBLE' DEF").await;
    assert!(
        result.is_ok(),
        "DEF with vector should succeed: {:?}",
        result
    );

    let result = interp.execute("[ 5 ] DOUBLE").await;
    assert!(
        result.is_ok(),
        "Executing DOUBLE should succeed: {:?}",
        result
    );

    assert_eq!(interp.stack.len(), 1, "Stack should have 1 element");
    if let Some(val) = interp.stack.last() {
        assert!(val.is_vector(), "Expected vector result");
        assert_eq!(val.len(), 1, "Result should have one element");
        let only = val.child(0).expect("len==1 implies child(0) exists");
        {
            if let Some(f) = only.as_scalar() {
                assert_eq!(f.to_i64().unwrap(), 10, "Result should be 10");
            } else {
                panic!("Expected scalar inside vector");
            }
        }
    }
}
// ── A word's own self-reference must not lock it ──────────────────────
// A recursive word used to depend on itself, and once that self-edge was
// in the dependency index, the guard that protects *callers* from losing
// a definition fired on the word itself: `DEF` and `DEL` both refused
// with "referenced by FIB — delete those words first", naming the word
// being deleted. `collect_external_dependents` (`resolve_word.rs`) still
// excludes a word's own self-edge from that guard, but LANG.DICTIONARY.ACYCLIC's
// DEF-time acyclicity check now refuses a self-referential definition
// outright, so no word can carry a self-edge for the exclusion to matter
// to any more; the two tests that pinned it (redefining and deleting a
// self-recursive word) were removed along with the scenario. The
// protection this exclusion carves an exception out of — a *different*
// word's reference still locking one — remains exercised below.

#[tokio::test]
async fn another_word_still_locks_a_referenced_word() {
    let mut interp = Interpreter::new();
    interp.execute("[ 1 ADD ] 'W' DEF").await.unwrap();
    interp.execute("[ W ] 'CALLER' DEF").await.unwrap();

    let err = interp
        .execute("[ 2 ADD ] 'W' DEF")
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("CALLER"),
        "redefinition must name the external dependent: {err}"
    );

    let err = interp.execute("'W' DEL").await.unwrap_err().to_string();
    assert!(
        err.contains("CALLER"),
        "deletion must name the external dependent: {err}"
    );
}

// ── A looked-up definition must round-trip ────────────────────────────
// Loading a user word, editing it, and defining it again is the ordinary
// way to correct a definition. That only works if what comes back is source
// `DEF` accepts — `[ ]` now that `{ }` is retired, since `DEF` takes any
// Vector as its body and a `|` clause tokenizes the same either way.

fn lookup_source(interp: &Interpreter, name: &str) -> String {
    match host_lookup(interp, name) {
        HostLookup::Definition(text) => text,
        HostLookup::Documentation(text) => {
            panic!("a User Word must come back as editable source: {text}")
        }
    }
}

#[tokio::test]
async fn lookup_of_a_user_word_round_trips_through_def() {
    let mut interp = Interpreter::new();
    interp.execute("[ 2 MUL ] 'DBL' DEF").await.unwrap();
    let loaded = lookup_source(&interp, "DBL");
    assert!(
        loaded.starts_with('['),
        "the body must round-trip as a Vector literal: {loaded}"
    );

    // Running the loaded text redefines the word, and it still computes.
    interp.execute(&loaded).await.unwrap();
    interp.execute("5 DBL").await.unwrap();
    assert_eq!(format!("{}", interp.stack.last().unwrap()), "10/1");
}

#[tokio::test]
async fn lookup_of_a_branching_word_round_trips_through_def() {
    let mut interp = Interpreter::new();
    interp
        .execute("[ 'N' BIND\n[ 'small' ] [ 'big' ]\nN [ 5 ] LT\nSELECT ] 'SIZE' DEF")
        .await
        .unwrap();
    let loaded = lookup_source(&interp, "SIZE");
    assert!(
        loaded.contains("SELECT"),
        "the branch must survive the round trip: {loaded}"
    );

    interp.execute(&loaded).await.unwrap();
    interp.execute("[ 7 ] SIZE").await.unwrap();
    assert_eq!(format!("{}", interp.stack.last().unwrap()), "[ 'big' ]");
}

#[tokio::test]
async fn a_contract_directive_becomes_the_defined_words_description() {
    let mut interp = Interpreter::new();
    interp
        .execute("#:contract INC inputs=1 outputs=1 field=closed\n[ [ 1 ] ADD ] 'INC' DEF")
        .await
        .unwrap();
    assert_eq!(
        interp.lookup_word_description("INC").as_deref(),
        Some("inputs=1 outputs=1 field=closed")
    );
}

#[tokio::test]
async fn a_contract_directive_survives_to_a_def_in_a_later_execute_call() {
    // The Playground runs one submission at a time, so a `#:contract`
    // line typed before the `DEF` it documents can arrive as its own
    // `execute()` call rather than sharing one with the `DEF`.
    let mut interp = Interpreter::new();
    interp
        .execute("#:contract INC inputs=1 outputs=1 field=closed")
        .await
        .unwrap();
    assert_eq!(interp.lookup_word_description("INC"), None);
    interp.execute("[ [ 1 ] ADD ] 'INC' DEF").await.unwrap();
    assert_eq!(
        interp.lookup_word_description("INC").as_deref(),
        Some("inputs=1 outputs=1 field=closed")
    );
}

#[tokio::test]
async fn a_word_defined_without_a_contract_directive_has_no_description() {
    let mut interp = Interpreter::new();
    interp.execute("[ [ 1 ] ADD ] 'INC' DEF").await.unwrap();
    assert_eq!(interp.lookup_word_description("INC"), None);
}

#[tokio::test]
async fn redefining_without_a_new_directive_drops_the_old_description() {
    let mut interp = Interpreter::new();
    interp
        .execute("#:contract INC inputs=1 outputs=1 field=closed\n[ [ 1 ] ADD ] 'INC' DEF")
        .await
        .unwrap();
    interp.execute("[ [ 2 ] ADD ] 'INC' DEF").await.unwrap();
    assert_eq!(interp.lookup_word_description("INC"), None);
}
