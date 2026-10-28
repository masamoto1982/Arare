//! Observable contracts that justify retaining native operational Standard Words.

use ajisai_core::interpreter::Interpreter;
use ajisai_core::ErrorCategory;

fn rendered_stack(interpreter: &Interpreter) -> Vec<String> {
    interpreter
        .get_stack()
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn effect_payloads(interpreter: &Interpreter) -> Vec<&str> {
    interpreter
        .host_effects()
        .iter()
        .map(|effect| effect.payload())
        .collect()
}

/// `MAP` is a Kernel Word; it is pinned here beside `FILTER`, whose visiting
/// order and isolated per-element stacks it shares.
#[tokio::test]
async fn map_visits_in_index_order_with_isolated_stacks_and_ordered_effects() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute("[ 3 1 2 ] [ 'E' BIND E PRINT E 10 ADD ] MAP")
        .await
        .unwrap();

    assert_eq!(effect_payloads(&interpreter), ["3/1", "1/1", "2/1"]);
    assert_eq!(rendered_stack(&interpreter), ["[ 13/1 11/1 12/1 ]"]);
}

#[tokio::test]
async fn filter_visits_in_index_order_and_observes_predicate_truth() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute("[ 3 1 2 ] [ 'E' BIND E PRINT E 1 GT ] FILTER")
        .await
        .unwrap();

    assert_eq!(effect_payloads(&interpreter), ["3/1", "1/1", "2/1"]);
    assert_eq!(rendered_stack(&interpreter), ["[ 3/1 2/1 ]"]);
}

/// `SCAN` answers the accumulator after each element, in index order, with one
/// lane out per lane in — and the accumulator it answers for a lane is the one
/// the next lane starts from, which is what makes the walk a walk rather than
/// a map.
///
/// Native retention rests on the cost. The same answer is reachable by folding
/// every prefix (`standard_derivation_laws.rs` shape, written here as the
/// comparison it is), but that re-reads each prefix from the start: quadratic
/// interpreted steps for a linear answer. With no recursion and no unbounded
/// loop in the language (`spec/termination.json`), carrying state from one
/// element to the next has no other shape, so the quadratic cost would be paid
/// by every running total, every state machine over a sequence, and every
/// recurrence.
#[tokio::test]
async fn scan_walks_in_index_order_and_answers_one_lane_per_element() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute("[ 3 1 2 ] 0 [ 'E' BIND E PRINT E ADD ] SCAN")
        .await
        .unwrap();

    // The block sees the element (printed) once per lane, in index order.
    assert_eq!(effect_payloads(&interpreter), ["3/1", "1/1", "2/1"]);
    assert_eq!(rendered_stack(&interpreter), ["[ 3/1 4/1 6/1 ]"]);

    // The same answer, derived: fold each prefix from the start.
    let mut derived = Interpreter::new();
    derived
        .execute("[ 3 1 2 ] 'V' BIND 1 3 RANGE [ 'K' BIND V K TAKE 0 [ ADD ] FOLD ] MAP")
        .await
        .unwrap();
    assert_eq!(rendered_stack(&derived), ["[ 3/1 4/1 6/1 ]"]);
}

/// The seed is not a lane, so it is not in the answer, and a walk with no
/// lanes answers no lanes. `FOLD` differs on both counts because it reduces to
/// one value rather than to a lane per lane: with nothing to reduce it answers
/// the seed it was handed.
#[tokio::test]
async fn scan_is_lane_for_lane_where_fold_is_seed_shaped() {
    let mut empty_scan = Interpreter::new();
    empty_scan.execute("[ ] 7 [ ADD ] SCAN").await.unwrap();
    assert_eq!(rendered_stack(&empty_scan), ["[ ]"]);

    let mut empty_fold = Interpreter::new();
    empty_fold.execute("[ ] 7 [ ADD ] FOLD").await.unwrap();
    assert_eq!(rendered_stack(&empty_fold), ["7/1"]);

    let mut absent_scan = Interpreter::new();
    absent_scan.execute("NIL 7 [ ADD ] SCAN").await.unwrap();
    assert_eq!(rendered_stack(&absent_scan), ["NIL"]);

    // An absent Vector is data to both (LANG.FAILURE.PASSTHROUGH): the
    // absence is the result, not the seed standing in for an empty fold.
    let mut absent_fold = Interpreter::new();
    absent_fold.execute("NIL 7 [ ADD ] FOLD").await.unwrap();
    assert_eq!(rendered_stack(&absent_fold), ["NIL"]);
}

