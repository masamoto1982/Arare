//! Test suite for interpreter operand consumption.

use crate::interpreter::Interpreter;

#[tokio::test]
async fn a_word_consumes_its_operands() {
    let mut interp = Interpreter::new();
    let result = interp.execute("[ 1 ] [ 2 ] ADD").await;
    assert!(result.is_ok(), "ADD should succeed: {:?}", result);

    assert_eq!(
        interp.stack.len(),
        1,
        "both operands leave, the result stays"
    );
}
