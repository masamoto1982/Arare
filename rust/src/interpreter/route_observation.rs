//! What a run leaves behind that a route could change, for the tests that
//! hold two routes equal (LANG.AUTHORITY.FREEDOM): the stack, the outcome,
//! the resource usage, the epochs, the trace and the source
//! position.

use crate::interpreter::Interpreter;
use crate::types::display::render_stack;

#[derive(Debug, PartialEq)]
pub(crate) struct Observation {
    outcome: std::result::Result<(), String>,
    stack: Vec<String>,
    values: String,
    usage: crate::interpreter::ResourceUsage,
    dictionary_epoch: u64,
    trace: String,
    position: Option<crate::tokenizer::SourceSpan>,
    /// Each User Word and the body it was stored with.
    dictionary: Vec<(String, Option<String>)>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Limits {
    pub(crate) steps: Option<usize>,
    pub(crate) work: Option<u64>,
    pub(crate) bits: Option<u64>,
}

/// Run `source` on an interpreter `configure` has set a route on, under
/// `limits`.
pub(crate) fn observe(
    source: &str,
    configure: impl FnOnce(&mut Interpreter),
    limits: Limits,
) -> Observation {
    let mut interp = Interpreter::new();
    configure(&mut interp);
    if let Some(steps) = limits.steps {
        interp.set_max_execution_steps(steps);
    }
    let mut runtime = *interp.runtime_limits();
    if let Some(work) = limits.work {
        runtime.max_numeric_work = work;
    }
    if let Some(bits) = limits.bits {
        runtime.max_bigint_bits = bits;
    }
    interp.set_runtime_limits(runtime);
    let outcome = crate::agent::block_on(interp.execute(source)).map_err(|e| format!("{e:?}"));
    Observation {
        outcome: outcome.map(|_| ()),
        stack: render_stack(interp.get_stack()),
        values: crate::test_support::representation(interp.get_stack().as_slice()),
        usage: interp.resource_usage(),
        dictionary_epoch: interp.dictionary_epoch,
        trace: format!("{:?}", interp.failed_word),
        position: interp.current_source_position(),
        dictionary: {
            let mut names: Vec<String> = interp.user_words.keys().cloned().collect();
            names.sort();
            names
                .into_iter()
                .map(|name| {
                    let body = interp.lookup_word_definition_tokens(&name);
                    (name, body)
                })
                .collect()
        },
    }
}
