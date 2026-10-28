//! Which Words may run inside a fused block, read from their contracts, and
//! the plain law each runs there.
//!
//! A fused walk (`fused_block`) holds only plain values — rationals and
//! Booleans — and must reproduce everything the interpreted walk would have
//! done. A Word can take part when its contract (spec/words.json, projected in
//! `kernel::generated`) says so:
//!
//! - it is pure and deterministic and declares no effects;
//! - it takes a fixed number of operands, none of them a `control` operand
//!   (a block, a name, a message), and answers exactly one value;
//! - every operand's declared domains include a scalar or a Boolean, and
//!   every result's are scalars, Booleans, `any` — a value the Word picks
//!   from its operands, as `SELECT` does — or a domain an operand also
//!   declares, which a plain operand never is (`MIN` over two Strings)
//!   (`stack.domains`, LANG.VALUES.DISJOINT);
//! - it costs exactly one execution step whatever its operands
//!   (`cost.steps`), as every op of a fused block is charged.
//!
//! The other cost axes are upper bounds over every operand, containers
//! included (`SELECT` declares linear collection work for the masks it lifts
//! over), so they do not say what a plain call costs. That is the law's own
//! report, and the route-equality suites compare it with the interpreted
//! route's, collection work included.
//!
//! The contract says a Word *may* run on plain values; what it does there is
//! the Word's own law. Most admitted Words have a hand-written op of their
//! own in every tier (`fused_block_lower::word_op`). Any other admitted Word
//! runs through [`PlainKernel`]: the scalar law its dispatch calls, applied
//! to plain operands, with the charges that dispatch makes for it. A kernel
//! that meets anything the law would not answer with a plain value — an
//! ERROR, a NIL, an irrational — declines, and the walk falls back to the
//! interpreted one, which reports it.
//!
//! `fusion_contract_tests` holds the three sides together: every Word with a
//! hand-written op or a kernel is admitted by its contract; every admitted
//! Word has one, or is listed with the reason it has none; and each kernel
//! answers as its dispatch does.

use crate::interpreter::fused_block::Plain;
use crate::interpreter::Interpreter;
use crate::kernel::generated::{
    generated_word_by_id, Arity, CostClass, Determinism, GeneratedWord, OperandRole, Purity,
    ValueDomain, WordId,
};
use crate::types::Value;
use num_integer::Integer;

/// Whether `word`'s contract lets it run on plain values inside a fused
/// block. See the module documentation for the conditions.
pub(crate) fn admits(word: &GeneratedWord) -> bool {
    let plain = |d: &ValueDomain| {
        matches!(
            d,
            ValueDomain::Scalar | ValueDomain::Boolean | ValueDomain::Any
        )
    };
    let operands_fit = word.operand_domains.iter().all(|set| set.iter().any(plain));
    // `any` is a result the Word picks from its operands (`SELECT`): plain
    // when they are, and a walk that meets anything else declines it. A result
    // domain that is not plain is admitted on the same ground when an operand
    // carries it too — `MIN` answers a String only from String operands, which
    // a walk never holds.
    let from_operand = |d: &ValueDomain| word.operand_domains.iter().any(|set| set.contains(d));
    let results_fit = word
        .result_domains
        .iter()
        .all(|set| set.iter().all(|d| plain(d) || from_operand(d)) && set.iter().any(plain));
    word.purity == Purity::Pure
        && word.determinism == Determinism::Deterministic
        && word.effects.is_empty()
        && matches!(word.stack_inputs, Arity::Fixed(_))
        && word.stack_outputs == Arity::Fixed(1)
        && !word.operand_roles.contains(&OperandRole::Control)
        && word.operand_domains.len() == word.operand_roles.len()
        && operands_fit
        && results_fit
        && word.cost.steps.class == CostClass::Const
        && word.cost.steps.exact
}

