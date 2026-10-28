//! The higher-order Words `MAP` and `FILTER`, and the block machinery they
//! share with `higher_order_fold` (`FOLD`/`SCAN`).
//!
//! Each Word takes a `[ ... ]` code operand on top of the collection it walks,
//! compiles the block once (`extract_executable_code`) and runs it per element
//! on a scratch stack (`execute_executable_code`).

use crate::error::{AjisaiError, Result};
use crate::interpreter::fused_block::FusedBlock;
use crate::interpreter::Interpreter;
use crate::types::{ScalarColumns, Stack, Value};
use std::sync::Arc;

mod fused_cache;

/// A block, compiled once.
///
/// A Word name written as a String used to be the other way to spell a code
/// operand (`[ 1 2 3 ] 'DBL' MAP`). It is gone, and this is a struct rather
/// than an enum because of it — see `extract_executable_code`.
pub(crate) struct ExecutableCode {
    code: crate::interpreter::engine::Code,
    /// The last fused lowering of the block, kept for the next walk.
    fused: std::sync::Mutex<Option<(fused_cache::FusedKey, Arc<FusedBlock>)>>,
}

/// The last few code operands compiled, so a block that runs a higher-order
/// Word per element — `[ [ 0 ] [ ADD ] FOLD ] MAP` — compiles its inner block
/// once rather than once per element.
///
/// An entry is keyed by the identity of the operand's storage, which the entry
/// keeps alive (it holds a clone of the operand), so an equal key is the same
/// Vector; and by the one setting compiling reads besides the tokens, the
/// nesting ceiling. Compiling a block charges and records nothing, so whether a block
/// was compiled or found here is unobservable (LANG.AUTHORITY.FREEDOM).
#[derive(Default)]
pub(crate) struct BlockCache {
    entries: [Option<BlockCacheEntry>; 4],
    next: usize,
}

