//! Pure Value -> protocol mapping.
//!
//! This is the single source of truth for the machine-facing value wire
//! format. It is shared by two serializers that must stay byte-compatible:
//! the agent report (`agent::report::protocol_node_json`, which renders a
//! `ProtocolNode` into JSON for agents) and the GUI's stack view
//! (`agent::stack_view`), which renders through it. It
//! carries no platform glue, so the entire decision surface is unit / MC/DC
//! / property tested natively (AQ-REQ-003, see `value_protocol_tests.rs`).

use crate::types::fraction::Fraction;
use crate::types::{DenseTensor, Value, ValueData};
use num_bigint::BigInt;

/// Pure, side-effect-free description of the protocol object consumers
/// receive for a stack value: its `type` and `value`, plus the value to
/// derive the `semantics` block from. Every field is a function of the value
/// alone (LANG.VALUES.DENOTATION).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProtocolNode {
    pub(crate) type_str: &'static str,
    pub(crate) value: ProtocolValue,
    /// Source value for the `semantics` block, which every node carries:
    /// whether a nested Vector is stored densely is not part of the value
    /// (LANG.AUTHORITY.FREEDOM), so it cannot decide what the node says.
    pub(crate) semantics: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProtocolValue {
    Null,
    Bool(bool),
    Text(String),
    Number {
        numerator: String,
        denominator: String,
    },
    Children(Vec<ProtocolNode>),
    /// A Record's two aligned sequences (wire type `record`).
    Record {
        keys: Vec<ProtocolNode>,
        values: Vec<ProtocolNode>,
    },
}

/// One canonical term of an algebraic value's multiquadratic normal form.
/// Strings keep arbitrary-precision integers lossless at every host boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProtocolExactTerm {
    pub(crate) numerator: String,
    pub(crate) denominator: String,
    pub(crate) radicand: String,
}

/// Return the exact wire representation when `value` is an algebraic scalar.
///
/// Both the WASM and native CLI serializers call this helper. Keeping the
/// extraction here prevents one host from exposing the normal form while
/// another silently returns only its rational approximation.
pub(crate) fn exact_terms(value: &Value) -> Option<Vec<ProtocolExactTerm>> {
    Some(
        algebraic_normal_form(value)?
            .into_iter()
            .map(|(coefficient, radicand)| ProtocolExactTerm {
                numerator: coefficient.numerator().to_string(),
                denominator: coefficient.denominator().to_string(),
                radicand: radicand.to_string(),
            })
            .collect(),
    )
}

/// The multiquadratic normal form Σ c_m √m behind an algebraic scalar, or
/// `None` for every other value.
///
/// `exact_terms` reads it for the wire, and the stack display renders the
/// same terms (`display::render_algebraic_terms`), so the rendering a reader
/// sees and the terms a consumer computes with are one extraction.
fn algebraic_normal_form(value: &Value) -> Option<Vec<(Fraction, BigInt)>> {
    let ValueData::ExactScalar(exact) = &value.data else {
        return None;
    };
    let crate::types::exact::ExactReal::Algebraic(algebraic) = &**exact else {
        return None;
    };
    Some(algebraic.normal_form_terms())
}

/// A rational near an irrational value that costs one integer square root per
/// term, for when the enclosure-based approximation answers nothing (a bound
/// no enclosure can reach). Each term `c·√m` contributes `c·⌊√m·10⁹⌋/10⁹`,
/// so the sum is within `terms·|c|·10⁻⁹` of the value: coarse, marked
/// `approximate` like every approximation here, and never a denominator of
/// zero.
fn termwise_approximation(er: &crate::types::exact::ExactReal) -> Fraction {
    let crate::types::exact::ExactReal::Algebraic(algebraic) = er else {
        return er
            .as_rational()
            .cloned()
            .unwrap_or_else(|| Fraction::from(0));
    };
    let scale = BigInt::from(1_000_000_000u64);
    let scale_squared = &scale * &scale;
    algebraic.normal_form_terms().into_iter().fold(
        Fraction::from(0),
        |sum, (coefficient, radicand)| {
            let root = (radicand * &scale_squared).sqrt();
            sum.add(&coefficient.mul(&Fraction::new(root, scale.clone())))
        },
    )
}

fn number_protocol_value(f: &Fraction) -> ProtocolValue {
    // The decimal spelling of each half as held; a pair in two machine words
    // is spelled from the words, not through two `BigInt` copies.
    if let crate::types::fraction::FractionRepr::Small(n, d) = f.repr {
        return ProtocolValue::Number {
            numerator: n.to_string(),
            denominator: d.to_string(),
        };
    }
    ProtocolValue::Number {
        numerator: f.numerator().to_string(),
        denominator: f.denominator().to_string(),
    }
}

