//! LANG.CONTRACT.FIELD, held against the implementation.
//!
//! Every Core Word declares in `spec/words.json` whether it keeps numbers in
//! the field (`closed`) or can answer one of the two points over zero from
//! operands that hold none (`leaving`). The declaration is what the contract
//! inference joins along a body, so a wrong one would make `field=closed` a
//! promise the language breaks. Three laws keep it honest:
//!
//! 1. every `leaving` Word has a witness that leaves the field from finite
//!    operands, and the witness table names exactly the `leaving` Words;
//! 2. every `closed` exact-arithmetic Word, run over finite operands —
//!    rational and irrational, signed and zero — never answers a point over
//!    zero;
//! 3. a one-Word block infers the field its Word declares, so the inference
//!    reads the declaration rather than a second copy of it;
//! 4. a `#:contract` declaration of `field=closed` is a bound the check holds a
//!    body to before anything runs;
//! 5. a `DIV` by a finite non-zero literal and a `POW` to a finite
//!    non-negative literal exponent are inferred `closed`, and running every
//!    such call over finite operands confirms it never leaves the field;
//! 6. a `leaving` body names where it leaves.

use crate::agent::api::check;
use crate::agent::contract::contracts_json;
use crate::coreword_registry::FieldClosure;
use crate::interpreter::Interpreter;
use crate::kernel::generated::{Arity, Family, GENERATED_WORDS};
use crate::types::{Value, ValueData};

/// One program per `leaving` Word that answers a point over zero from
/// operands holding none.
const LEAVING_WITNESSES: &[(&str, &str)] = &[
    ("DIV", "1 0 DIV"),
    ("POW", "0 -1 POW"),
    ("NUM", "'1/0' NUM"),
];

/// Finite operands: zero, both signs, a proper fraction and an irrational.
const FINITE_OPERANDS: &[&str] = &["0", "1", "-2", "3/4", "-1/2", "2 SQRT", "[ 0 -3 5/2 ]"];

fn holds_point_over_zero(value: &Value) -> bool {
    match &value.data {
        ValueData::Scalar(f) => !f.is_finite(),
        ValueData::Vector(items) => items.iter().any(holds_point_over_zero),
        ValueData::Tensor { data, .. } => !data.all_finite(),
        ValueData::Record(record) => record
            .keys()
            .iter()
            .chain(record.values())
            .any(holds_point_over_zero),
        _ => false,
    }
}

async fn stack_of(code: &str) -> Option<Vec<Value>> {
    let mut interp = Interpreter::new();
    interp.execute(code).await.ok()?;
    Some(interp.get_stack().to_vec())
}

#[tokio::test]
async fn every_leaving_word_has_a_witness_and_only_those_do() {
    let declared: Vec<&str> = GENERATED_WORDS
        .iter()
        .filter(|w| w.field == FieldClosure::Leaving)
        .map(|w| w.name)
        .collect();
    let witnessed: Vec<&str> = LEAVING_WITNESSES.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        declared, witnessed,
        "the `leaving` Words of spec/words.json and the witness table must name the same Words"
    );
    for (name, code) in LEAVING_WITNESSES {
        let stack = stack_of(code)
            .await
            .unwrap_or_else(|| panic!("witness `{code}` for {name} must run"));
        assert!(
            stack.iter().any(holds_point_over_zero),
            "witness `{code}` must leave the field, so {name} is `leaving`"
        );
    }
}

#[tokio::test]
async fn closed_arithmetic_stays_in_the_field_over_finite_operands() {
    for word in GENERATED_WORDS
        .iter()
        .filter(|w| w.field == FieldClosure::Closed && w.family == Family::KnowledgeOrder)
    {
        let Arity::Fixed(arity) = word.stack_inputs else {
            continue;
        };
        let mut combos: Vec<Vec<&str>> = vec![Vec::new()];
        for _ in 0..arity {
            combos = combos
                .into_iter()
                .flat_map(|prefix| {
                    FINITE_OPERANDS.iter().map(move |operand| {
                        let mut next = prefix.clone();
                        next.push(operand);
                        next
                    })
                })
                .collect();
        }
        for operands in combos {
            let code = format!("{} {}", operands.join(" "), word.name);
            // A refused operand raises; that is no number at all.
            if let Some(stack) = stack_of(&code).await {
                assert!(
                    !stack.iter().any(holds_point_over_zero),
                    "{} is declared `closed` but `{code}` left the field",
                    word.name
                );
            }
        }
    }
}

#[tokio::test]
async fn a_one_word_block_infers_the_field_its_word_declares() {
    for word in GENERATED_WORDS {
        let code = format!("[ {name} ] CONTRACT 'field' GET", name = word.name);
        let stack = stack_of(&code)
            .await
            .unwrap_or_else(|| panic!("`{code}` must run"));
        let texts: Vec<String> = stack
            .iter()
            .map(|v| match &v.data {
                ValueData::Text(t) => t.to_string(),
                other => panic!("`{code}` answered {other:?}"),
            })
            .collect();
        // A Word that runs a block it is handed knows nothing of that block
        // here, so the inference claims the weaker bound: it may leave.
        let expected = if matches!(word.name, "MAP" | "FILTER" | "FOLD" | "SCAN" | "EXEC") {
            "leaving"
        } else {
            word.field.as_spec_str()
        };
        assert_eq!(
            texts[0], expected,
            "a block holding only {} must infer {expected}",
            word.name
        );
    }
}

