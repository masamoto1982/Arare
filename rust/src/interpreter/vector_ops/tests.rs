//! Test suite for `crate::interpreter::vector_ops`, including the Words
//! under modifier modes.

use crate::interpreter::Interpreter;

#[tokio::test]
async fn test_range_basic_stacktop() {
    let mut interp = Interpreter::new();

    let result = interp.execute("0 5 RANGE").await;
    assert!(result.is_ok(), "RANGE should succeed: {:?}", result);

    assert_eq!(interp.stack.len(), 1);
}

#[tokio::test]
async fn test_range_counts_down_when_end_is_below_start() {
    let mut interp = Interpreter::new();
    interp.execute("3 0 RANGE").await.unwrap();
    assert_eq!(
        format!("{}", interp.stack.last().unwrap()),
        "[ 3/1 2/1 1/1 0/1 ]"
    );
}

#[tokio::test]
async fn test_range_single_element() {
    let mut interp = Interpreter::new();

    let result = interp.execute("5 5 RANGE").await;
    assert!(
        result.is_ok(),
        "RANGE single element should succeed: {:?}",
        result
    );
    assert_eq!(interp.stack.len(), 1);
}

/// A bound that is not an integer names no position in an integer sequence:
/// `invalidInteger`, with both bounds put back where the program wrote them.
#[tokio::test]
async fn test_range_non_integer_bound_restores_both_operands() {
    for code in ["0 1/2 RANGE", "'a' 5 RANGE"] {
        let mut interp = Interpreter::new();
        let result = interp.execute(code).await;
        assert!(result.is_err(), "{code} should fail");
        assert_eq!(interp.stack.len(), 2, "{code}: both bounds restored");
    }
}

/// Both bounds are leaves, so a Vector of bounds lifts to one sequence each.
#[tokio::test]
async fn test_range_lifts_over_a_vector_of_bounds() {
    let mut interp = Interpreter::new();
    interp.execute("[ 0 1 ] 2 RANGE").await.unwrap();
    assert_eq!(
        format!("{}", interp.stack.last().unwrap()),
        "[ [ 0/1 1/1 2/1 ] [ 1/1 2/1 ] ]"
    );
}

// ── RANGE builds columns, and they are the same numbers ─────────────────────
//
// The tests above assert that RANGE succeeds and leaves one value; none of them
// look at what that value holds, so they passed whatever the lanes were. That
// mattered when RANGE stopped boxing per-lane `Value`s and started writing `i64`
// columns: the construction changed, and nothing in the suite could have noticed
// if it had changed the numbers with it.

/// Whether the top of the stack is a dense (SoA) tensor.
///
/// The representation is the point of the change, so it is stated outright: a
/// RANGE result is a 1-D pure-integer dense tensor *by construction* —
/// `parse_range_args` answers in `i64`, so no lane can be rational, irrational
/// or absent. Building `Vec<Value>` instead threw that away at the source, and
/// every Word downstream then had to decline its dense fast path.
async fn range_lands_a_dense_tensor(code: &str) -> bool {
    let mut interp = Interpreter::new();
    interp
        .execute(code)
        .await
        .unwrap_or_else(|e| panic!("`{code}` must compute, got: {e:?}"));
    matches!(
        interp.get_stack().as_slice().last().map(|v| &v.data),
        Some(crate::types::ValueData::Tensor { .. })
    )
}

/// `EQ` against the literal spelling of the same sequence. This is the gate
/// that matters: a dense tensor and the nested vector of the same numbers are
/// one value (`value_identity` grants them cross-representation equality), so
/// if the columns hold anything other than what the literal says, the language
/// itself reports it.
async fn range_equals_literal(range: &str, literal: &str) -> Option<bool> {
    let code = format!("{range} {literal} EQ");
    let mut interp = Interpreter::new();
    interp
        .execute(&code)
        .await
        .unwrap_or_else(|e| panic!("`{code}` must compute, got: {e:?}"));
    interp.get_stack().as_slice().last()?.as_truth()
}

