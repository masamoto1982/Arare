use crate::error::{AjisaiError, Result};
use crate::interpreter::arithmetic_meter::{
    charge_binary_schema, check_result_size, measure_operand,
};
use crate::interpreter::record_ops;
use crate::interpreter::tensor_lane_ops::lane_nil_passthrough;
use crate::interpreter::tensor_ops::apply_binary_broadcast;
use crate::interpreter::value_extraction_helpers::{
    exact_real_of, extract_operands, nil_passthrough_binary,
};
use crate::interpreter::Interpreter;
use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;
use crate::types::{DenseTensor, Value, ValueData};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExactArithmeticSchema {
    Add,
    Sub,
    Mul,
    Div,
}

impl ExactArithmeticSchema {
    /// The scalar law over rationals, total: every one of the four operations
    /// answers a number, division by zero included (`fraction_extended`).
    pub(crate) fn fraction(self, a: &Fraction, b: &Fraction) -> Fraction {
        match self {
            ExactArithmeticSchema::Add => a.add(b),
            ExactArithmeticSchema::Sub => a.sub(b),
            ExactArithmeticSchema::Mul => a.mul(b),
            ExactArithmeticSchema::Div => a.div(b),
        }
    }

    fn exact_real(self, a: &ExactReal, b: &ExactReal) -> ExactReal {
        match self {
            ExactArithmeticSchema::Add => a.add(b),
            ExactArithmeticSchema::Sub => a.sub(b),
            ExactArithmeticSchema::Mul => a.mul(b),
            ExactArithmeticSchema::Div => a.div(b),
        }
    }
}

fn consume_stacktop_binary(interp: &mut Interpreter) {
    interp.stack.pop();
    interp.stack.pop();
}

/// `dense_kernels::arithmetic` on the two operands, when it answers. Every
/// lane it produces fits a machine word, so the size ceiling can refuse one
/// only when it is set below that.
fn push_dense_kernel_result(
    interp: &mut Interpreter,
    schema: ExactArithmeticSchema,
) -> Result<bool> {
    if !interp.dense_kernels_enabled || interp.stack.len() < 2 {
        return Ok(false);
    }
    let stack_len = interp.stack.len();
    let slots = interp.stack.as_slice();
    let Some(result) = crate::interpreter::dense_kernels::arithmetic(
        schema,
        &slots[stack_len - 2],
        &slots[stack_len - 1],
    ) else {
        return Ok(false);
    };
    if interp.runtime_limits.max_bigint_bits < 64 {
        check_result_size(interp, &result)?;
    }
    consume_stacktop_binary(interp);
    interp.stack.push(result);
    Ok(true)
}