/// Flatten a dense tensor into protocol leaves. A dense tensor holds numbers
/// only (neither a Boolean nor a NIL densifies), so every lane is a `number`,
/// one of the two points over zero included. An interior node of rank >= 2
/// is a Vector of Vectors, whose `semantics` is empty like any Vector's.
fn tensor_to_protocol(data: &DenseTensor, offset: usize, shape: &[usize]) -> Vec<ProtocolNode> {
    if shape.is_empty() || shape.len() == 1 {
        let len = shape.first().copied().unwrap_or_else(|| data.len());
        (offset..offset + len)
            .map(|lane| {
                let leaf = Value::from_dense_lane(data, lane);
                let value = match &leaf.data {
                    ValueData::Scalar(f) => number_protocol_value(f),
                    _ => unreachable!("a dense lane is a Scalar"),
                };
                ProtocolNode {
                    type_str: "number",
                    value,
                    semantics: leaf,
                }
            })
            .collect()
    } else {
        let outer = shape[0];
        let rest = &shape[1..];
        let stride: usize = rest.iter().product();
        (0..outer)
            .map(|i| ProtocolNode {
                type_str: "vector",
                value: ProtocolValue::Children(tensor_to_protocol(data, offset + i * stride, rest)),
                // A Vector's semantics block is empty whatever it holds, so
                // the row need not be materialized to derive it.
                semantics: Value::from_vector(Vec::new()),
            })
            .collect()
    }
}

/// The complete, pure Value -> protocol mapping. This is the single source of
/// truth for the value wire format (WASM and CLI) and the unit of native
/// verification for the serialization boundary. `type` is the value's domain
/// (LANG.VALUES.DISJOINT); nothing about how the value was produced reaches it.
pub(crate) fn value_to_protocol(value: &Value) -> ProtocolNode {
    let (type_str, protocol_value) = match &value.data {
        // A representation, not a domain: the wire sees the Vector it is.
        ValueData::Truths(t) => return value_to_protocol(&t.to_vector()),
        ValueData::Nil(_) => ("nil", ProtocolValue::Null),
        ValueData::Boolean(b) => ("boolean", ProtocolValue::Bool(*b)),
        ValueData::ExactScalar(er) => {
            // Serialize ExactScalar as best rational approximation with large
            // denominator. The resulting node carries `semantics:
            // Some(value.clone())` (the original exact real) plus an
            // `approximate: true` marker in its semantics block (see the
            // serializers), so the approximation is observable and the
            // consumer can reference the exact source (LANG.OBSERVATION.FIREWALL).
            let approx = er
                .best_rational_approximation(&BigInt::from(1_000_000_000u64))
                .unwrap_or_else(|| termwise_approximation(er));
            ("number", number_protocol_value(&approx))
        }
        ValueData::Scalar(f) => ("number", number_protocol_value(f)),
        ValueData::Text(s) => ("string", ProtocolValue::Text(s.to_string())),
        ValueData::Vector(children) => (
            "vector",
            ProtocolValue::Children(children.iter().map(value_to_protocol).collect()),
        ),
        ValueData::Tensor { data, shape } => (
            "vector",
            ProtocolValue::Children(tensor_to_protocol(data, 0, shape)),
        ),
        // A Symbol is a value of its own domain (LANG.VALUES.DISJOINT); it
        // crosses this boundary as its own bare name, the only thing there
        // is to show.
        ValueData::Symbol(name) => ("symbol", ProtocolValue::Text(name.to_string())),
        // A Record crosses the boundary as its two observable sequences,
        // each an array of nodes, so a host reads keys and values without
        // guessing a key's type (LANG.RECORDS.STRUCTURE).
        ValueData::Record(record) => (
            "record",
            ProtocolValue::Record {
                keys: record.keys().iter().map(value_to_protocol).collect(),
                values: record.values().iter().map(value_to_protocol).collect(),
            },
        ),
    };
    ProtocolNode {
        type_str,
        value: protocol_value,
        semantics: value.clone(),
    }
}

#[cfg(test)]
mod termwise_approximation_tests {
    use super::*;
    use crate::types::exact::ExactReal;

    /// The fallback never answers `0/0`, and lands within its stated bound.
    #[test]
    fn the_fallback_approximation_is_a_number_near_the_value() {
        let sqrt2 = ExactReal::from_sqrt_rational(Fraction::from(2)).expect("√2");
        let approx = termwise_approximation(&sqrt2);
        assert_eq!(
            approx,
            Fraction::new(
                BigInt::from(1_414_213_562u64),
                BigInt::from(1_000_000_000u64)
            )
        );
        assert!(approx.is_finite());
    }
}