#[tokio::test]
async fn higher_order_errors_restore_the_original_operand_atomically() {
    for word in ["MAP", "FILTER"] {
        let mut interpreter = Interpreter::new();
        let source = format!("[ 3 1 2 ] [ UNKNOWN-CALLBACK ] {word}");
        assert!(interpreter.execute(&source).await.is_err(), "{word}");
        assert_eq!(
            rendered_stack(&interpreter),
            ["[ 3/1 1/1 2/1 ]", "[ UNKNOWN-CALLBACK ]"],
            "{word}"
        );
    }
}

#[tokio::test]
async fn fill_checks_overflow_and_ceiling_before_materializing() {
    for source in [
        "[ 1000000 1000000 ] 7 FILL",
        "[ 99999999 99999999 99999999 ] 1 FILL",
    ] {
        let mut interpreter = Interpreter::new();
        let err = interpreter.execute(source).await.unwrap_err();
        assert_eq!(
            ErrorCategory::from_error(&err),
            Some(ErrorCategory::ResourceLimitExceeded),
            "`{source}`: {err:?}"
        );
        assert!(
            err.to_string().contains("materializedElements"),
            "`{source}`: the refusal names its ceiling: {err}"
        );
        assert_eq!(
            interpreter.get_stack().len(),
            2,
            "`{source}`: operands put back"
        );
    }
}

#[tokio::test]
async fn sort_is_deterministic_and_restores_malformed_operands() {
    for _ in 0..3 {
        let mut interpreter = Interpreter::new();
        interpreter.execute("[ 3 1 2 1 ] SORT").await.unwrap();
        assert_eq!(rendered_stack(&interpreter), ["[ 1/1 1/1 2/1 3/1 ]"]);
    }

    let mut malformed = Interpreter::new();
    assert!(malformed.execute("[ 3 'x' 2 ] SORT").await.is_err());
    assert_eq!(rendered_stack(&malformed), ["[ 3/1 'x' 2/1 ]"]);
}

/// `ORDER` answers the permutation `SORT` applies, and answers it stably: two
/// equal keys keep their original positions.
///
/// Stability is the property the reference's hand-written ranking idiom existed
/// to recover — `SORT` then a first-match search finds the same position twice when two
/// keys tie, so "the k nearest" could hand back k-1 or k+1 neighbours. Native
/// retention rests on this and on the cost: the idiom is O(n²) interpreted
/// steps, which is what put a ceiling of about ninety training points on a
/// k-nearest-neighbour classifier while `SORT` handled five thousand elements.
#[tokio::test]
async fn order_is_the_stable_permutation_sort_applies() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute("[ 18 13 1 1 13 2 ] ORDER")
        .await
        .unwrap();
    assert_eq!(
        rendered_stack(&interpreter),
        ["[ 2/1 3/1 5/1 1/1 4/1 0/1 ]"]
    );

    // Applying the permutation reproduces SORT exactly.
    let mut applied = Interpreter::new();
    applied
        .execute("[ 18 13 1 1 13 2 ] 'V' BIND V V ORDER GET")
        .await
        .unwrap();
    let mut sorted = Interpreter::new();
    sorted.execute("[ 18 13 1 1 13 2 ] SORT").await.unwrap();
    assert_eq!(rendered_stack(&applied), rendered_stack(&sorted));

    let mut malformed = Interpreter::new();
    assert!(malformed.execute("[ 3 'x' 2 ] ORDER").await.is_err());
    assert_eq!(rendered_stack(&malformed), ["[ 3/1 'x' 2/1 ]"]);
}

/// `UNIQUE` and `TALLY` are one pass read two ways: `TALLY`'s keys are
/// `UNIQUE`'s answer, in the same order, and its values the aligned counts
/// (LANG.RECORDS.STRUCTURE), whatever the element domain. Written out, each
/// is an O(n²) first-match scan, and they are the counting step of a
/// majority vote, a class prior, a histogram, a Gini and a naive-Bayes tally.
#[tokio::test]
async fn unique_and_tally_agree_on_order_and_length() {
    let mut unique = Interpreter::new();
    unique
        .execute("[ 'b' 'a' 'b' 'c' 'b' ] UNIQUE")
        .await
        .unwrap();
    let mut tally = Interpreter::new();
    tally
        .execute("[ 'b' 'a' 'b' 'c' 'b' ] TALLY")
        .await
        .unwrap();
    assert_eq!(rendered_stack(&unique), ["[ 'b' 'a' 'c' ]"]);
    assert_eq!(
        rendered_stack(&tally),
        ["[ 'b' 'a' 'c' ] [ 3/1 1/1 1/1 ] RECORD"]
    );
}

