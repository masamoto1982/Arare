//! What the exact field's dearer operations cost on the work meter, beyond
//! the products and sums `arithmetic_meter` prices: a power, the inverse an
//! irrational divisor needs, and a comparison that reaches the algebraic
//! field. Each is priced from its operands before it runs, so a refusal comes
//! before the work rather than after it.

use num_bigint::BigInt;
use num_traits::Signed;

use crate::error::Result;
use crate::interpreter::arithmetic::ExactArithmeticSchema;
use crate::interpreter::arithmetic_meter::{charge_binary_schema, measure_operand};
use crate::interpreter::runtime_limits::{binary_numeric_work, ALGEBRAIC_PAIR_UNITS};
use crate::interpreter::Interpreter;
use crate::types::exact::ExactReal;
use crate::types::Value;

/// The work `xⁿ` costs by square-and-multiply, for an `x` of `base_terms`
/// algebraic terms (0 for a rational) and an answer `result_bits` wide with
/// at most `result_terms` terms. `exponent` is non-negative.
///
/// A rational power is priced as one product at the answer's width. That
/// bounds the whole chain: the squarings before the last are each a quarter
/// the price of the one after, and each multiplication into the accumulator
/// costs no more than the squaring that built its factor, so the chain comes
/// to about two thirds of it. A power whose answer fits a machine word is one
/// unit, as `MUL` of two is, which is what the quickened and fused routes
/// charge for it.
///
/// An algebraic power is priced as the term pairs its chain multiplies — the
/// products `integer_power` performs, each term count capped at what the
/// answer can hold — every pair at the answer's width.
pub fn power_numeric_work(
    result_bits: u64,
    base_terms: u64,
    exponent: &BigInt,
    result_terms: u64,
) -> u64 {
    let product = binary_numeric_work(result_bits, result_bits);
    if base_terms == 0 {
        return product;
    }
    let (mut pairs, mut square, mut accumulator) = (0u64, base_terms, 1u64);
    let length = exponent.bits();
    for bit in 0..length {
        if exponent.bit(bit) {
            pairs = pairs.saturating_add(accumulator.saturating_mul(square));
            accumulator = accumulator.saturating_mul(square).min(result_terms);
        }
        if bit + 1 < length {
            pairs = pairs.saturating_add(square.saturating_mul(square));
            square = square.saturating_mul(square).min(result_terms);
        }
    }
    product
        .saturating_mul(pairs)
        .saturating_mul(ALGEBRAIC_PAIR_UNITS)
}

/// The work inverting an algebraic of `terms` terms over a basis of `basis`
/// radicals costs, `bits` wide (`Algebraic::reciprocal`).
///
/// The inverse is taken by conjugation, one basis element per level: `y`
/// times its conjugate `u − v` is `u² − v²`, which has one radical fewer and
/// so at most half as many terms as the field it came from can hold; its
/// inverse comes back up and is multiplied by the conjugate. Each level squares
/// a term map and doubles its coefficients' width. Over `k` radicals that is
/// about `4ᵏ` term pairs, where a divisor's term count squared — what division
/// was priced at — grows by a few percent per radical.
pub fn reciprocal_numeric_work(bits: u64, terms: u64, basis: u64) -> u64 {
    let (mut units, mut terms, mut bits) = (0u64, terms, bits);
    for level in 0..basis {
        if terms <= 1 {
            break;
        }
        let below = u32::try_from(basis - level - 1)
            .ok()
            .and_then(|radicals| 1u64.checked_shl(radicals))
            .unwrap_or(u64::MAX);
        let squared = terms.saturating_mul(terms);
        // `y·(u − v)`, then `(u − v)` times the inverse of that.
        let pairs = squared.saturating_add(terms.saturating_mul(below));
        units = units.saturating_add(binary_numeric_work(bits, bits).saturating_mul(pairs));
        terms = squared.min(below);
        bits = bits.saturating_mul(2);
    }
    units.saturating_mul(ALGEBRAIC_PAIR_UNITS)
}

/// What `xⁿ` costs on the work meter: the chain of products that takes it,
/// and for an algebraic `x` under a negative `n`, the inverse of what that
/// chain leaves.
pub(crate) fn power_work(x: &ExactReal, n: &BigInt) -> u64 {
    let magnitude = n.abs();
    let positive = x.power_size(&magnitude);
    let base_terms = match x {
        ExactReal::Rational(_) => 0,
        ExactReal::Algebraic(_) => x.algebraic_term_count() as u64,
    };
    let chain = power_numeric_work(positive.bits, base_terms, &magnitude, positive.terms);
    if base_terms == 0 || !n.is_negative() {
        return chain;
    }
    chain.saturating_add(reciprocal_numeric_work(
        positive.bits,
        positive.terms,
        x.algebraic_basis_len() as u64,
    ))
}

/// Charge an exact comparison — `LT` `GT` `EQ` `MIN` `MAX` — before it runs.
///
/// A rational pair is a limb compare and costs nothing here, as it costs
/// nothing on the comparison fast path. A pair reaching the algebraic field
/// is decided by a subtraction — a term merge over a common refinement of
/// the two bases — and then the sign of what it leaves, so it is charged as
/// `SUB` is on the same operands.
pub(crate) fn charge_comparison(
    interp: &mut Interpreter,
    left: &Value,
    right: &Value,
) -> Result<()> {
    let (left, right) = (measure_operand(left), measure_operand(right));
    if left.terms == 0 && right.terms == 0 {
        return Ok(());
    }
    charge_binary_schema(interp, ExactArithmeticSchema::Sub, left, right)
}
