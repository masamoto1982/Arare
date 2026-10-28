//! A Word call specialised to the operands it meets: the scalar arithmetic,
//! comparison, rounding and logic Words, and SELECT, on plain machine-word
//! rationals and truth values.
//!
//! The engine's ordinary Word dispatch runs the whole machinery for every call: the
//! declared NIL contract, the declared lift, the Word's own NIL passthrough
//! and Record lift, the operand meter, the scalar fast path, the result-size
//! check and the NIL trace — about a thousand instructions to add two small
//! numbers, most of them there for operands that are absent, lifted or wide.
//! When both operands are plain scalars whose halves fit a machine word, none
//! of that machinery has anything to do, and what remains is the arithmetic
//! and its charges.
//!
//! This answers exactly what the dispatch would, and charges exactly what it
//! does, or declines and leaves the call to it:
//!
//! - one execution step (`charge_execution_step`, the same counter);
//! - for `ADD` `SUB` `MUL` `DIV`, one unit of numeric work — two operands of
//!   one limb each, `binary_numeric_work(1, 1)`;
//! - for `POW`, the same one unit — a power whose answer fits a machine word
//!   is priced as one product of machine words (`power_numeric_work`);
//! - for every other Word it answers, no work;
//! - the nesting check the dispatch makes after every Word
//!   (`check_fresh_nesting`), which also resets the stack's fresh mark.
//!
//! It declines whenever the dispatch could do anything else: an operand that
//! is not a plain machine-word rational, a result that leaves a machine word
//! or a zero divisor (both the dispatch's to build), a size ceiling below a
//! machine word, or a step or work ceiling the call would cross (the
//! dispatch's to report). The NIL contract, the lift and the trace have
//! nothing to do on a call that meets no absence and makes none. Which route
//! answered is unobservable (LANG.AUTHORITY.FREEDOM); `quickened_tests`
//! holds the two equal.

use crate::interpreter::Interpreter;
use crate::kernel::generated::WordId;
use crate::types::fraction::{Fraction, FractionRepr};
use crate::types::small_rational::{self, add, div, mul, order, Pair};
use crate::types::{Value, ValueData};
use std::cmp::Ordering;

/// A plain value held unboxed: a rational whose halves each fit a machine
/// word, or a truth value. `quickened` reads its operands as these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Num(Pair),
    Bool(bool),
}

impl Slot {
    /// The slot a stack value is, when it is a plain one.
    #[inline]
    pub(crate) fn of(value: &Value) -> Option<Self> {
        match &value.data {
            // A zero denominator is an absent number, not a number.
            ValueData::Scalar(Fraction {
                repr: FractionRepr::Small(n, d),
            }) if *d != 0 => Some(Slot::Num((*n, *d))),
            ValueData::Boolean(b) => Some(Slot::Bool(*b)),
            _ => None,
        }
    }

    /// The value the interpreted route builds for this slot.
    #[inline]
    pub(crate) fn into_value(self) -> Value {
        match self {
            Slot::Num((n, d)) => {
                Value::from_fraction(Fraction::from_repr(FractionRepr::Small(n, d)))
            }
            Slot::Bool(b) => Value::from_bool(b),
        }
    }

    #[inline]
    fn num(self) -> Option<Pair> {
        match self {
            Slot::Num(p) => Some(p),
            Slot::Bool(_) => None,
        }
    }

    #[inline]
    fn truth(self) -> Option<bool> {
        match self {
            Slot::Bool(b) => Some(b),
            Slot::Num(_) => None,
        }
    }
}

/// The Words answered on plain slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Gt,
    Eq,
    Min,
    Max,
    Floor,
    Round,
    Not,
    And,
    Select,
    /// `POW` on plain operands (`small_power`): a step and one unit of work.
    Pow,
}

impl Kind {
    pub(crate) fn of(word: WordId) -> Option<Self> {
        Some(match word {
            WordId::Add => Kind::Add,
            WordId::Sub => Kind::Sub,
            WordId::Mul => Kind::Mul,
            WordId::Div => Kind::Div,
            WordId::Lt => Kind::Lt,
            WordId::Gt => Kind::Gt,
            WordId::Eq => Kind::Eq,
            WordId::Min => Kind::Min,
            WordId::Max => Kind::Max,
            WordId::Floor => Kind::Floor,
            WordId::Round => Kind::Round,
            WordId::Not => Kind::Not,
            WordId::And => Kind::And,
            WordId::Select => Kind::Select,
            WordId::Pow => Kind::Pow,
            _ => return None,
        })
    }

