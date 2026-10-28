//! Every name a definition body writes, wherever it stands.
//!
//! A body is a token sequence, and most of its names are `Token::Symbol`s.
//! But a body built from a Vector (`value_as_code`) can also carry a value
//! whole as a `Token::Value` — a Record, most often — and a Symbol inside it
//! is a name the body holds just as surely: `R 'k' GET EXEC` runs it. Reading
//! only the top-level Symbols let a Word reach itself through a Record its
//! body carried, past the DEF-time acyclicity check (LANG.DICTIONARY.ACYCLIC),
//! and let its content identity ignore the Words those names resolve to
//! (spec/identity.json). Every reader of "the names a body holds" goes
//! through this one walk, so they cannot disagree about it again.

use crate::types::{Token, Value};

/// Every Symbol `body` holds, in body order: the top-level ones and every one
/// nested inside a value the body carries whole.
pub(crate) fn body_symbol_names(body: &[Token]) -> Vec<std::sync::Arc<str>> {
    let mut names = Vec::new();
    for token in body {
        match token {
            Token::Symbol(name) => names.push(name.to_arc()),
            Token::Value(value) => value_symbol_names(value, &mut names),
            Token::Number(_) | Token::String(_) | Token::VectorStart | Token::VectorEnd => {}
        }
    }
    names
}

/// Every Symbol nested anywhere inside `value`.
pub(crate) fn value_symbol_names(value: &Value, out: &mut Vec<std::sync::Arc<str>>) {
    use crate::types::ValueData;
    match &value.data {
        // A truth column holds truth values and NILs, never a name.
        ValueData::Truths(_) => {}
        ValueData::Symbol(name) => out.push(name.clone()),
        ValueData::Vector(children) => {
            for child in children.iter() {
                value_symbol_names(child, out);
            }
        }
        ValueData::Record(record) => {
            for child in record.keys().iter().chain(record.values().iter()) {
                value_symbol_names(child, out);
            }
        }
        // A dense tensor holds numbers only; the other domains hold no name.
        ValueData::Tensor { .. }
        | ValueData::Scalar(_)
        | ValueData::ExactScalar(_)
        | ValueData::Boolean(_)
        | ValueData::Text(_)
        | ValueData::Nil(_) => {}
    }
}
