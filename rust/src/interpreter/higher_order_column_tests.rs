//! `MAP`'s unfused loop collects plain scalar answers into columns as they
//! arrive (`ScalarColumns`). Whatever the answers are, the result must be what
//! promoting the list of them builds: the same Tensor, or the same Vector.

use crate::interpreter::Interpreter;
use crate::types::Value;

#[test]
fn map_answers_what_promoting_its_answers_builds() {
    let blocks = [
        "[ [ 7 ] LENGTH ADD ]",
        "[ [ 7 ] LENGTH ADD 3 DIV ]",
        "[ [ 7 ] LENGTH 7 SUB ADD 4611686018427387904 MUL ]",
        "[ [ 7 ] LENGTH 7 SUB ADD 99999999999999999999 MUL ]",
        "[ [ 7 ] LENGTH 7 SUB ADD 'X' BIND 6 X DIV ]",
        "[ [ 7 ] LENGTH 7 SUB ADD 3 GT ]",
        "[ [ 7 ] LENGTH 7 SUB ADD 2 SQRT MUL ]",
        "[ [ 7 ] LENGTH 7 SUB ADD 'X' BIND [ X X ] ]",
        "[ [ 7 ] LENGTH 7 SUB ADD 'X' BIND X 2 GT ]",
        "[ 'X' BIND X [ 5 ] LENGTH X 2 GT SELECT ]",
        "[ 'X' BIND X 3 GT 'a' [ 7 ] LENGTH SELECT ]",
        "[ 'X' BIND [ 7 ] LENGTH X 2 GT [ 1 2 ] SWAP SELECT ]",
    ];
    for block in blocks {
        for target in ["[ 1 2 3 4 5 ]", "[ 1/2 3 7/3 ]", "[ 5 ]", "[ 0 1 2 ]"] {
            let run = |source: &str| {
                let mut interp = Interpreter::new();
                let _ = crate::agent::block_on(interp.execute(source));
                format!("{:?}", interp.get_stack().as_slice())
            };
            let mapped = run(&format!("{target} {block} MAP"));
            // The reference: each element through the block on its own,
            // the answers promoted as a list.
            let mut interp = Interpreter::new();
            let elements = crate::agent::block_on(interp.execute(target))
                .is_ok()
                .then(|| interp.get_stack().last().cloned().expect("a target"));
            let target_value = elements.expect("target runs");
            let mut answers = Vec::new();
            let mut failed = false;
            for i in 0..target_value.len() {
                let mut one = Interpreter::new();
                let element = target_value.child(i).unwrap();
                one.stack.push(element);
                let body = block.trim_start_matches("[ ").trim_end_matches(" ]");
                if crate::agent::block_on(one.execute(body)).is_err() {
                    failed = true;
                    break;
                }
                answers.push(one.stack.pop().expect("one answer"));
            }
            if failed {
                continue;
            }
            let expected = {
                let mut i2 = Interpreter::new();
                i2.stack.push(Value::from_vector_promoted(answers));
                format!("{:?}", i2.get_stack().as_slice())
            };
            assert_eq!(mapped, expected, "`{target} {block} MAP`");
        }
    }
}
