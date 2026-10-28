//! The termination argument, held against the implementation.
//!
//! `spec/termination.json` states why every Ajisai evaluation is finite: the
//! places evaluation descends into more evaluation, what strictly decreases at
//! each one, and the rules underneath — that a Word runs only when a Symbol
//! naming it is written in source, so the DEF-time acyclicity check sees every
//! call a body can make, and that a block built from data never runs inside
//! another, so no block runs a copy of itself. `scripts/check-termination.mjs` keeps that file closed
//! against the vocabulary; this file runs it.
//!
//! The acyclicity check is the load-bearing part and had no test of its own: no
//! test named it, `tests/conformance` had no case for it, and its only witness
//! was the simplest direct self-reference. Soundness alone is not the property
//! either — a check that refused everything would be sound and would make the
//! language useless — so completeness is tested at the same time, over random
//! definition graphs.

use ajisai_core::interpreter::Interpreter;
use ajisai_core::ErrorCategory;
use proptest::prelude::*;
use serde_json::Value as Json;
use std::collections::{HashMap, HashSet};

fn spec() -> Json {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../spec/termination.json");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read the termination argument at {path}: {e}"));
    serde_json::from_str(&text).expect("spec/termination.json is valid JSON")
}

/// Run a program on a fresh interpreter, returning the error text if it failed.
fn run(src: &str) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio current-thread runtime");
    rt.block_on(async {
        let mut interp = Interpreter::new();
        interp
            .execute(src)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
}

/// Run a program on a fresh interpreter, returning the outcome category of its
/// failure if it failed.
fn run_category(src: &str) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio current-thread runtime");
    rt.block_on(async {
        let mut interp = Interpreter::new();
        interp.execute(src).await.map(|_| ()).map_err(|e| {
            ErrorCategory::from_error(&e)
                .map(|c| c.as_protocol_str().to_string())
                .unwrap_or_else(|| e.to_string())
        })
    })
}

fn witnesses<'a>(node: &'a Json, path: &[&str]) -> &'a Vec<Json> {
    let mut current = node;
    for key in path {
        current = &current[key];
    }
    current
        .as_array()
        .unwrap_or_else(|| panic!("expected an array of witnesses at {path:?}"))
}

fn source_of(w: &Json) -> &str {
    w["source"].as_str().expect("witness source")
}

/// Every recursion site's witness runs, and finishes without any of the
/// ceilings the argument says are not what stops it. A site whose witness
/// needed a ceiling would mean the descent it names is not actually bounded.
#[test]
fn every_recursion_site_terminates_without_a_ceiling() {
    let s = spec();
    let sites = s["recursionSites"].as_array().expect("recursionSites");
    assert!(
        !sites.is_empty(),
        "the argument must declare recursion sites"
    );

    let ceilings: Vec<String> = s["ceilings"]["notLoadBearing"]
        .as_array()
        .expect("notLoadBearing")
        .iter()
        .map(|v| v.as_str().expect("ceiling id").to_string())
        .collect();

    for site in sites {
        let id = site["id"].as_str().expect("site id");
        let witness = site["witness"].as_str().expect("site witness");
        match run(witness) {
            Ok(()) => {}
            Err(message) => {
                for ceiling in &ceilings {
                    assert!(
                        !message.contains(ceiling.as_str()),
                        "site {id}: witness {witness:?} was stopped by {ceiling}, so what ends it \
                         is a ceiling and not the measure this site claims to decrease",
                    );
                }
                panic!("site {id}: witness {witness:?} failed: {message}");
            }
        }
    }
}

/// The invariant: a String is not a code operand, so a Word cannot be called by
/// a name the program computed. This is what makes the DEF-time check's view of
/// the call graph complete.
#[test]
fn a_string_cannot_name_a_word_to_call() {
    let s = spec();

    for w in witnesses(&s, &["invariant", "witnesses", "refused"]) {
        let src = source_of(w);
        let expect = w["expect"].as_str().expect("expected outcome");
        let category = expect.strip_prefix("error:").expect("an error: outcome");
        let message = run(src).expect_err(&format!("{src:?} must be refused"));
        assert!(
            message.contains("code operand"),
            "{src:?} should be refused as a bad code operand ({category}), got: {message}",
        );
    }

    for w in witnesses(&s, &["invariant", "witnesses", "accepted"]) {
        let src = source_of(w);
        run(src).unwrap_or_else(|e| panic!("{src:?} must still be accepted: {e}"));
    }
}

/// Every acyclicity witness behaves as the argument says: the refused ones are
/// refused for being self-referential, the accepted ones still work.
#[test]
fn acyclicity_witnesses_hold() {
    let s = spec();

    for w in witnesses(&s, &["acyclicity", "witnesses", "refused"]) {
        let src = source_of(w);
        let message = run(src).expect_err(&format!("{src:?} must be refused"));
        assert!(
            message.contains("itself") || message.contains("cycle") || message.contains("self"),
            "{src:?} should be refused as self-referential, got: {message}",
        );
    }

    for w in witnesses(&s, &["acyclicity", "witnesses", "accepted"]) {
        let src = source_of(w);
        run(src).unwrap_or_else(|e| panic!("{src:?} must be accepted: {e}"));
    }
}