/// `ZIP` transposes, and transposing twice is the identity on a rectangular
/// matrix. Unequal rows are a length error rather than a ragged result.
#[tokio::test]
async fn zip_is_transposition_and_is_involutive() {
    let mut once = Interpreter::new();
    once.execute("[ [ 1 2 3 ] [ 4 5 6 ] ] ZIP").await.unwrap();
    assert_eq!(
        rendered_stack(&once),
        ["[ [ 1/1 4/1 ] [ 2/1 5/1 ] [ 3/1 6/1 ] ]"]
    );

    let mut twice = Interpreter::new();
    twice
        .execute("[ [ 1 2 3 ] [ 4 5 6 ] ] ZIP ZIP")
        .await
        .unwrap();
    assert_eq!(
        rendered_stack(&twice),
        ["[ [ 1/1 2/1 3/1 ] [ 4/1 5/1 6/1 ] ]"]
    );

    let mut ragged = Interpreter::new();
    assert!(ragged.execute("[ [ 1 2 ] [ 3 ] ] ZIP").await.is_err());
}

/// `PUT` replaces one position and leaves every other alone, counting from the
/// end for a negative index as everything else does.
#[tokio::test]
async fn put_replaces_exactly_one_position() {
    let mut interpreter = Interpreter::new();
    interpreter.execute("[ 1 2 3 ] 1 9 PUT").await.unwrap();
    assert_eq!(rendered_stack(&interpreter), ["[ 1/1 9/1 3/1 ]"]);

    let mut from_end = Interpreter::new();
    from_end.execute("[ 1 2 3 ] -1 9 PUT").await.unwrap();
    assert_eq!(rendered_stack(&from_end), ["[ 1/1 2/1 9/1 ]"]);

    // A well-formed index that names no slot is data that did not work out,
    // so it projects rather than raising, and the operands are consumed as on
    // any other answer.
    let mut past_end = Interpreter::new();
    past_end.execute("[ 1 2 3 ] 5 9 PUT").await.unwrap();
    assert_eq!(rendered_stack(&past_end), ["NIL"]);
}

/// `GROUP` bundles by key into a Record, keys in `UNIQUE` key order, and keeps
/// every value exactly once, so a grouping never loses or duplicates data and
/// a group is read back by its key.
#[tokio::test]
async fn group_partitions_without_loss() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute("[ 'b' 'a' 'b' 'a' ] [ 1 2 3 4 ] GROUP")
        .await
        .unwrap();
    assert_eq!(
        rendered_stack(&interpreter),
        ["[ 'b' 'a' ] [ [ 1/1 3/1 ] [ 2/1 4/1 ] ] RECORD"]
    );

    let mut mismatched = Interpreter::new();
    assert!(mismatched
        .execute("[ 'a' 'b' ] [ 1 2 3 ] GROUP")
        .await
        .is_err());
}

/// `BSEARCH` answers what a first match over `GET` and `EQ` answers on an
/// ascending vector — the first index of the key, or a `notFound` absence —
/// and refuses an unsorted operand rather than answering from it.
#[tokio::test]
async fn bsearch_agrees_with_a_first_match_on_ascending_input_and_refuses_unsorted() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute("[ 1 3 3 7 ] [ 3 7 4 ] BSEARCH [ 1 3 3 7 ] 4 BSEARCH NIL-REASON")
        .await
        .unwrap();
    assert_eq!(
        rendered_stack(&interpreter),
        ["[ 1/1 3/1 NIL ]", "'notFound'"]
    );

    let mut interpreter = Interpreter::new();
    let result = interpreter.execute("[ 3 1 2 ] 2 BSEARCH").await;
    assert!(
        result.is_err(),
        "an unsorted operand must raise, not answer"
    );
    assert_eq!(rendered_stack(&interpreter), ["[ 3/1 1/1 2/1 ]", "2/1"]);
}

/// `SEARCH` and `REPLACE` answer what a window compared at every position
/// over `CHARS` answers, in one pass: positions count characters, and
/// replacement is left to right without overlap.
#[tokio::test]
async fn search_and_replace_are_the_one_pass_forms_of_the_window_scan() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute("'abcabc' 'ca' SEARCH 'abcabc' 'ca' 'X' REPLACE 'aaaa' 'aa' 'b' REPLACE")
        .await
        .unwrap();
    assert_eq!(rendered_stack(&interpreter), ["2/1", "'abXbc'", "'bb'"]);
}

