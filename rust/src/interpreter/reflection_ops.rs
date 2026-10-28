//! The reflection Words: `DIGEST`, `CONTRACT`
//! (LANG.DICTIONARY.RESOLUTION, LANG.DICTIONARY.MUTATION, LANG.CONTRACT.REGISTRY,
//! LANG.CONTRACT.CHECK).
//!
//! Each reads what the machine already knows about its dictionary from inside
//! the language — the resolution execution itself performs, the content
//! identity the dictionary already keeps, the contract record the registry
//! already holds — and none of them runs anything.
//!
//! `CONTRACT` names a Word the way `DEF`, `BIND` and `DEL` do, by a String,
//! or by a Symbol (a bare name written inside `[ ]`). Looking a contract up
//! from text is safe for the acyclicity check (LANG.DICTIONARY.ACYCLIC),
//! which is complete only because no Word turns text into a *call*: reading a
//! contract calls nothing, and the answer is a Record, never a Symbol.
//!
//! `DIGEST` takes a Symbol only as a Word's name. A String is a value there,
//! digested by its denotation like any other: if `'ADD' DIGEST` answered the
//! Word's identity, the digest of a piece of text would change whenever a
//! Word of that spelling was defined, and a value's digest would no longer be
//! a function of the value (LANG.VALUES.DENOTATION).
//!
//! `CONTRACT` also takes a block, whose contract it infers the way
//! `ajisai check` does — the pre-execution check reached from
//! inside the language (LANG.CONTRACT.CHECK) — never evaluating it, so the
//! call carries none of the block's own effects.

use super::ordering_ops::{restore, take_operand};
use crate::builtins::lookup_builtin_spec;
use crate::error::{AjisaiError, NilReason, Result};
use crate::interpreter::collection_meter;
use crate::interpreter::contract_record::{contract_record, core_contract_record};
use crate::interpreter::value_digest::value_digest;
use crate::interpreter::word_identity::content_digest;
use crate::interpreter::Interpreter;
use crate::types::{Value, ValueData};
use crate::word_name::canonical_word_name;

/// Version tag for a sealed Core Word's identity. Core Words have no body to
/// normalize, so their identity is the digest of the canonical name under a
/// tag of its own, distinct from every User identity and every value digest.
const CORE_WORD_IDENTITY_TAG: &[u8] = b"AJISAI-CORE-WORD-1";

fn not_a_name(got: &str) -> AjisaiError {
    AjisaiError::declared(
        "notASymbol",
        format!("expected a name of a Word (a String or a Symbol) or a block, got {got}"),
    )
}

/// The name a Symbol resolves under: case folded.
fn canonical_name(symbol: &str) -> String {
    canonical_word_name(symbol).into_owned()
}

/// Which tier a canonical name lives in, if any.
enum Resolved {
    Core,
    User,
}

fn resolve(interp: &Interpreter, canonical: &str) -> Option<Resolved> {
    if lookup_builtin_spec(canonical).is_some() {
        Some(Resolved::Core)
    } else if interp.user_words.contains_key(canonical) {
        Some(Resolved::User)
    } else {
        None
    }
}

fn symbol_name(value: &Value) -> Option<String> {
    match &value.data {
        ValueData::Symbol(name) => Some(name.to_string()),
        _ => None,
    }
}

/// A Word's name as `CONTRACT` reads it: a Symbol, or a String spelling one.
fn word_name(value: &Value) -> Option<String> {
    match &value.data {
        ValueData::Text(text) => Some(String::from(&**text)),
        _ => symbol_name(value),
    }
}

/// `DIGEST ( [ x ] -> [ digest ] )`: a Word's content identity for a Symbol
/// naming one, the denotation digest for every other value.
pub(crate) fn op_digest(interp: &mut Interpreter) -> Result<()> {
    let operand = take_operand(interp)?;
    // Hashing the denotation walks the whole value.
    if let Err(e) = collection_meter::charge_walk_of(interp, &operand) {
        restore(interp, operand);
        return Err(e);
    }
    let word_identity = symbol_name(&operand).and_then(|name| {
        let canonical = canonical_name(&name);
        match resolve(interp, &canonical)? {
            Resolved::Core => {
                let mut bytes = CORE_WORD_IDENTITY_TAG.to_vec();
                bytes.extend_from_slice(canonical.as_bytes());
                Some(content_digest(&bytes))
            }
            Resolved::User => interp.word_identity(&canonical).cloned(),
        }
    });
    let digest = word_identity.unwrap_or_else(|| value_digest(&operand));
    interp.stack.push(Value::from_string(&digest));
    Ok(())
}

/// `CONTRACT ( [ name | code ] -> [ record ] )`: the registered contract of
/// a Core Word, the inferred contract of a User Word or of a block (never
/// evaluated); `notFound` for a name (String or Symbol) of neither.
pub(crate) fn op_contract(interp: &mut Interpreter) -> Result<()> {
    let operand = take_operand(interp)?;
    // A block's inference walks the whole block, charged before it runs.
    if operand.is_vector() {
        if let Err(e) = collection_meter::charge_walk_of(interp, &operand) {
            restore(interp, operand);
            return Err(e);
        }
    }
    if let Some(elements) = operand.as_vector_view() {
        // A block: the same inference `ajisai check` runs, over
        // the block's tokens, without running one of them.
        let tokens = match crate::interpreter::value_as_code::value_elements_to_tokens(&elements) {
            Ok(tokens) => tokens,
            Err(e) => {
                restore(interp, operand);
                return Err(e);
            }
        };
        let contract = interp.infer_contract_for_block(&tokens);
        interp.stack.push(contract_record(&contract));
        return Ok(());
    }
    let Some(name) = word_name(&operand) else {
        let got = crate::types::Value::domain_name(&operand);
        restore(interp, operand);
        return Err(not_a_name(got));
    };
    let canonical = canonical_name(&name);
    let answer = match resolve(interp, &canonical) {
        Some(Resolved::Core) => core_contract_record(&canonical),
        Some(Resolved::User) => interp
            .infer_word_contract(&canonical)
            .map(|contract| contract_record(&contract)),
        None => None,
    };
    match answer {
        Some(record) => interp.stack.push(record),
        None => interp
            .stack
            .push(Value::nil_with_reason(NilReason::NotFound)),
    }
    Ok(())
}
