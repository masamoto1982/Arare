//! `check` refuses a literal block its higher-order Word cannot run, exactly
//! where running it would raise, and never a block that could run.

use super::first_block_arity_violation;
use crate::agent::api::check;
use crate::interpreter::Interpreter;

/// The category a running program fails with, if it fails.
async fn run_failure(source: &str) -> Option<String> {
    let mut interp = Interpreter::new();
    let err = interp.execute(source).await.err()?;
    let category = crate::error::ErrorCategory::from_error(&err)?;
    Some(category.as_protocol_str().to_string())
}

const LEAVES: &str = "blockContractViolation";
const READS_BELOW: &str = "stackUnderflow";

/// Programs whose block cannot leave exactly one value: `check` refuses each
/// with the category running it raises.
const REFUSED: &[(&str, &str, &str)] = &[
    ("[ 1 2 3 ] [ 2 MUL 7 ] MAP", "MAP", LEAVES),
    ("[ 1 2 3 ] [ 'X' BIND X X ] MAP", "MAP", LEAVES),
    ("[ 1 2 3 ] [ 1 GT TRUE ] FILTER", "FILTER", LEAVES),
    ("[ 1 2 3 ] [ 'X' BIND ] MAP", "MAP", LEAVES),
    ("[ 1 2 3 ] 0 [ ADD ADD ] FOLD", "FOLD", READS_BELOW),
    ("[ 1 2 3 ] [ ADD ] MAP", "MAP", READS_BELOW),
    ("[ 1 2 3 ] 0 [ 'A' BIND 'B' BIND ] SCAN", "SCAN", LEAVES),
    // Inside a definition's body, and nested inside a block that runs.
    ("[ [ 2 MUL 7 ] MAP ] 'F' DEF [ 1 2 ] F", "MAP", LEAVES),
    ("[ [ 1 2 ] [ 3 4 ] ] [ [ 1 2 ] MAP ] MAP", "MAP", LEAVES),
];

/// Programs `check` must let through: blocks that leave one value, and blocks
/// the inference cannot fix, which are not judged.
const ADMITTED: &[&str] = &[
    "[ 1 2 3 ] [ 2 MUL ] MAP",
    "[ 1 2 3 ] [ 1 GT ] FILTER",
    "[ 1 2 3 ] 0 [ ADD ] FOLD",
    "[ 1 2 3 ] 0 [ 'E' BIND 'A' BIND A E ADD ] FOLD",
    "[ 1 2 3 ] 0 [ ADD ] SCAN",
    // Binding the element leaves the accumulator: one value.
    "[ 1 2 3 ] 0 [ 'E' BIND ] SCAN",
    "[ 1 2 3 ] [ 'X' BIND X X MUL ] MAP",
    // A binding from the enclosing frame: the arity is not fixed here.
    "2 'N' BIND [ 1 2 3 ] [ N EQ ] FILTER",
    // A User Word the source defines is read through its own body.
    "[ 2 MUL ] 'DOUBLE' DEF [ 1 2 ] [ DOUBLE ] MAP",
    // A block that is data, never run.
    "[ [ 2 MUL 7 ] MAP ] 0 GET",
    // The block reaches MAP through a binding: not the literal before it.
    "[ 2 MUL 7 ] 'B' BIND [ 1 2 ] B MAP",
];

#[tokio::test]
async fn a_block_that_cannot_leave_one_value_is_refused_as_running_it_would_be() {
    for (source, word, raised) in REFUSED {
        let report = check(source).to_json();
        assert_eq!(report["status"], "error", "{source}: {report}");
        assert_eq!(report["error"]["category"], *raised, "{source}: {report}");
        assert_eq!(report["error"]["word"], *word, "{source}: {report}");
        assert!(report["error"]["column"].is_number(), "{source}: {report}");
        let category = run_failure(source)
            .await
            .unwrap_or_else(|| panic!("`{source}` must fail when run"));
        assert_eq!(category, *raised, "{source}");
    }
}

#[test]
fn a_block_that_can_leave_one_value_is_admitted() {
    for source in ADMITTED {
        assert!(
            first_block_arity_violation(source).is_none(),
            "{source} was refused"
        );
        assert_eq!(check(source).to_json()["status"], "ok", "{source}");
    }
}

#[test]
fn the_message_says_how_many_values_and_where() {
    let violation =
        first_block_arity_violation("1 2 ADD\n[ 1 2 3 ] [ 2 MUL 7 ] MAP").expect("refused");
    assert_eq!(
        violation.message,
        "MAP: the block written before it leaves 2 values from the 1 MAP gives it, \
         and a MAP block must leave exactly one"
    );
    let at = violation.position.expect("a position");
    assert_eq!((at.line, at.column), (2, 23));
    let reaching = first_block_arity_violation("[ 1 2 3 ] [ ADD ] MAP").expect("refused");
    assert!(reaching
        .message
        .contains("reads 2 values, and MAP gives it 1"));
}

#[tokio::test]
async fn an_empty_collection_still_runs_because_compute_leaves_the_block_to_the_run() {
    let source = "[ ] [ 2 MUL 7 ] MAP";
    assert_eq!(check(source).to_json()["status"], "error");
    assert!(
        run_failure(source).await.is_none(),
        "an empty MAP never runs its block"
    );
}
