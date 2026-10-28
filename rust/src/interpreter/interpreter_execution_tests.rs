//! Test suite for interpreter execution behavior.

use crate::interpreter::Interpreter;

#[tokio::test]
async fn test_simple_addition() {
    let mut interp = Interpreter::new();

    let code = "[ 2 ] [ 3 ] ADD";

    let result = interp.execute(code).await;
    assert!(
        result.is_ok(),
        "Simple addition should succeed: {:?}",
        result
    );

    assert_eq!(interp.stack.len(), 1, "Stack should have one element");
}

#[tokio::test]
async fn test_definition_and_call() {
    let mut interp = Interpreter::new();

    let code = r#"
[ [ 2 ] [ 3 ] ADD ] 'ADDTEST' DEF
ADDTEST
"#;

    let result = interp.execute(code).await;
    assert!(
        result.is_ok(),
        "Definition and call should succeed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_default_line_without_colon() {
    let mut interp = Interpreter::new();

    let code = "[ 5 ] [ 3 ] ADD";

    let result = interp.execute(code).await;
    assert!(
        result.is_ok(),
        "Default line without colon should succeed: {:?}",
        result
    );

    assert_eq!(interp.stack.len(), 1, "Stack should have one element");
    if let Some(val) = interp.stack.last() {
        assert!(val.is_vector(), "Expected vector result");
        assert_eq!(val.len(), 1, "Result should have one element");
        let only = val.child(0).expect("len==1 implies child(0) exists");
        {
            assert_eq!(
                only.as_scalar()
                    .expect("Expected scalar")
                    .numerator()
                    .to_string(),
                "8",
                "Result should be 8"
            );
        }
    }
}

#[tokio::test]
async fn test_def_with_new_syntax() {
    let mut interp = Interpreter::new();

    let code = "[ [ 42 ] ] 'ANSWER' DEF";

    let result = interp.execute(code).await;
    assert!(
        result.is_ok(),
        "DEF with new syntax should succeed: {:?}",
        result
    );

    assert!(
        interp.user_words.contains_key("ANSWER"),
        "ANSWER should be defined"
    );

    let call_result = interp.execute("ANSWER").await;
    assert!(call_result.is_ok(), "Calling ANSWER should succeed");
    assert_eq!(interp.stack.len(), 1, "Stack should have one element");
}

#[tokio::test]
async fn test_multiple_lines_without_colon() {
    let mut interp = Interpreter::new();

    let code = r#"
[ 1 ] [ 2 ] ADD
[ 3 ] MUL
"#;

    let result = interp.execute(code).await;
    assert!(
        result.is_ok(),
        "Multiple lines without colon should succeed: {:?}",
        result
    );

    assert_eq!(interp.stack.len(), 1, "Stack should have one element");
    if let Some(val) = interp.stack.last() {
        assert!(val.is_vector(), "Expected vector result");
        assert_eq!(val.len(), 1, "Result should have one element");
        let only = val.child(0).expect("len==1 implies child(0) exists");
        {
            assert_eq!(
                only.as_scalar()
                    .expect("Expected scalar")
                    .numerator()
                    .to_string(),
                "9",
                "Result should be 9"
            );
        }
    }
}

#[tokio::test]
async fn test_sequential_execution() {
    let mut interp = Interpreter::new();

    let code = r#"
[ 10 ] [ 20 ] ADD
[ 5 ] MUL
"#;

    let result = interp.execute(code).await;
    assert!(
        result.is_ok(),
        "Sequential lines should succeed: {:?}",
        result
    );

    assert_eq!(interp.stack.len(), 1, "Stack should have one element");
    if let Some(val) = interp.stack.last() {
        assert!(val.is_vector(), "Expected vector result");
        assert_eq!(val.len(), 1, "Result should have one element");
        let only = val.child(0).expect("len==1 implies child(0) exists");
        {
            assert_eq!(
                only.as_scalar()
                    .expect("Expected scalar")
                    .numerator()
                    .to_string(),
                "150",
                "Result should be 150"
            );
        }
    }
}

#[tokio::test]
async fn numbers_render_as_canonical_fractions_on_stack() {
    // Every number renders as a reduced numerator/denominator, integers
    // included. Surface literal style is not retained; `0.6 0.8 MUL` and any
    // mixed-style arithmetic therefore display uniformly.
    let cases = [
        ("1", "1/1"),
        ("0.5", "1/2"),
        ("2/1", "2/1"),
        ("4/2", "2/1"),
        ("0.6 0.8 MUL", "12/25"),
        ("3 4 ADD", "7/1"),
    ];
    for (program, expected) in cases {
        let mut interp = crate::interpreter::Interpreter::new();
        interp.execute(program).await.unwrap();
        assert_eq!(interp.stack.len(), 1, "program: {program}");
        let rendered = interp.stack[0].to_string();
        assert_eq!(
            rendered, expected,
            "`{program}` must render as canonical `{expected}`",
        );
    }
}

#[tokio::test]
async fn comparison_words_return_scalar_booleans() {
    let cases = [
        ("1 2 LT", true),
        ("2 1 GT", true),
        ("2 1 LT", false),
        ("1 1 EQ", true),
        ("1 2 EQ", false),
    ];
    for (program, expected) in cases {
        let mut interp = crate::interpreter::Interpreter::new();
        interp.execute(program).await.unwrap();
        assert_eq!(interp.stack.len(), 1, "program: {program}");
        assert_eq!(
            interp.stack[0].as_truth(),
            Some(expected),
            "program: {program}"
        );
    }
}