#[cfg(test)]
thread_local! {
    static SCALAR_FASTPATH_HITS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Count a scalar fast-path answer, in a test build only, for the tests that
/// pin which route ran.
#[inline(always)]
pub(crate) fn note_scalar_fastpath() {
    #[cfg(test)]
    SCALAR_FASTPATH_HITS.with(|c| c.set(c.get() + 1));
}

/// Scalar fast-path answers on this thread.
#[cfg(test)]
pub(crate) fn scalar_fastpath_hits_on_this_thread() -> u64 {
    SCALAR_FASTPATH_HITS.with(|c| c.get())
}

fn push_exact_real_schema_result(
    interp: &mut Interpreter,
    schema: ExactArithmeticSchema,
    a: &Value,
    b: &Value,
) -> Result<bool> {
    let has_exact = matches!(&a.data, ValueData::ExactScalar(_))
        || matches!(&b.data, ValueData::ExactScalar(_));
    if !has_exact {
        return Ok(false);
    }
    let Some(a_exact) = exact_real_of(a) else {
        return Ok(false);
    };
    let Some(b_exact) = exact_real_of(b) else {
        return Ok(false);
    };

    // The work of this operation was charged at the dispatch entry, before any
    // route was chosen — see `charge_binary_schema`.
    let result = Value::from_exact_real(schema.exact_real(&a_exact, &b_exact));
    // Bound accumulation: reject a result whose term count or coefficient
    // bit-length crosses the ceiling, before it is consumed and pushed (so a
    // limit failure leaves the operands on the stack, not a corrupted partial
    // state).
    check_result_size(interp, &result)?;

    consume_stacktop_binary(interp);
    interp.stack.push(result);
    Ok(true)
}

pub(crate) fn stacktop_pair(interp: &Interpreter) -> Option<(Value, Value)> {
    if interp.stack.len() < 2 {
        return None;
    }
    let stack_len = interp.stack.len();
    Some((
        interp.stack[stack_len - 2].clone(),
        interp.stack[stack_len - 1].clone(),
    ))
}

pub(crate) enum ScalarFastWrap {
    Scalar,
    Tensor(Vec<usize>),
}

struct ScalarFastOperand {
    fraction: Fraction,
    wrap: ScalarFastWrap,
}

fn scalar_fast_operand(value: &Value) -> Option<ScalarFastOperand> {
    match &value.data {
        ValueData::Scalar(f) => Some(ScalarFastOperand {
            fraction: f.clone(),
            wrap: ScalarFastWrap::Scalar,
        }),
        ValueData::Tensor { data, shape } if data.len() == 1 => Some(ScalarFastOperand {
            fraction: data.fraction_at(0),
            wrap: ScalarFastWrap::Tensor((**shape).clone()),
        }),
        ValueData::Vector(children) if children.len() == 1 => {
            let child = scalar_fast_operand(&children[0])?;
            let mut shape = Vec::with_capacity(2);
            shape.push(1);
            match child.wrap {
                ScalarFastWrap::Scalar => {}
                ScalarFastWrap::Tensor(child_shape) => shape.extend(child_shape),
            }
            Some(ScalarFastOperand {
                fraction: child.fraction,
                wrap: ScalarFastWrap::Tensor(shape),
            })
        }
        _ => None,
    }
}

fn same_scalar_fast_wrap(a: &ScalarFastWrap, b: &ScalarFastWrap) -> bool {
    match (a, b) {
        (ScalarFastWrap::Scalar, ScalarFastWrap::Scalar) => true,
        (ScalarFastWrap::Tensor(a_shape), ScalarFastWrap::Tensor(b_shape)) => a_shape == b_shape,
        _ => false,
    }
}

fn build_scalar_fast_result(result: Fraction, wrap: &ScalarFastWrap) -> Value {
    match wrap {
        ScalarFastWrap::Scalar => Value::from_fraction(result),
        ScalarFastWrap::Tensor(shape) => {
            if let Some(data) = DenseTensor::from_fractions(vec![result.clone()], shape.clone()) {
                Value::new(
                    ValueData::Tensor {
                        data: Arc::new(data),
                        shape: Arc::new(shape.clone()),
                    },
                    None,
                )
            } else {
                Value::from_tensor(vec![result], shape.clone())
            }
        }
    }
}

fn push_scalar_fastpath_result(
    interp: &mut Interpreter,
    schema: ExactArithmeticSchema,
) -> Result<bool> {
    if !interp.scalar_fastpath_enabled || interp.stack.len() < 2 {
        return Ok(false);
    }

    let stack_len = interp.stack.len();
    let Some(a) = scalar_fast_operand(&interp.stack[stack_len - 2]) else {
        return Ok(false);
    };
    let Some(b) = scalar_fast_operand(&interp.stack[stack_len - 1]) else {
        return Ok(false);
    };
    if !same_scalar_fast_wrap(&a.wrap, &b.wrap) {
        return Ok(false);
    }

    // The work of this operation was charged at the dispatch entry, before any
    // route was chosen — see `charge_binary_schema`.
    let result = build_scalar_fast_result(schema.fraction(&a.fraction, &b.fraction), &a.wrap);
    // Bound accumulation, so the operand feeding the next multiply is still a
    // sane size. Without this the chain above grows without any ceiling naming
    // itself.
    check_result_size(interp, &result)?;
    interp.stack.pop();
    interp.stack.pop();
    interp.stack.push(result);
    note_scalar_fastpath();
    Ok(true)
}

fn apply_exact_arithmetic_schema(
    interp: &mut Interpreter,
    schema: ExactArithmeticSchema,
) -> Result<()> {
    if nil_passthrough_binary(interp) {
        return Ok(());
    }
    // A Record operand lifts the Word over its values (LANG.COLLECTIONS.LIFT).
    if record_ops::lift_binary(interp, &|interp| {
        apply_exact_arithmetic_schema(interp, schema)
    })? {
        return Ok(());
    }

    // Charged once, here, before a route is chosen. Which route runs is an
    // optimization decision and unobservable by LANG.AUTHORITY.FREEDOM; a
    // safety control priced per route made it observable, as the difference
    // between `2 3 MUL` and `[ 2 ] 3 MUL`.
    if interp.stack.len() >= 2 {
        let stack_len = interp.stack.len();
        let (left, right) = {
            let slots = interp.stack.as_slice();
            (
                measure_operand(&slots[stack_len - 2]),
                measure_operand(&slots[stack_len - 1]),
            )
        };
        charge_binary_schema(interp, schema, left, right)?;
    }

    if push_scalar_fastpath_result(interp, schema)? {
        return Ok(());
    }

    if push_dense_kernel_result(interp, schema)? {
        return Ok(());
    }

    if let Some((a, b)) = stacktop_pair(interp) {
        if push_exact_real_schema_result(interp, schema, &a, &b)? {
            return Ok(());
        }
        // Vectors/structures carrying irrational `ExactScalar` lanes cannot use
        // the rational broadcast (it hard-errors on `FlatTensor::from_value`);
        // route them through the exact-real recursive broadcast instead.
        if push_exact_real_broadcast_result(interp, schema, &a, &b)? {
            return Ok(());
        }
    }

    apply_binary_arithmetic(interp, |a, b| Ok(schema.fraction(a, b)))
}

/// `true` when `val` is, or structurally contains, an irrational `ExactScalar`
/// leaf. The all-rational `Fraction`/`FlatTensor` broadcast path cannot carry
/// irrational algebraic lanes (`FlatTensor::from_value` rejects
/// `ExactScalar`), so its presence selects the exact-real recursive route
/// below instead. Bare scalar `ExactScalar` operands are already handled by
/// `push_exact_real_schema_result` upstream; this predicate exists to catch the
/// *vector/structural* cases that would otherwise hard-error in broadcast.
pub(crate) fn value_contains_exact_scalar(val: &Value) -> bool {
    match &val.data {
        ValueData::ExactScalar(_) => true,
        ValueData::Vector(items) => items.iter().any(value_contains_exact_scalar),
        // Dense tensors only hold rational `Fraction` lanes; they can never
        // contain an `ExactScalar`.
        _ => false,
    }
}

/// One lane of the exact-real lift: the passthrough law, then `schema`. The
/// shape rules are the rational lift's — one `broadcast_tree` walk with this
/// at the leaves — and `Value::from_exact_real` renormalizes a lane that
/// lands back on a rational to a plain `Scalar`.
fn exact_real_lane(a: &Value, b: &Value, schema: ExactArithmeticSchema) -> Result<Value> {
    // Absence before arithmetic, for the reason the rational lift gives
    // (`tensor_lane_ops::lane_nil_passthrough`).
    if let Some(nil) = lane_nil_passthrough(a, b) {
        return Ok(nil);
    }
    let (Some(ea), Some(eb)) = (exact_broadcast_leaf(a), exact_broadcast_leaf(b)) else {
        // Reached only through ADD/SUB/MUL/DIV's own binary dispatch, which
        // all declare `nonNumeric` uniformly.
        return Err(AjisaiError::declared(
            "nonNumeric",
            format!(
                "expected a Scalar or a Vector, got {}",
                if exact_broadcast_leaf(a).is_none() {
                    a.domain_name()
                } else {
                    b.domain_name()
                }
            ),
        ));
    };
    Ok(Value::from_exact_real(schema.exact_real(&ea, &eb)))
}

/// Exact-real value of a broadcast leaf (`Scalar` or `ExactScalar`). `None`
/// for non-numeric leaves, a NIL included, matching
/// `tensor_ops::broadcast_leaf`: the lane-wise lift settles absence before
/// this is asked.
fn exact_broadcast_leaf(value: &Value) -> Option<ExactReal> {
    match &value.data {
        ValueData::Scalar(f) => Some(ExactReal::from_fraction(f.clone())),
        ValueData::ExactScalar(er) => Some((**er).clone()),
        _ => None,
    }
}

/// Structural broadcast for operands containing irrational `ExactScalar`
/// lanes. Returns `Ok(false)` (leaving the stack untouched) for the cases the
/// caller still routes elsewhere — a top-level NIL — so the existing
/// NIL-passthrough and reduction paths keep their behavior.
pub(crate) fn push_exact_real_broadcast_result(
    interp: &mut Interpreter,
    schema: ExactArithmeticSchema,
    a: &Value,
    b: &Value,
) -> Result<bool> {
    if a.is_nil() || b.is_nil() {
        return Ok(false);
    }
    if !value_contains_exact_scalar(a) && !value_contains_exact_scalar(b) {
        return Ok(false);
    }
    let result = crate::interpreter::broadcast_tree::broadcast_tree(a, b, &|x, y| {
        exact_real_lane(x, y, schema)
    })?;
    // Irrational lanes are where the term count explodes, and this route used
    // to reach no size ceiling at all: `algebraicTerms` was a limit a vector
    // literal turned off.
    check_result_size(interp, &result)?;
    consume_stacktop_binary(interp);
    interp.stack.push(result);
    Ok(true)
}

fn apply_binary_arithmetic<F>(interp: &mut Interpreter, op: F) -> Result<()>
where
    F: Fn(&Fraction, &Fraction) -> Result<Fraction> + Copy + Sync,
{
    let operands = extract_operands(interp, 2)?;
    let a_val = &operands[0];
    let b_val = &operands[1];

    let computed = apply_binary_broadcast(a_val, b_val, op);
    let computed = computed.and_then(|result| {
        check_result_size(interp, &result)?;
        Ok(result)
    });
    let result = match computed {
        Ok(r) => r,
        Err(e) => {
            for val in operands {
                interp.stack.push(val);
            }
            return Err(e);
        }
    };

    interp.stack.push(result);

    Ok(())
}

pub fn op_add(interp: &mut Interpreter) -> Result<()> {
    apply_exact_arithmetic_schema(interp, ExactArithmeticSchema::Add)
}

pub fn op_sub(interp: &mut Interpreter) -> Result<()> {
    apply_exact_arithmetic_schema(interp, ExactArithmeticSchema::Sub)
}

pub fn op_mul(interp: &mut Interpreter) -> Result<()> {
    apply_exact_arithmetic_schema(interp, ExactArithmeticSchema::Mul)
}

pub fn op_div(interp: &mut Interpreter) -> Result<()> {
    apply_exact_arithmetic_schema(interp, ExactArithmeticSchema::Div)
}
