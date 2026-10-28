//! The value domains each Word declares (`stack.domains` in spec/words.json)
//! against what the Word does when it runs.
//!
//! Every Word with a fixed arity is run on every combination of sample values
//! drawn from its declared operand domains, and two things must hold:
//!
//! - **The results are as declared.** Whatever value the Word answers lies in
//!   the domains declared for that result. A NIL is always allowed: a NIL
//!   result is either a NIL operand passed through or a projection, both of
//!   which other fields declare (`nilPolicy`, `projection`).
//! - **Every declared operand domain is real.** For each operand and each
//!   domain declared for it, some combination with a sample of that domain
//!   there runs without an ERROR and answers something other than NIL. A
//!   domain no sample can use is a declaration the Word does not keep.
//!
//! The fused route reads these declarations to decide which Words may run
//! inside a block (`fusion_contract`), so a wrong one is a wrong route.

use crate::interpreter::Interpreter;
use crate::kernel::generated::{Arity, GeneratedWord, ValueDomain, GENERATED_WORDS};
use crate::types::{Value, ValueData};

/// Sample programs for each domain: each pushes exactly one value.
fn samples(domain: ValueDomain) -> Vec<&'static str> {
    match domain {
        ValueDomain::Scalar => vec!["0", "1", "2", "-3", "1/2", "7", "2 SQRT"],
        ValueDomain::Boolean => vec!["TRUE", "FALSE"],
        ValueDomain::String => vec!["'a'", "'b a'", "'X'", "'W'", "'7'", "'[1,2]'", "'ADD'"],
        ValueDomain::Vector => vec![
            "[ 1 2 ]",
            "[ ]",
            "[ 3 1 2 ]",
            "[ 'a' 'b' ]",
            "[ [ 1 2 ] [ 3 4 ] ]",
            "[ 1 ADD ]",
            "[ 2 2 ]",
        ],
        // Keys of every domain a key is addressed by (LANG.RECORDS.STRUCTURE):
        // a String, a Scalar, a Boolean and a Symbol.
        ValueDomain::Record => vec![
            "[ 'a' 'b' ] [ 1 2 ] RECORD",
            "[ 'x' ] [ 5 ] RECORD",
            "[ 1 TRUE ] [ 'one' 'yes' ] RECORD",
            "[ ADD ] [ 9 ] RECORD",
        ],
        ValueDomain::Nil => vec!["NIL"],
        ValueDomain::Symbol => vec!["[ ADD ] 0 GET"],
        ValueDomain::Any => [
            ValueDomain::Scalar,
            ValueDomain::Boolean,
            ValueDomain::String,
            ValueDomain::Vector,
            ValueDomain::Record,
            ValueDomain::Nil,
            ValueDomain::Symbol,
        ]
        .into_iter()
        .flat_map(samples)
        .collect(),
    }
}

/// What a program has to set up before a Word can be used at all: `DEL` needs
/// a Word to delete. The samples then name it (`'W'`).
fn prelude(word: &GeneratedWord) -> &'static str {
    match word.name {
        "DEL" => "[ 1 ] 'W' DEF",
        _ => "",
    }
}

/// Words left out of the operand-domain witness, each for a reason no sample
/// can change.
const NOT_WITNESSED: &[(&str, &str)] = &[(
    "FAIL",
    "raises its message by definition: no operand makes it answer",
)];

fn domain_of(value: &Value) -> Option<ValueDomain> {
    if value.is_nil() {
        return None;
    }
    Some(match &value.data {
        ValueData::Boolean(_) => ValueDomain::Boolean,
        ValueData::Scalar(_) | ValueData::ExactScalar(_) => ValueDomain::Scalar,
        ValueData::Vector(_) | ValueData::Tensor { .. } | ValueData::Truths(_) => {
            ValueDomain::Vector
        }
        ValueData::Text(_) => ValueDomain::String,
        ValueData::Record(_) => ValueDomain::Record,
        ValueData::Symbol(_) => ValueDomain::Symbol,
        ValueData::Nil(_) => return None,
    })
}

