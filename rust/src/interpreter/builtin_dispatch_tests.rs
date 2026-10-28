//! A Core Word is dispatched from the registry entry its definition carries,
//! not from a second lookup by name.
//!
//! Dispatch used to resolve a name to an `Arc<WordDefinition>` and
//! then, for a Core Word, used to find the *same* Word again by string:
//! `execute_builtin` re-canonicalized the name and `generated_word` scanned the
//! 65-entry registry comparing names. That ran once per element — 220,000
//! `memcmp` calls, 6.25% of the instructions of a 20,000-element `[ ABS ] MAP`
//! — to re-answer what resolution had just answered.
//!
//! `WordDefinition::generated` holds the entry instead, because
//! `builtins::register` has it in hand when it builds the definition. These pin
//! what has to be true for that to be sound: the entry is present for every
//! Core Word and is the *right* one, a User Word has none, the two routes run
//! the same Word the same way (LANG.AUTHORITY.FREEDOM — the choice of route is
//! unobservable), and the name route still answers for the cases that reach it.

use crate::interpreter::Interpreter;
use crate::kernel::generated::GENERATED_WORDS;

/// The stack as a program can see it: each value rendered.
fn rendered(interp: &Interpreter) -> Vec<String> {
    interp
        .get_stack()
        .iter()
        .map(|value| value.to_string())
        .collect()
}

/// Present, and pointing at the Word it is registered under. A definition
/// carrying the *wrong* entry would dispatch a different primitive under a
/// correct-looking name, which no other test would catch.
#[test]
fn every_core_word_carries_its_own_registry_entry() {
    let interp = Interpreter::new();
    for word in GENERATED_WORDS {
        let (name, def) = interp
            .resolve_word_entry(word.name)
            .unwrap_or_else(|| panic!("Core Word `{}` must resolve", word.name));
        assert_eq!(name.as_ref(), word.name);
        let carried = def
            .generated
            .unwrap_or_else(|| panic!("`{}` must carry its registry entry", word.name));
        assert_eq!(
            carried.name, word.name,
            "`{}` carries the entry for `{}`",
            word.name, carried.name
        );
        assert_eq!(carried.id, word.id, "`{}` carries a foreign id", word.name);
        assert!(
            def.body.is_empty(),
            "a Core Word has no body, which is what selects this route"
        );
    }
}

/// `DEF` cannot define a Core Word, so a User Word has no entry to carry —
/// and must not appear to. One that did would be dispatched as a primitive
/// instead of having its body run.
#[tokio::test]
async fn a_user_word_carries_no_registry_entry() {
    let mut interp = Interpreter::new();
    interp
        .execute("[ 1 ADD ] 'INC' DEF")
        .await
        .expect("INC defines");
    let (_, def) = interp.resolve_word_entry("INC").expect("INC resolves");
    assert!(
        def.generated.is_none(),
        "a User Word must not carry a registry entry"
    );
}

/// The two routes into a primitive must be the same step. Compared at the
/// point where they actually differ — the carried entry versus the registry
/// scan — rather than through a whole dispatch. Both a value operand
/// and a NIL one, since the declared-NIL contract is applied inside the
/// shared step and a route that skipped it would answer differently for the
/// same program.
#[tokio::test]
async fn the_carried_entry_and_the_registry_scan_are_the_same_step() {
    for operand in ["2 3", "NIL 3", "2 NIL"] {
        for word in GENERATED_WORDS {
            let mut by_entry = Interpreter::new();
            by_entry.execute(operand).await.expect("operands push");
            let via_entry = by_entry.execute_generated_word(word);

            let mut by_name = Interpreter::new();
            by_name.execute(operand).await.expect("operands push");
            let via_name = by_name.execute_builtin_direct(word.name);

            assert_eq!(
                via_entry.is_ok(),
                via_name.is_ok(),
                "`{operand} {}` must succeed or fail the same way by either route \
                 (entry: {via_entry:?}, name: {via_name:?})",
                word.name
            );
            assert_eq!(
                format!("{via_entry:?}"),
                format!("{via_name:?}"),
                "`{operand} {}` must report the same outcome by either route",
                word.name
            );
            // Rendered, not structural: what a program can see is the
            // comparison this gate is about (LANG.VALUES.DENOTATION).
            assert_eq!(
                rendered(&by_entry),
                rendered(&by_name),
                "`{operand} {}` must leave the same observable stack by either route",
                word.name
            );
        }
    }
}

/// The `None` arm is not dead code. A definition with an empty body and no
/// registry entry takes it, and an unknown name must still be reported
/// rather than silently doing nothing.
#[tokio::test]
async fn an_unknown_name_is_still_reported() {
    let mut interp = Interpreter::new();
    let error = interp
        .execute("1 NOSUCHWORD")
        .await
        .expect_err("an unknown name must not run");
    assert!(
        format!("{error:?}").contains("UnknownWord"),
        "expected UnknownWord, got: {error:?}"
    );
    assert!(
        interp.execute_builtin_direct("NOSUCHWORD").is_err(),
        "the by-name route must refuse an unknown name too"
    );
}