struct BlockCacheEntry {
    _operand: Value,
    key: BlockKey,
    code: std::sync::Arc<ExecutableCode>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
struct BlockKey {
    storage: usize,
    max_nesting_depth: usize,
}

fn block_key(interp: &Interpreter, val: &Value) -> Option<BlockKey> {
    let storage = match &val.data {
        crate::types::ValueData::Vector(items) => std::sync::Arc::as_ptr(items) as usize,
        crate::types::ValueData::Tensor { data, .. } => std::sync::Arc::as_ptr(data) as usize,
        _ => return None,
    };
    Some(BlockKey {
        storage,
        max_nesting_depth: interp.runtime_limits.max_nesting_depth,
    })
}

pub(crate) fn extract_executable_code(
    interp: &mut Interpreter,
    val: &Value,
) -> Result<std::sync::Arc<ExecutableCode>> {
    let key = block_key(interp, val);
    if let Some(key) = key {
        let hit = interp
            .block_cache
            .entries
            .iter()
            .flatten()
            .find(|entry| entry.key == key);
        if let Some(entry) = hit {
            return Ok(entry.code.clone());
        }
    }
    let code = std::sync::Arc::new(compile_executable_code(interp, val)?);
    if let Some(key) = key {
        let cache = &mut interp.block_cache;
        cache.entries[cache.next] = Some(BlockCacheEntry {
            _operand: val.clone(),
            key,
            code: code.clone(),
        });
        cache.next = (cache.next + 1) % cache.entries.len();
    }
    Ok(code)
}

fn compile_executable_code(interp: &mut Interpreter, val: &Value) -> Result<ExecutableCode> {
    // Every Vector is a code operand candidate now (CodeBlock/Vector
    // unification) — bridged back to tokens (`value_as_code.rs`).
    // `as_vector_view` (Tensor-aware) — see control.rs's EXEC for why.
    // Which of the higher-order word's two operands *is* the code one is a
    // separate question this function does not answer: callers decide by
    // stack position before reaching it (the top-of-stack operand), same as
    // before this unification.
    if let Some(elements) = val.as_vector_view() {
        let tokens = crate::interpreter::value_as_code::value_elements_to_tokens(&elements)?;
        // Compiled here, once, rather than in the caller's loop: this is the one
        // place a block becomes executable, and every higher-order Word reaches
        // it before its first element.
        let code = crate::interpreter::engine::compile(&tokens, interp);
        return Ok(ExecutableCode {
            code,
            fused: Default::default(),
        });
    }

    // A String is not code. LANG.SOURCE.CODE says code is a Vector, and
    // LANG.VALUES.DISJOINT makes String a different domain; `EXEC` has always
    // refused one here with this same category, and these Words used to accept
    // it as a Word name to call (`[ 1 2 3 ] 'DBL' MAP`).
    //
    // That spelling was the language's only dynamic call path, and it defeated
    // the DEF-time acyclicity check that LANG.DICTIONARY.ACYCLIC's termination
    // argument rests on: the name could be *computed*, so it appeared in no
    // token of the body for the check to see. `[ [ 1 ] [ 'J' 'W' ] JOIN MAP ]
    // 'JW' DEF` was accepted and then recursed until the native depth guard
    // fired — termination decided by a ceiling, which that clause says it is
    // not. A Symbol cannot be built this way (no Word turns text into one), so
    // refusing the String closes the path rather than narrowing it.
    Err(AjisaiError::declared(
        "notExecutable",
        format!(
            "expected a Vector ([ ... ]) as the code operand, got {}",
            val.domain_name()
        ),
    ))
}

/// Whether a higher-order block's predicate result keeps its element, read in
/// truth position (LANG.VALUES.TRUTH): TRUE keeps it, and FALSE and UNKNOWN —
/// a NIL read here, whatever its reason — do not, since only a predicate that
/// holds selects, as SQL's `WHERE` keeps only the rows whose condition is
/// true.
///
/// The domains are disjoint (LANG.VALUES.DISJOINT), so nothing else is a truth
/// value: a scalar is not a Boolean even when it is non-zero, and a singleton
/// Vector is not its element. Each of those used to be accepted here, which
/// gave `FILTER` a truthiness rule no other Word shared — `[ 1 2 3 ] [ 1 ]
/// FILTER` silently kept every element instead of raising `nonTruthValue`, the
/// same declared condition `AND`/`NOT` raise for the identical fault. A NIL, by
/// contrast, is UNKNOWN in truth position for every Word that reads one, so
/// here too.
pub(crate) fn extract_predicate_boolean(condition_result: Value) -> Result<bool> {
    if let Some(b) = condition_result.as_truth() {
        return Ok(b);
    }
    if condition_result.is_nil() {
        return Ok(false);
    }

    Err(AjisaiError::declared(
        "nonTruthValue",
        format!(
            "expected a truth value from the predicate block, got {}",
            condition_result.domain_name()
        ),
    ))
}

pub(crate) fn execute_executable_code(
    interp: &mut Interpreter,
    exec: &ExecutableCode,
) -> Result<()> {
    interp.run_block(&exec.code)
}

pub fn op_filter(interp: &mut Interpreter) -> Result<()> {
    run_element_walk::<FilterWalk>(
        interp,
        "FILTER",
        crate::interpreter::fused_block::FusedWalk::Filter,
    )
}

/// The one value a block left on its scratch stack, or the count it left
/// instead (LANG.COLLECTIONS.HIGHER). A surplus used to be discarded and the
/// top taken, so `[ 1 2 3 ] [ 1 GT TRUE ] FILTER` kept every element on the
/// strength of the `TRUE` above the comparison — a quiet wrong answer.
pub(super) fn block_result(interp: &mut Interpreter) -> std::result::Result<Value, usize> {
    match interp.stack.len() {
        1 => Ok(interp.stack.pop().expect("one value")),
        left => Err(left),
    }
}

/// `expected the block to leave one value, and it left none` / `… left 2`.
pub(super) fn block_arity_message(expected: &str, left: usize) -> String {
    let count = if left == 0 {
        "none".to_string()
    } else {
        left.to_string()
    };
    format!("expected {expected}, and it left {count}")
}

pub fn op_map(interp: &mut Interpreter) -> Result<()> {
    run_element_walk::<MapWalk>(
        interp,
        "MAP",
        crate::interpreter::fused_block::FusedWalk::Map,
    )
}

/// What one element-wise walk does with each element's answer, and what it
/// answers at the end. `MAP` and `FILTER` differ in nothing else: the operand
/// handling, the NIL passthrough, the empty case, the fused attempt and the
/// per-element scratch-stack loop are `run_element_walk`'s, as `FOLD`/`SCAN`
/// share `run_accumulator_walk`. The two used to carry a copy each of that
/// sixty-line frame.
trait ElementWalk {
    /// What the block owes: the one value `block_arity_message` names when
    /// the block leaves another count.
    const EXPECTED_RESULT: &'static str;

    /// A walk over `n_elements` elements, with room for its answers.
    fn with_capacity(n_elements: usize) -> Self;

    /// Called with the element the block was given (by its index in `target`)
    /// and the one value the block left.
    fn visit(&mut self, target: &Value, index: usize, result: Value) -> Result<()>;

    /// The Word's answer once every element has been visited.
    fn finish(self) -> Value;
}

struct FilterWalk {
    kept: Vec<Value>,
}

impl ElementWalk for FilterWalk {
    const EXPECTED_RESULT: &'static str = "the predicate block to leave one truth value";

    fn with_capacity(n_elements: usize) -> Self {
        FilterWalk {
            kept: Vec::with_capacity(n_elements),
        }
    }

    fn visit(&mut self, target: &Value, index: usize, result: Value) -> Result<()> {
        if extract_predicate_boolean(result)? {
            self.kept.push(
                target
                    .child(index)
                    .expect("FILTER: child index in 0..len must be valid"),
            );
        }
        Ok(())
    }

