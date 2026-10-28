use super::{extract_vector_elements, with_stacktop_vector_target_no_arg};
use crate::error::{AjisaiError, Result};
use crate::interpreter::value_extraction_helpers::extract_bigint_from_value;
use crate::interpreter::Interpreter;
use crate::types::fraction::saturating_narrow;
use crate::types::fraction::Fraction;
use crate::types::Value;
use num_bigint::BigInt;
use num_traits::{One, ToPrimitive};

/// Join two vectors, lifting one level of nesting out of each.
///
/// Both operands are vectors by the time this runs — `op_concat` rejects
/// anything else — so there is no singleton-lifting branch here: an element is
/// carried across exactly as it sits, and `[ [ 1 ] ] [ [ 2 ] ] CONCAT` stays
/// `[ [ 1 ] [ 2 ] ]`.
///
/// Two dense Tensors whose rows have one shape join as columns
/// (`DenseTensor::concatenated`). The nested route boxed every lane of both
/// operands into a `Value` and answered an AoS `Vector` — 72 ms for two
/// million-lane ranges, and a result every Word downstream then had to read
/// the slow way. The joined Tensor equals the Vector it replaces
/// (`tensor_eq_vector`), and `CONCAT`'s charge is priced from the operands
/// before either route runs, so the choice is unobservable
/// (LANG.AUTHORITY.FREEDOM).
fn concat_values(left: &Value, right: &Value) -> Value {
    use crate::types::ValueData;
    if let (
        ValueData::Tensor {
            data: left_data,
            shape: left_shape,
        },
        ValueData::Tensor {
            data: right_data,
            shape: right_shape,
        },
    ) = (&left.data, &right.data)
    {
        if !left_shape.is_empty() && !right_shape.is_empty() && left_shape[1..] == right_shape[1..]
        {
            let mut shape = left_shape.to_vec();
            shape[0] += right_shape[0];
            return Value::from_dense_tensor(
                left_data.concatenated(right_data, shape.clone()),
                shape,
            );
        }
    }
    let left = left.as_vector_view().unwrap_or_default();
    let right = right.as_vector_view().unwrap_or_default();
    let mut elements = Vec::with_capacity(left.len() + right.len());
    elements.extend_from_slice(&left);
    elements.extend_from_slice(&right);
    Value::from_vector(elements)
}

/// One bound of `RANGE`: any integer. Anything else — a fraction, a String, a
/// Boolean — names no position in an integer sequence, so it is the malformed
/// use `invalidInteger` declares. A Vector never reaches here: `leaf` operands
/// are lifted by the dispatcher first.
///
/// An integer outside `i64` is still an integer: `2^63 2^63+2 RANGE` is three
/// numbers, and refusing it as `invalidInteger` for its size made the answer
/// jump from a sequence to an ERROR at a storage boundary the language does not
/// have. How many lanes the bounds span is what the ceiling bounds.
fn parse_range_bound(bound: &Value, label: &str) -> Result<BigInt> {
    extract_bigint_from_value(bound).map_err(|_| {
        AjisaiError::declared(
            "invalidInteger",
            format!(
                "the {} must be an integer, got {}",
                label,
                bound.domain_name()
            ),
        )
    })
}

/// `CONCAT` — join the top two vectors (SPEC: `2 -> 1`, `errorWhen:
/// [nonVector]`).
///
/// The arity is exactly the declared one. `CONCAT` used to accept an undeclared
/// count-prefixed form (`a b c 3 CONCAT`, negative for reversed order) that it
/// recognized by *sniffing* the stack top for a bare integer, and to treat a
/// non-vector operand as a singleton instead of refusing it. Both are gone: a
/// calling convention the specification does not declare is a second contract
/// written nowhere, and the sniff decided `CONCAT`'s arity from the *value* of
/// an operand — which is what made `[ 1 2 ] [ 3 ] CONCAT` an underflow, and
/// what left the declared `nonVector` error unreachable (`2 3 CONCAT` reported
/// a short stack rather than a non-vector operand). It also disagreed with the
/// dispatch NIL guard, which clamps its window to the declared arity of 2 and
/// so could not see the operands a longer count would reach.
pub fn op_concat(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 2 {
        return Err(AjisaiError::stack_underflow());
    }

    let base = interp.stack.len() - 2;
    let operands: Vec<Value> = interp.stack.split_off(base).into_values();

    if let Some(got) = operands
        .iter()
        .find(|operand| !operand.is_vector())
        .map(|operand| operand.domain_name())
    {
        // The operands were already taken off; put them back so the
        // stack a reader inspects after the error is the one they wrote.
        interp.stack.extend(operands);
        return Err(AjisaiError::declared(
            "nonVector",
            format!("expected two Vectors, got {got}"),
        ));
    }

    // Both halves are copied into the join, so both are priced. Charged before
    // the copy runs, with the operands put back first so a refusal leaves the
    // stack the way the program wrote it.
    let units = crate::interpreter::collection_meter::element_cost(&operands[0])
        .copies(operands[0].len())
        .saturating_add(
            crate::interpreter::collection_meter::element_cost(&operands[1])
                .copies(operands[1].len()),
        );
    if let Err(e) = crate::interpreter::collection_meter::charge(interp, units) {
        interp.stack.extend(operands);
        return Err(e);
    }

    interp.stack.push(concat_values(&operands[0], &operands[1]));
    Ok(())
}

