//! Where arithmetic is charged and bounded — the work meter's boundary.
//!
//! This lives beside the arithmetic dispatch rather than inside it, because it
//! is a property *of* the dispatch and not of any route the dispatch chooses.
//! It used to be neither: two of the six routes out of
//! `arithmetic::apply_exact_arithmetic_schema` charged, both reachable only
//! when both operands were scalar-shaped, and the other four were free. So
//! `2 3 MUL` was priced and `[ 2 ] 3 MUL` was not, and `algebraicTerms` was a
//! ceiling a vector literal turned off. Which route runs is an optimization
//! decision, unobservable by LANG.AUTHORITY.FREEDOM; a safety control priced
//! per route made it observable, which is the one thing a limit must never do.
//!
//! The charge is taken at the entry, before a route is chosen and before either
//! operand is consumed, so a refusal leaves the stack as the program left it.
//! The size check is taken per route, on the result, before the operands go —
//! it bounds what an operation *leaves behind* so the operand feeding the next
//! one is still a sane size.

use std::cell::Cell;

use crate::error::{AjisaiError, Result};
use crate::interpreter::arithmetic::ExactArithmeticSchema;
use crate::interpreter::exact_work::reciprocal_numeric_work;
use crate::interpreter::runtime_limits::{
    broadcast_numeric_work, exact_work_bits, fraction_result_bits, fraction_work_bits, OperandWork,
    ALGEBRAIC_PAIR_UNITS, ALGEBRAIC_SUM_TERM_UNITS,
};
use crate::interpreter::Interpreter;
use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;
use crate::types::{DenseTensor, Value, ValueData};

/// The work meter's view of an operand, gathered without regard to how the
/// value stores its lanes.
pub(crate) fn measure_operand(value: &Value) -> OperandWork {
    match &value.data {
        ValueData::Truths(t) => measure_operand(&t.to_vector()),
        ValueData::Scalar(f) => OperandWork::leaf(fraction_work_bits(f)),
        ValueData::ExactScalar(er) => OperandWork {
            lanes: 1,
            bits: exact_work_bits(er),
            terms: er.algebraic_term_count() as u64,
            basis: er.algebraic_basis_len() as u64,
        },
        // A dense tensor's lanes are `i64` numerators and denominators by
        // construction, so each is a single limb and the width is known without
        // reading the data — which matters, because this runs on every
        // arithmetic Word and a tensor is the shape that carries a million of
        // them.
        ValueData::Tensor { data, .. } => OperandWork {
            lanes: data.len() as u64,
            bits: 1,
            terms: 0,
            basis: 0,
        },
        ValueData::Vector(children) => children
            .iter()
            .map(measure_operand)
            .reduce(OperandWork::join)
            .unwrap_or(OperandWork::leaf(1)),
        // A Record lifts arithmetic over its values (LANG.COLLECTIONS.LIFT),
        // so its work is theirs.
        ValueData::Record(record) => record
            .values()
            .iter()
            .map(measure_operand)
            .reduce(OperandWork::join)
            .unwrap_or(OperandWork::leaf(1)),
        // No arithmetic happens on these; the structure error they raise is
        // not work.
        ValueData::Boolean(_) | ValueData::Nil(_) | ValueData::Text(_) | ValueData::Symbol(_) => {
            OperandWork::leaf(1)
        }
    }
}

/// Charge the whole operation before any of it runs, and before either operand
/// is consumed — so a refusal leaves the stack exactly as the program left it
/// and the interpreter stays usable.
pub(crate) fn charge_binary_schema(
    interp: &mut Interpreter,
    schema: ExactArithmeticSchema,
    left: OperandWork,
    right: OperandWork,
) -> Result<()> {
    let algebraic = left.terms > 0 || right.terms > 0;
    let pair_units = if algebraic {
        // A term pair is not one bignum multiply — it is a coefficient product,
        // a radicand product, a square-free decomposition against a growing
        // basis and an ordered-map insert — so it carries the measured
        // constant on top of the bignum work each pair performs.
        let left_terms = left.terms.max(1);
        let right_terms = right.terms.max(1);
        match schema {
            // Division multiplies by the right operand's inverse, which is
            // charged below.
            ExactArithmeticSchema::Mul | ExactArithmeticSchema::Div => left_terms
                .saturating_mul(right_terms)
                .saturating_mul(ALGEBRAIC_PAIR_UNITS),
            // A sum merges two term maps rather than multiplying term pairs.
            ExactArithmeticSchema::Add | ExactArithmeticSchema::Sub => left_terms
                .saturating_add(right_terms)
                .saturating_mul(ALGEBRAIC_SUM_TERM_UNITS),
        }
    } else {
        1
    };
    // An irrational divisor is inverted by conjugation first, once per lane.
    let inverse = if matches!(schema, ExactArithmeticSchema::Div) && right.terms > 0 {
        reciprocal_numeric_work(right.bits, right.terms, right.basis)
            .saturating_mul(left.lanes.max(right.lanes).max(1))
    } else {
        0
    };

    interp.charge_numeric_work(
        broadcast_numeric_work(left, right, pair_units).saturating_add(inverse),
    )
}