    /// How many operands the Word consumes; each leaves one result.
    pub(crate) fn arity(self) -> usize {
        match self {
            Kind::Floor | Kind::Round | Kind::Not => 1,
            Kind::Select => 3,
            _ => 2,
        }
    }
}

/// What a Word answered on plain slots, with the numeric work the dispatch
/// charges for it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Answer {
    pub(crate) value: Slot,
    pub(crate) work: u64,
}

#[inline]
fn answered(value: Slot, work: u64) -> Option<Answer> {
    Some(Answer { value, work })
}

/// What `kind` answers on `operands` (deepest first, `kind.arity()` of
/// them), or `None` for any call the dispatch must make itself.
#[inline]
pub(crate) fn apply(kind: Kind, operands: &[Slot]) -> Option<Answer> {
    let x = |i: usize| operands[i];
    if matches!(
        kind,
        Kind::Add | Kind::Sub | Kind::Mul | Kind::Div | Kind::Lt | Kind::Gt | Kind::Eq
    ) {
        super::arithmetic::note_scalar_fastpath();
    }
    match kind {
        Kind::Add | Kind::Sub | Kind::Mul | Kind::Div => {
            let (a, b) = (x(0).num()?, x(1).num()?);
            // Two integers: their sum, difference or product is an integer,
            // and fits a machine word exactly when the checked operation
            // does — which is what `add` and `mul` answer for denominators
            // of 1, there after a call and a test of each denominator.
            if a.1 == 1 && b.1 == 1 && kind != Kind::Div {
                let n = match kind {
                    Kind::Add => a.0.checked_add(b.0),
                    Kind::Sub => a.0.checked_sub(b.0),
                    _ => a.0.checked_mul(b.0),
                }?;
                return answered(Slot::Num((n, 1)), 1);
            }
            let r = match kind {
                Kind::Add => add(a, b, false),
                Kind::Sub => add(a, b, true),
                Kind::Mul => mul(a, b),
                _ => div(a, b),
            }?;
            answered(Slot::Num(r), 1)
        }
        Kind::Lt | Kind::Gt => {
            let (a, b) = (x(0).num()?, x(1).num()?);
            let want = if kind == Kind::Lt {
                Ordering::Less
            } else {
                Ordering::Greater
            };
            answered(Slot::Bool(order(a, b) == want), 0)
        }
        // Two numbers compare on the fast path; two truth values by
        // `pairwise_eq`, off it.
        Kind::Eq => match (x(0), x(1)) {
            (Slot::Num(a), Slot::Num(b)) => answered(Slot::Bool(a == b), 0),
            (Slot::Bool(a), Slot::Bool(b)) => answered(Slot::Bool(a == b), 0),
            _ => None,
        },
        // The left operand on a tie, as MIN and MAX keep it.
        Kind::Min | Kind::Max => {
            let (a, b) = (x(0).num()?, x(1).num()?);
            let take_right = if kind == Kind::Min {
                order(b, a) == Ordering::Less
            } else {
                order(a, b) == Ordering::Less
            };
            answered(Slot::Num(if take_right { b } else { a }), 0)
        }
        Kind::Floor => {
            let (n, d) = x(0).num()?;
            answered(Slot::Num((n.div_euclid(d), 1)), 0)
        }
        Kind::Round => {
            let (n, d) = x(0).num()?;
            answered(
                Slot::Num((small_rational::round_half_away_from_zero(n, d), 1)),
                0,
            )
        }
        Kind::Not => answered(Slot::Bool(!x(0).truth()?), 0),
        // Both operands are read before either decides: a FALSE beside a
        // number is the dispatch's ERROR, not FALSE.
        Kind::And => {
            let (a, b) = (x(0).truth()?, x(1).truth()?);
            answered(Slot::Bool(a && b), 0)
        }
        // Either candidate may be a number or a truth value; anything else
        // would be lifted over, which is the dispatch's to do.
        Kind::Select => answered(if x(2).truth()? { x(0) } else { x(1) }, 0),
        Kind::Pow => {
            let (a, b) = (x(0).num()?, x(1).num()?);
            answered(Slot::Num(small_power(a, b)?), 1)
        }
    }
}