pub fn op_reverse(interp: &mut Interpreter) -> Result<()> {
    crate::interpreter::collection_meter::charge_stacktop_copy(interp, |len| len)?;

    let reversed = with_stacktop_vector_target_no_arg(interp, |vector_val| {
        // A flat dense buffer reverses as columns. The nested route below
        // unpacked the tensor into one boxed `Value` per lane, reversed *those*,
        // and handed back an AoS `Vector` — so reversing 262,144 numbers cost
        // 68.5 ms and, worse, threw the dense representation away, leaving every
        // Word downstream to decline its own dense path. Rank is the caller's
        // check because reversing a rank-2 tensor reverses rows, and a row is a
        // stride rather than a lane; those keep the nested route.
        //
        // The charge is unaffected: `charge_stacktop_copy` above prices this by
        // length alone, so both routes cost the same, which is what keeps the
        // choice of route unobservable (LANG.AUTHORITY.FREEDOM).
        if let crate::types::ValueData::Tensor { data, shape } = &vector_val.data {
            if shape.len() == 1 {
                return Ok(Value::from_dense_tensor(
                    data.reversed_lanes(),
                    (**shape).clone(),
                ));
            }
        }
        let mut v = extract_vector_elements(vector_val);
        v.reverse();
        Ok(Value::from_vector(v))
    })?;
    interp.stack.push(reversed);
    Ok(())
}