#[tokio::test]
async fn range_results_are_dense_tensors() {
    for code in ["0 5 RANGE", "10 0 RANGE", "5 5 RANGE", "-3 3 RANGE"] {
        assert!(
            range_lands_a_dense_tensor(code).await,
            "`{code}` must build i64 columns, not boxed per-lane Values"
        );
    }
}

#[tokio::test]
async fn range_lanes_are_the_numbers_the_literal_spells() {
    // Every shape the argument parser admits: the default ascending step, an
    // explicit step that does not divide the span evenly, a descending step,
    // a single-element range, and a span crossing zero.
    for (range, literal) in [
        ("0 5 RANGE", "[ 0 1 2 3 4 5 ]"),
        ("10 7 RANGE", "[ 10 9 8 7 ]"),
        ("5 5 RANGE", "[ 5 ]"),
        ("-3 3 RANGE", "[ -3 -2 -1 0 1 2 3 ]"),
        ("3 -3 RANGE", "[ 3 2 1 0 -1 -2 -3 ]"),
    ] {
        assert_eq!(
            range_equals_literal(range, literal).await,
            Some(true),
            "`{range}` must hold exactly {literal}"
        );
    }
}

/// The count the space-ceiling guard computes must be the count the loop
/// writes, or the guard is bounding a different number than the one allocated.
/// `LENGTH` is how the language asks.
#[tokio::test]
async fn range_length_matches_the_counted_span() {
    for (code, expected) in [
        ("0 5 RANGE LENGTH", 6),
        ("10 7 RANGE LENGTH", 4),
        ("5 5 RANGE LENGTH", 1),
        ("-3 3 RANGE LENGTH", 7),
    ] {
        let mut interp = Interpreter::new();
        interp
            .execute(code)
            .await
            .unwrap_or_else(|e| panic!("`{code}` must compute, got: {e:?}"));
        let length = interp
            .get_stack()
            .as_slice()
            .last()
            .and_then(|v| v.as_scalar().and_then(|f| f.to_i64()))
            .unwrap_or_else(|| panic!("`{code}` must leave an integer length"));
        assert_eq!(length, expected, "`{code}` must count {expected} lanes");
    }
}

/// A bound outside `i64` is still an integer, so RANGE builds the sequence
/// rather than raising `invalidInteger`, which the spec reserves for a bound
/// that is not an integer. The ceiling still bounds a span that is too long.
#[tokio::test]
async fn range_bounds_past_i64_are_integers_and_build() {
    for (range, literal) in [
        (
            "9223372036854775808 9223372036854775810 RANGE",
            "[ 9223372036854775808 9223372036854775809 9223372036854775810 ]",
        ),
        (
            "9223372036854775806 9223372036854775808 RANGE",
            "[ 9223372036854775806 9223372036854775807 9223372036854775808 ]",
        ),
        (
            "-9223372036854775808 -9223372036854775810 RANGE",
            "[ -9223372036854775808 -9223372036854775809 -9223372036854775810 ]",
        ),
        (
            "100000000000000000000 100000000000000000000 RANGE",
            "[ 100000000000000000000 ]",
        ),
    ] {
        assert_eq!(
            range_equals_literal(range, literal).await,
            Some(true),
            "`{range}` must hold exactly {literal}"
        );
    }
    assert_eq!(
        crate::test_support::error_of("0 100000000000000000000 RANGE").await,
        "resourceLimitExceeded"
    );
}

// ── REVERSE keeps a flat dense buffer dense ─────────────────────────────────

/// `EQ` between two programs. Cross-representation equality means a dense
/// result and the nested literal of the same numbers are one value, so this
/// asks the language whether the reversal produced what it should have.
async fn programs_are_equal(left: &str, right: &str) -> Option<bool> {
    let code = format!("{left} {right} EQ");
    let mut interp = Interpreter::new();
    interp
        .execute(&code)
        .await
        .unwrap_or_else(|e| panic!("`{code}` must compute, got: {e:?}"));
    interp.get_stack().as_slice().last()?.as_truth()
}