/// `base` raised to the non-negative integer `exponent`, by square-and-
/// multiply with each half checked, or `None` when a half leaves a machine
/// word or the operands are not the plain case: a base in lowest terms with a
/// positive denominator (so its powers stay in lowest terms) and an integer
/// exponent. `x⁰` is 1, `0ⁿ` is 0 and `0⁰` is 1, as `power_by_integer` has it.
/// Negative and fractional exponents, and every overflow, are the dispatch's.
#[inline]
pub(crate) fn small_power(base: Pair, exponent: Pair) -> Option<Pair> {
    let ((n, d), (e, one)) = (base, exponent);
    if one != 1 || e < 0 || d <= 0 {
        return None;
    }
    // The dispatch refuses an exponent whose answer could pass this many bits
    // (`INTEGER_POWER_RESULT_BITS`), even for a base of 1, whose answer is
    // small: the refusal is its to report.
    let width = |v: i64| u64::from(64 - v.unsigned_abs().leading_zeros());
    let bits = width(n).max(width(d)).max(2);
    if u128::from(e.unsigned_abs()) * u128::from(bits) > 1 << 20 {
        return None;
    }
    let mut e = u64::try_from(e).ok()?;
    let (mut num, mut den) = (1i64, 1i64);
    let (mut base_n, mut base_d) = (n, d);
    while e > 0 {
        if e & 1 == 1 {
            num = num.checked_mul(base_n)?;
            den = den.checked_mul(base_d)?;
        }
        e >>= 1;
        if e > 0 {
            base_n = base_n.checked_mul(base_n)?;
            base_d = base_d.checked_mul(base_d)?;
        }
    }
    Some((num, den))
}

/// `LENGTH` of a non-NIL Vector or Tensor: one step and nothing else — no
/// work, no fast-path hit, no NIL minted. It reads its operand without a copy
/// and declines, touching nothing, for anything else.
fn try_length_call(interp: &mut Interpreter) -> bool {
    let Some(target) = interp.stack.as_slice().last() else {
        return false;
    };
    if !target.is_vector() {
        return false;
    }
    let value = Value::from_fraction(Fraction::from(target.len() as i64));
    if interp.execution_step_count >= interp.max_execution_steps {
        return false;
    }
    interp.execution_step_count += 1;
    interp.stack.pop();
    interp.stack.push(value);
    interp
        .check_fresh_nesting()
        .expect("a plain result is within any nesting ceiling");
    #[cfg(test)]
    QUICKENED.with(|c| c.set(c.get() + 1));
    true
}

/// A Word with no `Kind` of its own that the contract admits to plain values
/// (`fusion_contract`): its plain law, charged as its dispatch charges it.
fn try_kernel_call(interp: &mut Interpreter, word: WordId) -> bool {
    let Some(kernel) = crate::interpreter::fusion_contract::kernel(word) else {
        return false;
    };
    let slots = interp.stack.as_slice();
    let Some(base) = slots.len().checked_sub(kernel.arity) else {
        return false;
    };
    let Some(operands) = slots[base..]
        .iter()
        .map(crate::interpreter::fused_block::Plain::of)
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some(answer) = (kernel.apply)(&operands, interp) else {
        return false;
    };
    if interp.execution_step_count >= interp.max_execution_steps
        || interp.numeric_work_used.saturating_add(answer.work)
            > interp.runtime_limits.max_numeric_work
    {
        return false;
    }
    interp.execution_step_count += 1;
    interp.numeric_work_used += answer.work;
    interp.stack.truncate(base);
    interp.stack.push(answer.value.into_value());
    interp
        .check_fresh_nesting()
        .expect("a plain result is within any nesting ceiling");
    #[cfg(test)]
    QUICKENED.with(|c| c.set(c.get() + 1));
    true
}

/// Run `word` on the plain values on top of the stack, or answer `false`
/// having touched nothing.
pub(crate) fn try_scalar_call(interp: &mut Interpreter, word: WordId) -> bool {
    if !interp.quickening_enabled || !interp.scalar_fastpath_enabled {
        return false;
    }
    // A Word with a `Kind` compiles to `Op::Scalar`, whose own route
    // (`Interpreter::scalar_in_place`) has already tried and declined it on
    // this very stack: what is left to try here is `LENGTH` and the kernels.
    if Kind::of(word).is_some() {
        return false;
    }
    if word == WordId::Length {
        return try_length_call(interp);
    }
    try_kernel_call(interp, word)
}

#[cfg(test)]
thread_local! {
    static QUICKENED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Calls this route answered on this thread, for tests that pin it ran.
#[cfg(test)]
pub(crate) fn note_quickened() {
    QUICKENED.with(|c| c.set(c.get() + 1));
}

#[cfg(test)]
pub(crate) fn quickened_calls_on_this_thread() -> u64 {
    QUICKENED.with(|c| c.get())
}