/// What a kernel answered: the value and the numeric work the dispatch would
/// have charged for it, beside its one step.
pub(crate) struct Answer {
    pub(crate) value: Plain,
    pub(crate) work: u64,
}

/// A Word's law on plain operands, first-pushed first.
#[derive(Debug)]
pub(crate) struct PlainKernel {
    pub(crate) word: WordId,
    pub(crate) arity: usize,
    pub(crate) apply: fn(&[Plain], &Interpreter) -> Option<Answer>,
}

/// The Words that run through a kernel. A Word is here only when its contract
/// admits it and it has no hand-written op; `fusion_contract_tests` checks
/// both.
static KERNELS: &[PlainKernel] = &[
    PlainKernel {
        word: WordId::Gcd,
        arity: 2,
        apply: gcd,
    },
    PlainKernel {
        word: WordId::NilCheck,
        arity: 1,
        apply: nil_check,
    },
    PlainKernel {
        word: WordId::Depth,
        arity: 1,
        apply: depth,
    },
];

/// The kernel for `word`, when its contract admits it and one is written.
pub(crate) fn kernel(word: WordId) -> Option<&'static PlainKernel> {
    let kernel = KERNELS.iter().find(|k| k.word == word)?;
    debug_assert!(admits(generated_word_by_id(word)));
    Some(kernel)
}

/// Every kernel, for the tests that hold them to their contracts.
#[cfg(test)]
pub(crate) fn kernels() -> &'static [PlainKernel] {
    KERNELS
}

/// A value the Word's law answered, as a plain value with no other charge.
fn uncharged(value: Value) -> Option<Answer> {
    Some(Answer {
        value: Plain::of(&value)?,
        work: 0,
    })
}

/// `GCD`: `math_ops::gcd_scalar`, the law `op_gcd` lifts, charged what
/// `op_gcd` charges for it — `binary_numeric_work` of the operands' widths, as
/// `ADD` is; a non-integer operand is its NIL projection, which declines
/// here.
fn gcd(operands: &[Plain], _: &Interpreter) -> Option<Answer> {
    let [a, b] = operands else { return None };
    let work = match (a, b) {
        (Plain::Num(x), Plain::Num(y)) => {
            use crate::interpreter::runtime_limits::{binary_numeric_work, fraction_work_bits};
            binary_numeric_work(fraction_work_bits(x), fraction_work_bits(y))
        }
        _ => return None,
    };
    // Two machine-word integers: the gcd the law computes, without the
    // `BigInt`s it reads them into. `Fraction::new` would hold the answer as
    // this same machine-word pair, so the value is the law's to the
    // representation (the route suites compare it).
    if let (Plain::Num(x), Plain::Num(y)) = (a, b) {
        if let (Some((n, 1)), Some((m, 1))) = (x.extract_i64_pair(), y.extract_i64_pair()) {
            if let Ok(g) = i64::try_from(n.unsigned_abs().gcd(&m.unsigned_abs())) {
                return Some(Answer {
                    value: Plain::Num(crate::types::fraction::Fraction::from(g)),
                    work,
                });
            }
        }
    }
    let law =
        crate::interpreter::math_ops::gcd_scalar(&a.clone().into_value(), &b.clone().into_value());
    Some(Answer {
        work,
        ..uncharged(law.ok()?)?
    })
}

/// `NIL?`: a plain value is never NIL (`nil_diagnostics::op_nil_check`).
fn nil_check(operands: &[Plain], _: &Interpreter) -> Option<Answer> {
    let [a] = operands else { return None };
    uncharged(Value::from_bool(a.clone().into_value().is_nil()))
}

/// `DEPTH`: `shape_words::depth_of`, which is 0 for anything but a Vector.
fn depth(operands: &[Plain], _: &Interpreter) -> Option<Answer> {
    let [a] = operands else { return None };
    let depth = crate::interpreter::shape_words::depth_of(&a.clone().into_value());
    uncharged(Value::from_int(depth as i64))
}