/// `start end RANGE` — every integer from `start` to `end`, both included,
/// counting down when `end` is below `start`: `0 3 RANGE` is `[ 0 1 2 3 ]`,
/// `3 0 RANGE` is `[ 3 2 1 0 ]`. There is no step operand: a stride is a
/// multiplication of this sequence (`0 3 RANGE 3 MUL`), so the bounds alone
/// decide the direction and no pair of bounds describes an infinite sequence.
pub fn op_range(interp: &mut Interpreter) -> Result<()> {
    if interp.stack.len() < 2 {
        return Err(AjisaiError::stack_underflow());
    }
    let end_val = interp.stack.pop().expect("length checked");
    let start_val = interp.stack.pop().expect("length checked");
    let bounds = parse_range_bound(&start_val, "start")
        .and_then(|start| parse_range_bound(&end_val, "end").map(|end| (start, end)));
    let (start, end) = match bounds {
        Ok(bounds) => bounds,
        Err(error) => {
            interp.stack.push(start_val);
            interp.stack.push(end_val);
            return Err(error);
        }
    };
    // Count the elements in BigInt so the span arithmetic cannot overflow for
    // any pair of bounds; a count past `u128` is past every ceiling anyway.
    //
    // Guard against unbounded materialization before allocating. RANGE loops
    // internally, so it counts as one execution step and bypasses the
    // step-count backstop; an input like `0 9999999999999 RANGE` would
    // otherwise drive the process into an OOM abort (a WASM trap in the
    // playground) instead of a recoverable error.
    let element_count = ((&end - &start).magnitude() + 1u32).to_u128();
    // CS5: the cap is the injectable per-interpreter ceiling (folded into
    // `RuntimeLimits`), so tests can fire this guard with a tiny limit and
    // child runtimes inherit it — same behavior and message as before.
    let max_materialized = interp.runtime_limits.max_materialized_elements;
    let element_count = match element_count {
        Some(count) if count <= max_materialized as u128 => count,
        _ => {
            // A well-formed, finite range longer than the host materializes is
            // refused by name (`resourceLimitExceeded`, `materializedElements`;
            // LANG.MACHINE.LIMITS), with the bounds put back as every refusal
            // leaves its operands. It is never a value: a ceiling is the
            // host's, and a value must mean the same thing on every host.
            interp.stack.push(start_val);
            interp.stack.push(end_val);
            return Err(
                crate::interpreter::ceiling_refusal::materialization_refused(
                    max_materialized,
                    element_count,
                ),
            );
        }
    };

    // Materializing an element costs what copying one costs: the elements are
    // freshly built rather than cloned, but they are the same boxed values, and
    // a program can ask for them as often as it likes. Charged before the
    // allocation, with the bounds put back on a refusal.
    if let Err(e) =
        crate::interpreter::collection_meter::charge_materialization(interp, element_count as usize)
    {
        interp.stack.push(start_val);
        interp.stack.push(end_val);
        return Err(e);
    }

    let count = element_count as usize;
    let ascending = start <= end;

    // Bounds past `i64` build their lanes as exact integers, one boxed value
    // each: the columns below hold only `i64`.
    let (Some(start), Some(_)) = (start.to_i64(), end.to_i64()) else {
        let step = if ascending {
            BigInt::one()
        } else {
            -BigInt::one()
        };
        let mut lanes = Vec::with_capacity(count);
        let mut current = start;
        for _ in 0..count {
            lanes.push(Value::from_fraction(Fraction::new(
                current.clone(),
                BigInt::one(),
            )));
            current += &step;
        }
        interp.stack.push(Value::from_vector(lanes));
        return Ok(());
    };

    // Built as columns, not as boxed lanes. Within `i64` bounds *every* value
    // RANGE can produce is an `i64` with denominator 1 and no lane absent — a
    // 1-D pure-integer dense tensor is not a guess about this result, it is
    // what the result is. Building `Vec<Value>` instead boxed each lane into a
    // 96-byte `Value` wrapping a 64-byte `Fraction` to carry 8 bytes of
    // integer, and then every Word downstream had to decline its dense fast
    // path because the dense representation had been thrown away at
    // construction: `0 262143 RANGE` spent 9.9 ms laying out 25 MB to describe
    // 2 MB of numbers.
    //
    // `element_count` is exact (`|end - start| + 1`), so lane `i` is
    // `start ± i`, which never passes `end` and so never overflows: a plain
    // loop over the indices, which the compiler vectorises.
    let mut numerators = vec![0i64; count];
    if ascending {
        for (i, lane) in numerators.iter_mut().enumerate() {
            *lane = start + i as i64;
        }
    } else {
        for (i, lane) in numerators.iter_mut().enumerate() {
            *lane = start - i as i64;
        }
    }

    interp.stack.push(Value::from_int_tensor(numerators));

    Ok(())
}

pub fn op_collect(interp: &mut Interpreter) -> Result<()> {
    let count_val = interp.stack.pop().ok_or(AjisaiError::stack_underflow())?;

    let count_bigint = match extract_bigint_from_value(&count_val) {
        Ok(bi) => bi,
        Err(_) => {
            let got = crate::types::display::describe_operand(&count_val);
            interp.stack.push(count_val);
            return Err(AjisaiError::declared(
                "invalidInteger",
                format!("expected an integer count, got {got}"),
            ));
        }
    };

    // Read as a `u64` on every target. A count a 32-bit `usize` cannot hold is
    // still a count — more values than any stack holds — so it saturates into
    // the underflow below instead of being declined as `invalidInteger` on
    // wasm32 alone.
    let count: usize = match count_bigint.to_u64() {
        Some(c) => saturating_narrow(c),
        None => {
            let got = crate::types::display::describe_operand(&count_val);
            interp.stack.push(count_val);
            return Err(AjisaiError::declared(
                "invalidInteger",
                format!("expected a non-negative integer count, got {got}"),
            ));
        }
    };

    if interp.stack.len() < count {
        interp.stack.push(count_val);
        return Err(AjisaiError::stack_underflow());
    }

    if let Err(e) = crate::interpreter::collection_meter::charge_materialization(interp, count) {
        interp.stack.push(count_val);
        return Err(e);
    }

    let collected: Vec<Value> = interp
        .stack
        .split_off(interp.stack.len() - count)
        .into_values();

    interp.stack.push(Value::from_vector(collected));
    Ok(())
}
