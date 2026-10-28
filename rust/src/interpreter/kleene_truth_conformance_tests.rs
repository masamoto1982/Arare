//! The full strong-Kleene truth tables for `AND`/`NOT` (LANG.VALUES.TRUTH),
//! with NIL standing for UNKNOWN. Split out from `nil_conformance_tests` to
//! stay under the the file-size budget in docs/dev/specification-implementation-rules.md file-size budget.

use crate::test_support::run_ok;

/// FALSE absorbs into `AND` even against a NIL
/// operand; only where neither operand is the absorbing value does a NIL
/// operand surface in the result.
#[tokio::test]
async fn strong_kleene_and_not_truth_tables() {
    for (code, expect_nil, expect_bool) in [
        // AND: definite rows.
        ("TRUE TRUE AND", false, Some(true)),
        ("TRUE FALSE AND", false, Some(false)),
        ("FALSE TRUE AND", false, Some(false)),
        ("FALSE FALSE AND", false, Some(false)),
        // AND: FALSE absorbs a NIL operand into FALSE, from either side.
        ("FALSE NIL AND", false, Some(false)),
        ("NIL FALSE AND", false, Some(false)),
        // AND: TRUE does not settle it, so a NIL operand surfaces as UNKNOWN.
        ("TRUE NIL AND", true, None),
        ("NIL TRUE AND", true, None),
        ("NIL NIL AND", true, None),
        // NOT: no second operand to absorb into, so NIL stays NIL.
        ("TRUE NOT", false, Some(false)),
        ("FALSE NOT", false, Some(true)),
        ("NIL NOT", true, None),
    ] {
        let stack = run_ok(code).await;
        assert_eq!(stack.len(), 1, "`{code}` must leave exactly one value");
        if expect_nil {
            assert!(stack[0].is_nil(), "`{code}` must produce NIL (UNKNOWN)");
            assert_eq!(
                stack[0].nil_reason(),
                Some(&crate::error::NilReason::Literal),
                "`{code}`'s UNKNOWN is the NIL operand it read, reason intact"
            );
        } else {
            assert_eq!(
                stack[0].as_truth(),
                expect_bool,
                "`{code}` must decide {expect_bool:?}"
            );
        }
    }
}