fn admits(declared: &[ValueDomain], domain: ValueDomain) -> bool {
    declared.contains(&ValueDomain::Any) || declared.contains(&domain)
}

/// Every combination of samples, one per operand, with the domain each was
/// drawn from.
fn combinations(word: &GeneratedWord) -> Vec<Vec<(ValueDomain, &'static str)>> {
    let mut out: Vec<Vec<(ValueDomain, &'static str)>> = vec![Vec::new()];
    for declared in word.operand_domains {
        let options: Vec<(ValueDomain, &'static str)> = declared
            .iter()
            .flat_map(|&domain| samples(domain).into_iter().map(move |s| (domain, s)))
            .collect();
        out = out
            .into_iter()
            .flat_map(|prefix| {
                options.iter().map(move |option| {
                    let mut next = prefix.clone();
                    next.push(*option);
                    next
                })
            })
            .collect();
    }
    out
}

/// `Some(results)` when the program ran without an ERROR: the values the
/// Word left, as many as it declares.
fn run(word: &GeneratedWord, operands: &[(ValueDomain, &str)]) -> Option<Vec<Value>> {
    let source = format!(
        "{} {} {}",
        prelude(word),
        operands
            .iter()
            .map(|(_, s)| *s)
            .collect::<Vec<_>>()
            .join(" "),
        word.name
    );
    let mut interp = Interpreter::new();
    crate::agent::block_on(interp.execute(&source)).ok()?;
    let stack = interp.get_stack();
    let Arity::Fixed(outputs) = word.stack_outputs else {
        unreachable!("only fixed arities declare domains")
    };
    let outputs = outputs as usize;
    assert!(
        stack.len() >= outputs,
        "`{source}` left {} value(s) for {outputs} declared result(s)",
        stack.len()
    );
    Some(stack.as_slice()[stack.len() - outputs..].to_vec())
}

#[test]
fn every_fixed_arity_word_declares_domains() {
    for word in GENERATED_WORDS {
        let fixed = matches!(
            (word.stack_inputs, word.stack_outputs),
            (Arity::Fixed(_), Arity::Fixed(_))
        );
        let declared = !word.operand_domains.is_empty() || !word.result_domains.is_empty();
        assert_eq!(
            fixed, declared,
            "{}: domains are declared exactly for a fixed arity",
            word.name
        );
    }
}

#[test]
fn declared_domains_match_what_the_words_do() {
    let mut problems = Vec::new();
    for word in GENERATED_WORDS {
        if word.operand_domains.is_empty() && word.result_domains.is_empty() {
            continue;
        }
        let combos = combinations(word);
        // (operand index, domain) pairs some run used without ERROR or NIL.
        let mut witnessed: Vec<(usize, ValueDomain)> = Vec::new();
        for operands in &combos {
            let Some(results) = run(word, operands) else {
                continue;
            };
            let mut answered = word.result_domains.is_empty();
            for (value, declared) in results.iter().zip(word.result_domains) {
                match domain_of(value) {
                    // A NIL counts as an answer only where NIL is the declared
                    // result (`NIL`, `ABSENT`); elsewhere it is passed through
                    // or projected, which proves nothing about the operands.
                    None => answered |= declared.contains(&ValueDomain::Nil),
                    Some(domain) if admits(declared, domain) => answered = true,
                    Some(domain) => problems.push(format!(
                        "{}: `{}` answered a {domain:?}, outside its declared {declared:?}",
                        word.name,
                        operands
                            .iter()
                            .map(|(_, s)| *s)
                            .collect::<Vec<_>>()
                            .join(" ")
                    )),
                }
            }
            if answered {
                witnessed.extend(operands.iter().enumerate().map(|(i, (d, _))| (i, *d)));
            }
        }
        if NOT_WITNESSED.iter().any(|(name, _)| *name == word.name) {
            continue;
        }
        for (i, declared) in word.operand_domains.iter().enumerate() {
            for &domain in *declared {
                if !witnessed.contains(&(i, domain)) {
                    problems.push(format!(
                        "{}: operand {} declares {domain:?}, but no sample of it ran to an answer",
                        word.name,
                        i + 1
                    ));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