    fn finish(self) -> Value {
        Value::from_vector_promoted(self.kept)
    }
}

/// The answers, as columns while every one is a plain scalar (a block that
/// answers numbers does, almost always) and as a list from the first that is
/// not: the Tensor is the one `from_vector_promoted` builds from the list.
struct MapWalk {
    n_elements: usize,
    columns: Option<ScalarColumns>,
    results: Vec<Value>,
}

impl ElementWalk for MapWalk {
    const EXPECTED_RESULT: &'static str = "the block to leave one value";

    fn with_capacity(n_elements: usize) -> Self {
        MapWalk {
            n_elements,
            columns: Some(ScalarColumns::with_capacity(n_elements)),
            results: Vec::new(),
        }
    }

    // The block's one result *is* the mapped element, whatever its shape. A
    // one-element Vector used to be unwrapped here, back when a scalar was
    // itself a one-element Vector and the two were indistinguishable. They
    // are separate domains now (LANG.VALUES.DISJOINT), and the unwrapping
    // outlived its reason: `[ 1 2 ] { 1 COLLECT } MAP` answered `[ 1/1 2/1 ]`,
    // so a block asking in as many words for a Vector of one got a scalar,
    // and there was no way at all to map to singletons. Worse, it was silent
    // and unequal — `[ [ 1 ] ] { REVERSE } MAP 0 GET 5 ADD` answered `6/1`
    // where `[ 6/1 ]` is the answer, which is exactly the quiet wrong result
    // LANG.FAILURE.TRICHOTOMY exists to rule out.
    fn visit(&mut self, _target: &Value, _index: usize, result: Value) -> Result<()> {
        match self.columns.as_mut() {
            Some(columns) => {
                if !columns.push(&result) {
                    self.results = self
                        .columns
                        .take()
                        .expect("checked above")
                        .into_values(self.n_elements);
                    self.results.push(result);
                }
            }
            None => self.results.push(result),
        }
        Ok(())
    }

    fn finish(self) -> Value {
        match self.columns {
            Some(columns) => columns.finish(),
            None => Value::from_vector_promoted(self.results),
        }
    }
}

fn run_element_walk<W: ElementWalk>(
    interp: &mut Interpreter,
    word: &'static str,
    fused_walk: crate::interpreter::fused_block::FusedWalk,
) -> Result<()> {
    let code_val: Value = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    let executable = match extract_executable_code(interp, &code_val) {
        Ok(exec) => exec,
        Err(e) => {
            interp.stack.push(code_val);
            return Err(e);
        }
    };

    let target_val: Value = interp.stack.pop().ok_or_else(|| {
        interp.stack.push(code_val.clone());
        AjisaiError::stack_underflow()
    })?;

    if target_val.is_nil() {
        interp
            .stack
            .push(Value::nil_inheriting_absence_from(&target_val));
        return Ok(());
    }

    if !target_val.is_vector() {
        let got = target_val.domain_name();
        interp.stack.push(target_val);
        interp.stack.push(code_val);
        return Err(AjisaiError::declared(
            "nonVector",
            format!("expected a Vector, got {got}"),
        ));
    }

    let n_elements: usize = target_val.len();
    if n_elements == 0 {
        interp.stack.push(Value::from_vector(Vec::new()));
        return Ok(());
    }

    if let Some(result) = executable
        .fused(interp, 1)
        .and_then(|block| block.run(interp, fused_walk, &target_val, None))
    {
        interp.stack.push(result);
        return Ok(());
    }

    let mut walk = W::with_capacity(n_elements);
    let mut saved_stack: Stack = Stack::new();
    std::mem::swap(&mut interp.stack, &mut saved_stack);
    let saved_no_change_check: bool = interp.disable_no_change_check;
    interp.disable_no_change_check = true;

    let mut error: Option<AjisaiError> = None;
    let mut children = target_val.children_reader();
    for i in 0..n_elements {
        let elem: Value = children
            .get(i)
            .unwrap_or_else(|| panic!("{word}: child index in 0..len must be valid"));
        interp.stack.clear();
        interp.stack.push(elem);
        match execute_executable_code(interp, &executable) {
            Ok(_) => match block_result(interp) {
                Ok(result) => {
                    if let Err(e) = walk.visit(&target_val, i, result) {
                        error = Some(e);
                        break;
                    }
                }
                Err(left) => {
                    error = Some(AjisaiError::declared(
                        "blockContractViolation",
                        block_arity_message(W::EXPECTED_RESULT, left),
                    ));
                    break;
                }
            },
            Err(e) => {
                error = Some(e);
                break;
            }
        }
    }
    interp.disable_no_change_check = saved_no_change_check;
    interp.stack = saved_stack;

    if let Some(e) = error {
        interp.stack.push(target_val);
        interp.stack.push(code_val);
        return Err(e);
    }

    interp.stack.push(walk.finish());
    Ok(())
}