#[tokio::test]
async fn reverse_keeps_a_flat_dense_result_dense() {
    // The representation is the point: reversing used to unpack the tensor into
    // boxed per-lane `Value`s and hand back an AoS `Vector`, which left every
    // Word downstream to decline its own dense path.
    assert!(
        range_lands_a_dense_tensor("0 7 RANGE REVERSE").await,
        "a flat dense REVERSE must stay dense"
    );
    assert!(
        range_lands_a_dense_tensor("[ 1 2 3 4 5 6 7 8 ] REVERSE").await,
        "a dense literal's REVERSE must stay dense"
    );
}

#[tokio::test]
async fn reverse_produces_the_reversed_sequence() {
    for (reversed, literal) in [
        ("[ 1 2 3 4 5 6 7 8 ] REVERSE", "[ 8 7 6 5 4 3 2 1 ]"),
        ("0 5 RANGE REVERSE", "[ 5 4 3 2 1 0 ]"),
        ("[ 5 ] REVERSE", "[ 5 ]"),
        // Rational lanes: the columns reverse together, so 1/3 2/3 1 4/3
        // becomes 4/3 1 2/3 1/3 rather than pairing a numerator with the
        // wrong denominator.
        (
            "[ 1 2 3 4 ] [ 3 DIV ] MAP REVERSE",
            "[ 4 3 2 1 ] [ 3 DIV ] MAP",
        ),
    ] {
        assert_eq!(
            programs_are_equal(reversed, literal).await,
            Some(true),
            "`{reversed}` must equal `{literal}`"
        );
    }
}

#[tokio::test]
async fn reverse_twice_restores_the_original() {
    for source in [
        "[ 1 2 3 4 5 6 7 8 ]",
        "20 0 RANGE",
        "[ 1 NIL 3 4 5 6 7 8 ]",
        "[ [ 1 2 ] [ 3 4 ] [ 5 6 ] ]",
        "[ 'a' 'b' 'c' ]",
    ] {
        assert_eq!(
            programs_are_equal(&format!("{source} REVERSE REVERSE"), source).await,
            Some(true),
            "`{source} REVERSE REVERSE` must restore `{source}`"
        );
    }
}

/// An absent lane lands at `len - 1 - index`, and the lanes around it do not
/// become absent. Read through the language, so the dense remap is checked
/// where a program would see it.
#[tokio::test]
async fn reverse_moves_an_absent_lane_to_its_mirrored_index() {
    // `[ 1 NIL 3 4 5 6 7 8 ]` densifies with lane 1 absent; reversed, index 6.
    for (code, expect_nil) in [
        ("[ 1 NIL 3 4 5 6 7 8 ] REVERSE 6 GET NIL?", true),
        ("[ 1 NIL 3 4 5 6 7 8 ] REVERSE 1 GET NIL?", false),
        ("[ 1 NIL 3 4 5 6 7 8 ] REVERSE 7 GET NIL?", false),
    ] {
        let mut interp = Interpreter::new();
        interp
            .execute(code)
            .await
            .unwrap_or_else(|e| panic!("`{code}` must compute, got: {e:?}"));
        assert_eq!(
            interp
                .get_stack()
                .as_slice()
                .last()
                .and_then(|v| v.as_truth()),
            Some(expect_nil),
            "`{code}` must answer {expect_nil}"
        );
    }
}

/// Rank above 1 keeps the nested route, because reversing a rank-2 tensor
/// reverses its *rows* and a row is a stride rather than a lane.
#[tokio::test]
async fn reverse_of_a_rank_two_value_reverses_rows() {
    assert_eq!(
        programs_are_equal(
            "[ [ 1 2 ] [ 3 4 ] [ 5 6 ] ] REVERSE",
            "[ [ 5 6 ] [ 3 4 ] [ 1 2 ] ]"
        )
        .await,
        Some(true),
        "a rank-2 REVERSE must reverse rows, not lanes"
    );
}

