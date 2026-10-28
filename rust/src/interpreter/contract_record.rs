//! A Word's contract as a Record — the one shape `CONTRACT` answers for a Core
//! Word, a User Word and a block (LANG.CONTRACT.REGISTRY): `inputs` and
//! `outputs`, each a count or `'variable'`, and `field`, `'closed'` or
//! `'leaving'`. A Core Word's is registered and a User Word's or a block's is
//! inferred, in the same keys and the same vocabulary, so the two compare.

use crate::interpreter::word_contract::{ContractFlow, WordContract};
use crate::kernel::generated::{generated_word, Arity, FieldClosure};
use crate::types::{RecordData, Value};

/// The inferred contract of a User Word or a block, as a Record.
pub(crate) fn contract_record(contract: &WordContract) -> Value {
    let (inputs, outputs) = match contract.flow {
        ContractFlow::Fixed { consumes, produces } => (
            Value::from_int(i64::from(consumes)),
            Value::from_int(i64::from(produces)),
        ),
        ContractFlow::Dynamic => (
            Value::from_string("variable"),
            Value::from_string("variable"),
        ),
    };
    record(inputs, outputs, contract.field)
}

/// The registered contract of a Core Word, read off the arity
/// `spec/words.json` declares rather than off the mass contract the
/// analyzers use: a Word whose arity the data decides on one side keeps
/// its fixed count on the other — `COLLECT` answers one Vector and `EXEC`
/// reads one block — which `ContractFlow::Dynamic` cannot carry.
pub(crate) fn core_contract_record(canonical: &str) -> Option<Value> {
    let word = generated_word(canonical)?;
    let arity = |arity: Arity| match arity {
        Arity::Fixed(count) => Value::from_int(i64::from(count)),
        Arity::Variable => Value::from_string("variable"),
    };
    Some(record(
        arity(word.stack_inputs),
        arity(word.stack_outputs),
        word.field,
    ))
}

fn record(inputs: Value, outputs: Value, field: FieldClosure) -> Value {
    let keys = ["inputs", "outputs", "field"]
        .map(Value::from_string)
        .to_vec();
    let values = vec![inputs, outputs, Value::from_string(field.as_spec_str())];
    Value::from_record(RecordData::new(keys, values).expect("contract keys are distinct"))
}