/// The widest lane of a dense tensor. Its lanes are `i64` by construction, so
/// this reads machine words rather than bignums.
fn dense_tensor_result_bits(data: &DenseTensor) -> u64 {
    fn width(value: i64) -> u64 {
        u64::from(64 - value.unsigned_abs().leading_zeros())
    }
    data.numerators
        .iter()
        .zip(data.denominators.iter())
        .map(|(numerator, denominator)| width(*numerator).max(width(*denominator)))
        .max()
        .unwrap_or(0)
}

/// The widest lane and largest algebraic term count a result carries.
fn measure_result(value: &Value) -> (u64, usize) {
    match &value.data {
        ValueData::Truths(t) => measure_result(&t.to_vector()),
        ValueData::Scalar(f) => (fraction_result_bits(f), 0),
        ValueData::ExactScalar(er) => (er.max_coefficient_bits(), er.algebraic_term_count()),
        ValueData::Tensor { data, .. } => (dense_tensor_result_bits(data), 0),
        ValueData::Vector(children) => children.iter().fold((0, 0), |(bits, terms), child| {
            let (child_bits, child_terms) = measure_result(child);
            (bits.max(child_bits), terms.max(child_terms))
        }),
        ValueData::Record(record) => record.values().iter().fold((0, 0), |(bits, terms), child| {
            let (child_bits, child_terms) = measure_result(child);
            (bits.max(child_bits), terms.max(child_terms))
        }),
        ValueData::Boolean(_) | ValueData::Nil(_) | ValueData::Text(_) | ValueData::Symbol(_) => {
            (0, 0)
        }
    }
}

/// Reject a result whose widest lane crosses the accumulation ceilings, so the
/// operand feeding the next operation is still a sane size. The per-operation
/// cost is bounded separately by the pre-charge above; this bounds what the
/// operation leaves behind.
pub(crate) fn check_result_size(interp: &Interpreter, value: &Value) -> Result<()> {
    let (bits, terms) = measure_result(value);
    interp.note_algebraic_terms(terms);
    interp.runtime_limits.check_algebraic_size(terms, bits)
}

// The share of the run's `numericWork` a square root may spend factoring its
// radicand (`types::exact::squarefree`).
//
// A radicand is reduced to its square-free part so that one number has one
// normal form (LANG.VALUES.DENOTATION). That needs a factorization, whose cost
// is not bounded by anything the operand's size alone predicts, so it is
// metered on the same budget as the rest of the arithmetic: the root is taken
// against what the run has left, the work actually spent is charged
// afterwards, and a radicand the budget cannot factor is the same
// `resourceLimitExceeded` any other exhausted work meter raises.
//
// `POW` takes its products from the same budget: a lifted power takes a root
// and a power lane by lane, and each is priced against what the run has left
// before it is taken.
pub(crate) struct RadicandBudget {
    limit: u64,
    start: u64,
    remaining: Cell<u64>,
    exhausted: Cell<bool>,
}

impl RadicandBudget {
    pub(crate) fn of(interp: &Interpreter) -> Self {
        let start = interp
            .runtime_limits
            .max_numeric_work
            .saturating_sub(interp.numeric_work_used);
        RadicandBudget {
            limit: interp.runtime_limits.max_numeric_work,
            start,
            remaining: Cell::new(start),
            exhausted: Cell::new(false),
        }
    }

    /// The budget left for one root, taken back by [`Self::spent`].
    pub(crate) fn take(&self) -> u64 {
        self.remaining.get()
    }

    /// Record what one root left of the budget it was given, and whether it
    /// ran out.
    pub(crate) fn spent(&self, left: u64, exhausted: bool) {
        self.remaining.set(left);
        if exhausted {
            self.exhausted.set(true);
        }
    }