#[tokio::test]
async fn a_literal_over_zero_leaves_the_field_wherever_it_is_written() {
    for (code, expected) in [
        ("[ 2 MUL 1 ADD ] CONTRACT 'field' GET", "closed"),
        ("[ 1/0 MIN ] CONTRACT 'field' GET", "leaving"),
        ("[ [ 0/0 ] CONCAT ] CONTRACT 'field' GET", "leaving"),
        ("[ [ 2 MUL ] MAP ] CONTRACT 'field' GET", "closed"),
        ("[ [ 0 DIV ] MAP ] CONTRACT 'field' GET", "leaving"),
    ] {
        let stack = stack_of(code).await.expect("CONTRACT runs");
        let answer = match &stack.last().expect("an answer").data {
            ValueData::Text(t) => t.to_string(),
            other => panic!("`{code}` answered {other:?}"),
        };
        assert_eq!(answer, expected, "`{code}`");
    }
}

fn field_decls(source: &str) -> serde_json::Value {
    check(source).to_json()["contractDecls"].clone()
}

fn decl_exit_code(source: &str) -> i32 {
    check(source).exit_code()
}

// -----------------------------------------------------------------
// `field` (LANG.CONTRACT.FIELD): a declared `closed` is a promise that the
// Word never answers a point over zero from operands that hold none.
// -----------------------------------------------------------------

#[test]
fn a_closed_field_declaration_is_verified_over_field_arithmetic() {
    let source = "[ 2 MUL 3 ADD 2 SQRT MUL ] 'W' DEF\n#:contract W field=closed";
    let decls = field_decls(source);
    assert_eq!(decls["outcome"], "value", "{decls}");
    assert_eq!(decl_exit_code(source), 0);
}

#[test]
fn a_closed_field_declaration_over_a_division_is_violated() {
    for body in ["[ 1 ] LENGTH DIV", "1/0 MIN", "-1 POW", "STR NUM"] {
        let source = format!("[ {body} ] 'W' DEF\n#:contract W field=closed");
        let decls = field_decls(&source);
        assert_eq!(decls["outcome"], "error", "body: {body}: {decls}");
        assert!(
            decls.to_string().contains("field=leaving"),
            "body: {body}: {decls}"
        );
    }
}

#[test]
fn a_leaving_field_declaration_admits_a_closed_body() {
    let source = "[ 1 ADD ] 'W' DEF\n#:contract W field=leaving";
    assert_eq!(field_decls(source)["outcome"], "value");
}

#[test]
fn a_field_value_outside_the_vocabulary_is_malformed() {
    let source = "[ 1 ADD ] 'W' DEF\n#:contract W field=finite";
    assert_ne!(decl_exit_code(source), 0);
}

/// Literal operands the inference may read past `DIV` / `POW`.
const LITERAL_DIVISORS: &[&str] = &["1", "-2", "3/4", "-1/2", "7"];
const LITERAL_EXPONENTS: &[&str] = &["0", "1", "2", "3", "1/2", "3/2"];

#[tokio::test]
async fn a_literal_operand_that_cannot_leave_the_field_keeps_the_call_closed() {
    for (word, literals) in [("DIV", LITERAL_DIVISORS), ("POW", LITERAL_EXPONENTS)] {
        for literal in literals {
            let block = format!("[ {literal} {word} ]");
            let stack = stack_of(&format!("{block} CONTRACT 'field' GET"))
                .await
                .expect("CONTRACT runs");
            assert!(
                matches!(&stack[0].data, ValueData::Text(t) if &**t == "closed"),
                "{block} must infer closed"
            );
            // The inference's claim, checked by running it.
            for operand in FINITE_OPERANDS {
                let code = format!("{operand} {literal} {word}");
                if let Some(stack) = stack_of(&code).await {
                    assert!(
                        !stack.iter().any(holds_point_over_zero),
                        "`{code}` left the field although {block} is inferred closed"
                    );
                }
            }
        }
    }
    for block in [
        "[ 0 DIV ]",
        "[ -1 POW ]",
        "[ 1/0 DIV ]",
        "[ [ 1 ] LENGTH DIV ]",
        "[ LENGTH 1 SUB DIV ]",
    ] {
        let stack = stack_of(&format!("{block} CONTRACT 'field' GET"))
            .await
            .expect("CONTRACT runs");
        assert!(
            matches!(&stack[0].data, ValueData::Text(t) if &**t == "leaving"),
            "{block} must stay leaving: its operand is not a literal that decides it"
        );
    }
}

#[test]
fn a_leaving_body_names_where_it_leaves() {
    let reports = contracts_json(
        "[ 'xs' BIND xs 0 [ ADD ] FOLD xs LENGTH DIV ] 'MEAN' DEF\n\
         [ 2 DIV ] 'HALF' DEF\n\
         [ MEAN HALF 1/0 MIN ] 'SPREAD' DEF",
    );
    let exits = |name: &str| {
        let r = reports
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == name)
            .unwrap_or_else(|| panic!("no report for {name}"));
        (r["field"].clone(), r["fieldExits"].clone())
    };
    assert_eq!(
        exits("MEAN"),
        ("leaving".into(), serde_json::json!(["DIV"]))
    );
    assert_eq!(exits("HALF"), ("closed".into(), serde_json::json!([])));
    assert_eq!(
        exits("SPREAD"),
        ("leaving".into(), serde_json::json!(["MEAN", "1/0"]))
    );
    let decls = field_decls("[ [ 1 ] LENGTH DIV ] 'W' DEF\n#:contract W field=closed");
    assert!(
        decls.to_string().contains("at `DIV`"),
        "the violation names the exit: {decls}"
    );
}