/// Build `DEF`s for a graph over W0..Wn where `edges[i]` are the words Wi's
/// body names, and return them in `order`.
fn definition_program(
    n: usize,
    edges: &HashMap<usize, Vec<usize>>,
    order: &[usize],
) -> Vec<String> {
    order
        .iter()
        .map(|&i| {
            let body: String = edges
                .get(&i)
                .map(|targets| {
                    targets
                        .iter()
                        .map(|&t| format!("W{t}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            let body = if body.is_empty() {
                "1".to_string()
            } else {
                body
            };
            format!("[ {body} ] 'W{i}' DEF")
        })
        .collect::<Vec<_>>()
        .into_iter()
        .take(n.max(order.len()))
        .collect()
}

/// Does the graph restricted to `defined` contain a cycle reachable from any
/// node? Plain DFS; the graph is tiny.
fn has_cycle(edges: &HashMap<usize, Vec<usize>>, defined: &HashSet<usize>) -> bool {
    fn visit(
        node: usize,
        edges: &HashMap<usize, Vec<usize>>,
        defined: &HashSet<usize>,
        stack: &mut HashSet<usize>,
        done: &mut HashSet<usize>,
    ) -> bool {
        if stack.contains(&node) {
            return true;
        }
        if done.contains(&node) || !defined.contains(&node) {
            return false;
        }
        stack.insert(node);
        for &next in edges.get(&node).map(|v| v.as_slice()).unwrap_or(&[]) {
            if visit(next, edges, defined, stack, done) {
                return true;
            }
        }
        stack.remove(&node);
        done.insert(node);
        false
    }

    let mut done = HashSet::new();
    for &node in defined {
        let mut stack = HashSet::new();
        if visit(node, edges, defined, &mut stack, &mut done) {
            return true;
        }
    }
    false
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Soundness and completeness of the DEF-time acyclicity check, over random
    /// definition graphs defined in random order.
    ///
    /// Soundness: the User dictionary never ends up holding a cycle — whenever
    /// a DEF would close one it is refused, and refused as self-referential.
    /// Completeness: a DEF that would *not* close a cycle always succeeds, so
    /// the check refuses no acyclic program. Definition order is part of the
    /// generated input because a forward reference to a Word that does not
    /// exist yet is a dead end at the moment it is written, and the cycle only
    /// becomes visible at the later DEF that closes it.
    #[test]
    fn def_accepts_exactly_the_acyclic_definitions(
        adjacency in prop::collection::vec(prop::collection::vec(0usize..5, 0..3), 5..=5),
        order in Just((0..5usize).collect::<Vec<_>>()).prop_shuffle(),
    ) {
        let edges: HashMap<usize, Vec<usize>> =
            adjacency.iter().enumerate().map(|(i, t)| (i, t.clone())).collect();

        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("tokio current-thread runtime");

        rt.block_on(async {
            let mut interp = Interpreter::new();
            let mut defined: HashSet<usize> = HashSet::new();

            for (index, program) in order
                .iter()
                .zip(definition_program(5, &edges, &order))
                .map(|(i, p)| (*i, p))
            {
                let mut candidate = defined.clone();
                candidate.insert(index);
                let would_cycle = has_cycle(&edges, &candidate);

                let result = interp.execute(&program).await;

                if would_cycle {
                    let message = result
                        .err()
                        .unwrap_or_else(|| panic!("{program} closes a cycle but was accepted"))
                        .to_string();
                    prop_assert!(
                        message.contains("itself")
                            || message.contains("cycle")
                            || message.contains("self"),
                        "{program} closes a cycle but was refused for another reason: {message}",
                    );
                } else {
                    prop_assert!(
                        result.is_ok(),
                        "{program} closes no cycle but was refused: {:?}",
                        result.err().map(|e| e.to_string()),
                    );
                    defined.insert(index);
                }
            }
            Ok(())
        })?;
    }
}

/// How a generated body reaches a Word it calls: written directly, inside a
/// literal block run by `EXEC`, or inside a `MAP` block over a one-element
/// Vector. All three are descents the termination argument names
/// (`userWordCall`, `EXEC`, `MAP`).
#[derive(Clone, Copy, Debug)]
enum Reach {
    Direct,
    Exec,
    Map,
}

fn reach() -> impl Strategy<Value = Reach> {
    prop_oneof![Just(Reach::Direct), Just(Reach::Exec), Just(Reach::Map)]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Every program over an acyclic User dictionary ends, without any
    /// ceiling, at the value the dictionary denotes.
    ///
    /// Wi's body names only Words Wj with j < i — the shape LANG.DICTIONARY.ACYCLIC
    /// guarantees — reached directly, through `EXEC` or through `MAP`. Each
    /// Word leaves one number: 1 when it calls nothing, else the sum of what
    /// its callees leave, plus 1 for each callee reached through `MAP` (its
    /// block adds the element). Calling the last Word must answer exactly that
    /// sum and stop on its own, never at `callDepth` or `executionSteps`.
    ///
    /// The inverse half is `no_program_reaches_a_ceiling_by_running_blocks`
    /// below.
    #[test]
    fn programs_over_an_acyclic_dictionary_end_at_their_value(
        calls in prop::collection::vec(
            prop::collection::vec((any::<prop::sample::Index>(), reach()), 0..3),
            6..=6,
        ),
    ) {
        let mut program = Vec::new();
        let mut value: Vec<u64> = Vec::new();
        for (i, callees) in calls.iter().enumerate() {
            let chosen: Vec<(usize, Reach)> = if i == 0 {
                Vec::new()
            } else {
                callees.iter().map(|(index, how)| (index.index(i), *how)).collect()
            };
            if chosen.is_empty() {
                program.push(format!("[ 1 ] 'W{i}' DEF"));
                value.push(1);
                continue;
            }
            let mut parts = Vec::new();
            let mut sum = 0u64;
            for (n, (j, how)) in chosen.iter().enumerate() {
                parts.push(match how {
                    Reach::Direct => format!("W{j}"),
                    Reach::Exec => format!("[ W{j} ] EXEC"),
                    Reach::Map => format!("[ 1 ] [ W{j} ADD ] MAP 0 GET"),
                });
                if n > 0 {
                    parts.push("ADD".to_string());
                }
                sum += value[*j] + u64::from(matches!(how, Reach::Map));
            }
            program.push(format!("[ {} ] 'W{i}' DEF", parts.join(" ")));
            value.push(sum);
        }
        let source = format!("{}\nW5", program.join("\n"));
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("tokio current-thread runtime");
        let shown = rt.block_on(async {
            let mut interp = Interpreter::new();
            interp.execute(&source).await.map_err(|e| e.to_string())?;
            Ok::<_, String>(format!("{}", interp.get_stack().last().expect("W5 leaves a value")))
        });
        prop_assert_eq!(shown, Ok(format!("{}/1", value[5])), "{}", source);
    }
}

/// The stratification witnesses behave as the argument says: each refused one
/// is refused as the category it names — never stopped by a ceiling, which
/// is how the first of them used to end — and each accepted one runs.
#[test]
fn a_block_built_from_data_never_runs_inside_another() {
    let s = spec();

    for w in witnesses(&s, &["stratification", "witnesses", "refused"]) {
        let src = source_of(w);
        let expect = w["expect"].as_str().expect("expected outcome");
        let category = expect.strip_prefix("error:").expect("an error: outcome");
        assert_eq!(
            run_category(src),
            Err(category.to_string()),
            "{src:?} must be refused as {category}"
        );
    }

    for w in witnesses(&s, &["stratification", "witnesses", "accepted"]) {
        let src = source_of(w);
        run(src).unwrap_or_else(|e| panic!("{src:?} must be accepted: {e}"));
    }
}

/// Pieces of a program that build, copy, bind, choose and run blocks — every
/// way the counter-example of 1.0.0-beta.2 was put together, and the ways a
/// rewrite of it would reach for.
const BLOCK_PIECES: &[&str] = &[
    "[ 1 COLLECT [ 0 0 ] GET EXEC EXEC ]",
    "[ 1 COLLECT [ 0 0 ] GET EXEC ]",
    "1 COLLECT [ 0 0 ] GET",
    "1 COLLECT [ 0 0 ] GET EXEC EXEC",
    "EXEC",
    "[ EXEC ]",
    "[ EXEC EXEC ]",
    "[ 1 ]",
    "[ 'F' BIND F F EXEC ]",
    "'G' BIND G",
    "TRUE SELECT",
    "'B' BIND [ 1 2 ] B MAP",
    "'C' BIND [ 1 ] C 1 COLLECT CONCAT",
    "'D' BIND [ 1 2 ] 0 D FOLD",
    "[ 1 COLLECT [ 0 0 ] GET 'E' BIND E EXEC ]",
    "RUN",
];

/// The inverse of the law above: however a program puts blocks together, it
/// ends — with a value or with a refusal — and no ceiling is what ends it.
/// Before 1.0.0-beta.3 a block could run a copy of itself, and only
/// `callDepth` stopped it; take the `nestedDataBlock` check out of the engine
/// and this property finds that again within a few cases.
mod inverse {
    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]

        #[test]
        fn no_program_reaches_a_ceiling_by_running_blocks(
            run_word in prop::collection::vec(prop::sample::select(BLOCK_PIECES), 0..4),
            program in prop::collection::vec(prop::sample::select(BLOCK_PIECES), 1..12),
        ) {
            let body: Vec<&str> = run_word.into_iter().filter(|p| *p != "RUN").collect();
            let source = format!("[ {} ] 'RUN' DEF\n{}", body.join(" "), program.join(" "));
            match run_category(&source) {
                Ok(()) => {}
                Err(category) => prop_assert!(
                    category != "resourceLimitExceeded" && category != "executionLimitExceeded",
                    "{} was stopped by {}", source, category
                ),
            }
        }
    }
}