    /// Take `units` from the budget for a power's products, or mark it
    /// exhausted and take nothing when they do not fit.
    pub(crate) fn charge(&self, units: u64) -> bool {
        match self.remaining.get().checked_sub(units) {
            Some(left) => {
                self.remaining.set(left);
                true
            }
            None => {
                self.exhausted.set(true);
                false
            }
        }
    }

    /// √`radicand` within the budget, or the exhausted meter's error.
    pub(crate) fn sqrt(&self, radicand: Fraction) -> Result<Option<ExactReal>> {
        let mut left = self.take();
        let root = ExactReal::try_sqrt_rational(radicand, &mut left);
        self.spent(left, root.is_err());
        root.map_err(|_| self.exhausted_error())
    }

    /// The error the work meter raises when this root's factorization used
    /// up what the run had left.
    pub(crate) fn exhausted_error(&self) -> AjisaiError {
        AjisaiError::ResourceLimitExceeded {
            resource: crate::error::ResourceLimit::NumericWork,
            limit: self.limit,
            observed: Some(self.limit.saturating_add(1)),
            progress: None,
        }
    }

    /// Charge the work spent to the run's meter; an exhausted budget charges
    /// everything that was left and one unit more.
    pub(crate) fn settle(&self, interp: &mut Interpreter) -> Result<()> {
        if self.exhausted.get() {
            return interp.charge_numeric_work(self.start.saturating_add(1));
        }
        interp.charge_numeric_work(self.start - self.remaining.get())
    }
}

#[cfg(test)]
mod tests {
    //! Behavioral coverage for the ExactScalar path of `op_div`
    //! (LANG.VALUES.EXACT): a quotient with an irrational operand is an exact
    //! value, never an error.

    use crate::test_support::run_ok;
    use crate::types::fraction::Fraction;
    use crate::types::{Value, ValueData};

    /// `√2 2 /` is evaluated as an exact value.
    #[tokio::test]
    async fn sqrt2_div_rational_is_exact_not_error() {
        let stack = run_ok("2 SQRT 2 DIV").await;
        assert_eq!(stack.len(), 1);
        let top = &stack[0];
        assert!(
            !top.is_nil(),
            "√2 2 / must not collapse to NIL/error, got {top:?}"
        );
        assert!(
            matches!(top.data, ValueData::ExactScalar(_) | ValueData::Scalar(_)),
            "√2 2 / must stay an exact real, got {top:?}"
        );
    }

    /// `√2 √2 /` is evaluated exactly, not as an error.
    #[tokio::test]
    async fn sqrt2_div_sqrt2_is_exact() {
        let stack = run_ok("2 SQRT 2 SQRT DIV").await;
        assert_eq!(stack.len(), 1);
        let top = &stack[0];
        assert!(
            !top.is_nil(),
            "√2 √2 / must not collapse to NIL/error, got {top:?}"
        );
        assert!(
            matches!(top.data, ValueData::ExactScalar(_) | ValueData::Scalar(_)),
            "√2 √2 / must stay an exact real, got {top:?}"
        );
    }

    /// `√2 0 /` is `1/0`: an irrational dividend contributes its sign to a
    /// quotient by zero (LANG.VALUES.EXACT), and `0 √2 DIV` is 0.
    #[tokio::test]
    async fn sqrt2_div_zero_is_the_positive_point_over_zero() {
        let stack = run_ok("2 SQRT 0 DIV").await;
        assert_eq!(stack.len(), 1);
        assert_eq!(stack[0], Value::from_fraction(Fraction::infinity()));
        let stack = run_ok("2 SQRT -1 MUL 0 DIV").await;
        assert_eq!(stack[0], Value::from_fraction(Fraction::infinity()));
        let stack = run_ok("1 2 SQRT 0 DIV DIV").await;
        assert_eq!(stack[0], Value::from_int(0));
    }

    /// Ordinary rational division is unchanged (no regression): `6 3 DIV` -> `2`.
    #[tokio::test]
    async fn rational_division_unchanged() {
        let stack = run_ok("6 3 DIV").await;
        assert_eq!(stack.len(), 1);
        let frac = stack[0]
            .as_scalar()
            .cloned()
            .unwrap_or_else(|| panic!("6 3 / must be a rational, got {:?}", stack[0]));
        assert_eq!(frac.to_i64(), Some(2), "6 3 / must equal 2");
    }
}