/// `FORMAT` answers what `ROUND` scaled to `10^digits` followed by a decimal
/// spelling of the result would answer — the same rounded quantity, under the
/// same tie rule — in one place, as text, so the rounding never re-enters
/// arithmetic. The last digit of an irrational is decided exactly, never
/// guessed.
#[tokio::test]
async fn format_agrees_with_scaled_round() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute(
            "2/3 2 FORMAT 2/3 100 MUL ROUND 100 DIV 5/2 0 FORMAT 7/2 0 FORMAT 2 SQRT 3 FORMAT 2 SQRT 3 SQRT ADD 2 FORMAT",
        )
        .await
        .unwrap();
    assert_eq!(
        rendered_stack(&interpreter),
        ["'0.67'", "67/100", "'3'", "'4'", "'1.414'", "'3.15'"]
    );

    let mut interpreter = Interpreter::new();
    let result = interpreter.execute("1/3 -1 FORMAT").await;
    assert!(result.is_err(), "a negative digit count must raise");
    assert_eq!(rendered_stack(&interpreter), ["1/3", "-1/1"]);
}

/// `JSON-DECODE` lands each JSON kind on its own domain with numbers exact,
/// `JSON-ENCODE` writes only what JSON can spell exactly, and the two compose
/// to the identity on the JSON image; a rational with no finite decimal
/// travels as its lexeme in a string, so decoding gives the String `NUM`
/// recovers the number from, and nothing is rounded on the way.
#[tokio::test]
async fn json_decode_and_encode_are_exact_and_compose_to_the_identity() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute(
            "'{\"a\": 0.1, \"b\": [true, null, \"x\"]}' JSON-DECODE 'a' GET 10 MUL \
             [ 'a' 'b' ] [ 1/4 [ TRUE NIL 'x' ] ] RECORD JSON-ENCODE \
             [ 'a' 'b' ] [ 1/4 [ TRUE NIL 'x' ] ] RECORD 'V' BIND V V JSON-ENCODE JSON-DECODE EQ \
             1/3 JSON-ENCODE 'J' BIND J J JSON-DECODE NUM \
             2 SQRT JSON-ENCODE NIL-REASON \
             '[1,' JSON-DECODE NIL-REASON",
        )
        .await
        .unwrap();
    assert_eq!(
        rendered_stack(&interpreter),
        [
            "1/1",
            "'{\"a\":0.25,\"b\":[true,null,\"x\"]}'",
            "TRUE",
            "'\"1/3\"'",
            "1/3",
            "'domainMiss'",
            "'invalidEncoding'",
        ]
    );
}

/// `GCD` answers what Euclid's algorithm written over a fixed number of
/// steps answers on operands that terminate within them, and `RATIO` reads
/// back the two parts `DIV` rebuilds the rational from: both expose what the
/// machine already does to keep every rational reduced.
#[tokio::test]
async fn gcd_and_ratio_agree_with_the_kernel_spellings() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute(
            "12 18 GCD 18 18 12 DIV FLOOR 12 MUL SUB 12 GCD \
             6/4 RATIO 0 GET 6/4 RATIO 1 GET DIV 3/2 EQ \
             2 SQRT RATIO NIL-REASON 1/2 4 GCD NIL-REASON",
        )
        .await
        .unwrap();
    assert_eq!(
        rendered_stack(&interpreter),
        ["6/1", "6/1", "TRUE", "'domainMiss'", "'domainMiss'"]
    );
}

/// `UPPER` and `LOWER` apply Unicode's default case mapping and nothing
/// language-specific, so the same text maps the same way wherever it runs;
/// a mapping that changes length (`ß` → `SS`) is applied whole.
#[tokio::test]
async fn upper_and_lower_apply_the_default_unicode_mapping() {
    let mut interpreter = Interpreter::new();
    interpreter
        .execute("'Ajisai' UPPER 'Ajisai' LOWER 'straße' UPPER 'ΣΑΣ' LOWER 'İ' LOWER CHARS LENGTH 'a1-' UPPER")
        .await
        .unwrap();
    assert_eq!(
        rendered_stack(&interpreter),
        ["'AJISAI'", "'ajisai'", "'STRASSE'", "'σασ'", "2/1", "'A1-'"]
    );
    let mut interpreter = Interpreter::new();
    assert!(interpreter.execute("42 UPPER").await.is_err());
    assert_eq!(rendered_stack(&interpreter), ["42/1"]);
}
