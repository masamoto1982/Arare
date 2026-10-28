//! One lifting rule for every Word (LANG.COLLECTIONS.LIFT).
//!
//! A Word whose operand is read as one Scalar, String or Boolean (`leaf`) or
//! in truth position (`truth`) applies to each element when that operand is a
//! Vector or a Record. This runs every such Word both ways and requires the
//! lifted answer to be the per-element answers, assembled in the same
//! container. The sample table has to cover every Word the registry marks,
//! so a new lifted operand without a law fails here.

use ajisai_core::interpreter::Interpreter;
use ajisai_core::kernel::generated::{OperandRole, GENERATED_WORDS};

const R: &str = "[ 'a' 'b' ] [ 1 2 ] RECORD";

/// Two operand tuples per Word, differing only at its lifted positions.
fn samples(word: &str) -> Option<(Vec<&'static str>, Vec<&'static str>)> {
    let pair = |a: &[&'static str], b: &[&'static str]| Some((a.to_vec(), b.to_vec()));
    match word {
        "ADD" | "SUB" | "MUL" | "DIV" | "MIN" | "MAX" | "LT" | "GT" => {
            pair(&["1", "2"], &["3", "4"])
        }
        "POW" => pair(&["2", "2"], &["3", "2"]),
        "GCD" => pair(&["4", "6"], &["9", "6"]),
        "FLOOR" | "ROUND" => pair(&["1/2"], &["3/2"]),
        "SQRT" => pair(&["4"], &["2"]),
        "RATIO" => pair(&["1/2"], &["3/4"]),
        "AND" => pair(&["TRUE", "TRUE"], &["FALSE", "TRUE"]),
        "NOT" => pair(&["TRUE"], &["FALSE"]),
        "SELECT" => pair(&["1", "2", "TRUE"], &["1", "2", "FALSE"]),
        "GET" => pair(&["[ 10 20 ]", "0"], &["[ 10 20 ]", "1"]),
        "TAKE" | "DROP" => pair(&["[ 1 2 3 ]", "1"], &["[ 1 2 3 ]", "2"]),
        "PUT" => pair(&["[ 1 2 ]", "0", "9"], &["[ 1 2 ]", "1", "9"]),
        "BSEARCH" => pair(&["[ 1 3 5 ]", "3"], &["[ 1 3 5 ]", "5"]),
        "WITHOUT" | "HAS?" => pair(&[R, "'a'"], &[R, "'b'"]),
        "CHARS" | "UPPER" | "LOWER" => pair(&["'ab'"], &["'c'"]),
        "TRIM" => pair(&["' a '"], &["'b '"]),
        "NUM" => pair(&["'1'"], &["'x'"]),
        "STR" => pair(&["1/2"], &["TRUE"]),
        "JSON-DECODE" => pair(&["'1'"], &["'[ 1 ]'"]),
        "TOKENIZE" => pair(&["'a,b'", "','"], &["'c'", "','"]),
        "SEARCH" => pair(&["'ab'", "'b'"], &["'cd'", "'c'"]),
        "REPLACE" => pair(&["'ab'", "'a'", "'x'"], &["'cd'", "'c'", "'y'"]),
        "FORMAT" => pair(&["1/3", "2"], &["2/3", "1"]),
        "RANGE" => pair(&["0", "2"], &["3", "1"]),
        "FILL" => pair(&["[ 2 ]", "0"], &["[ 2 ]", "1"]),
        _ => None,
    }
}

async fn render(source: &str) -> String {
    let mut interp = Interpreter::new();
    interp
        .execute(source)
        .await
        .unwrap_or_else(|e| panic!("`{source}` should run: {e}"));
    interp
        .get_stack()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

#[tokio::test]
async fn a_lifted_operand_applies_the_word_to_each_element() {
    let mut missing = Vec::new();
    for word in GENERATED_WORDS {
        let lifted: Vec<bool> = word
            .operand_roles
            .iter()
            .map(|role| matches!(role, OperandRole::Leaf | OperandRole::Truth))
            .collect();
        if !lifted.contains(&true) {
            continue;
        }
        let Some((a, b)) = samples(word.name) else {
            missing.push(word.name);
            continue;
        };
        let name = word.name;
        let each = format!("{} {name} {} {name}", a.join(" "), b.join(" "));

        let over_vector: Vec<String> = a
            .iter()
            .zip(&b)
            .zip(&lifted)
            .map(|((x, y), l)| {
                if *l {
                    format!("{x} {y} 2 COLLECT")
                } else {
                    x.to_string()
                }
            })
            .collect();
        assert_eq!(
            render(&format!("{} {name}", over_vector.join(" "))).await,
            render(&format!("{each} 2 COLLECT")).await,
            "{name} over a Vector"
        );

        let over_record: Vec<String> = a
            .iter()
            .zip(&b)
            .zip(&lifted)
            .map(|((x, y), l)| {
                if *l {
                    format!("[ 'x' 'y' ] {x} {y} 2 COLLECT RECORD")
                } else {
                    x.to_string()
                }
            })
            .collect();
        assert_eq!(
            render(&format!("{} {name}", over_record.join(" "))).await,
            render(&format!("[ 'x' 'y' ] {each} 2 COLLECT RECORD")).await,
            "{name} over a Record"
        );
    }
    assert!(missing.is_empty(), "no lifting sample for {missing:?}");
}