#[tokio::test]
async fn test_collect_basic() {
    let mut interp = Interpreter::new();

    let result = interp.execute("1 2 3 3 COLLECT").await;
    assert!(result.is_ok(), "COLLECT should succeed: {:?}", result);
    assert_eq!(interp.stack.len(), 1);

    let val = &interp.stack[0];
    assert_eq!(val.shape(), vec![3], "Result should have 3 elements");
}

#[tokio::test]
async fn test_collect_vectors_without_flattening() {
    let mut interp = Interpreter::new();

    let result = interp.execute("[ 1 2 ] [ 3 4 ] 2 COLLECT").await;
    assert!(
        result.is_ok(),
        "COLLECT vectors should succeed: {:?}",
        result
    );
    assert_eq!(interp.stack.len(), 1);

    let val = &interp.stack[0];
    assert!(val.is_vector(), "Result should be a vector");
}
#[tokio::test]
async fn test_collect_error_underflow() {
    let mut interp = Interpreter::new();

    let result = interp.execute("1 2 5 COLLECT").await;
    assert!(
        result.is_err(),
        "COLLECT with insufficient stack should fail"
    );

    assert_eq!(interp.stack.len(), 3, "Stack should have count pushed back");
}

#[tokio::test]
async fn test_collect_zero_count_is_the_empty_vector() {
    // N is a non-negative integer, so zero is a count like any other: it
    // takes nothing and answers `[ ]`, leaving the stack below untouched.
    let mut interp = Interpreter::new();

    let result = interp.execute("1 2 3 0 COLLECT").await;
    assert!(
        result.is_ok(),
        "0 COLLECT answers the empty Vector: {:?}",
        result
    );
    assert_eq!(interp.stack.len(), 4);
    assert_eq!(
        crate::types::display::render_stack(interp.get_stack())
            .last()
            .map(String::as_str),
        Some("[ ]")
    );
}

#[tokio::test]
async fn test_collect_error_negative_count() {
    let mut interp = Interpreter::new();

    let result = interp.execute("1 2 3 -2 COLLECT").await;
    assert!(result.is_err(), "COLLECT with negative count should fail");
}

#[tokio::test]
async fn test_get_consumes_its_operands() {
    let mut interp = Interpreter::new();

    let result = interp.execute("[ 10 20 30 ] 0 GET").await;
    assert!(result.is_ok(), "GET should succeed: {:?}", result);
    assert_eq!(
        interp.stack.len(),
        1,
        "GET consumes what it reads: both operands leave, the element stays"
    );
}

#[tokio::test]
async fn test_length_consumes_its_operand() {
    let mut interp = Interpreter::new();

    let result = interp.execute("[ 1 2 3 4 5 ] LENGTH").await;
    assert!(result.is_ok(), "LENGTH should succeed: {:?}", result);
    assert_eq!(
        interp.stack.len(),
        1,
        "LENGTH consumes what it reads: the measured vector leaves the stack"
    );
}

#[tokio::test]
async fn test_get_returns_the_element() {
    let mut interp = Interpreter::new();
    let result = interp.execute("[ 10 20 30 ] 0 GET").await;
    assert!(result.is_ok(), "GET should succeed: {:?}", result);
    assert_eq!(interp.stack.len(), 1, "only the element remains");
    let result_scalar = interp.stack[0]
        .as_scalar()
        .expect("result should be scalar");
    assert_eq!(result_scalar.to_i64(), Some(10));
}

#[tokio::test]
async fn test_print_outputs_and_consumes() {
    let mut interp = Interpreter::new();
    let result = interp.execute("[ 42 ] PRINT").await;
    assert!(result.is_ok(), "PRINT should succeed: {:?}", result);
    assert!(interp.stack.is_empty(), "PRINT consumes its operand");
    assert!(
        interp.output_buffer.contains("42/1"),
        "PRINT should output the value, got: {}",
        interp.output_buffer
    );
}

#[tokio::test]
async fn test_floor_consumes_operand() {
    let mut interp = Interpreter::new();
    let result = interp.execute("[ 3.7 ] FLOOR").await;
    assert!(result.is_ok(), "FLOOR should succeed: {:?}", result);
    assert_eq!(interp.stack.len(), 1, "FLOOR leaves only its result");
}
