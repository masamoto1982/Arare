//! Lossless `Value` ⇄ persistence-node codec for session save/restore.
//!
//! This is the state-persistence wire format used by the WASM boundary
//! (`snapshot_stack` / `restore_stack_snapshot`). It is deliberately kept
//! **separate** from the observation protocol in
//! [`crate::types::value_protocol`]. The two have opposite requirements:
//!
//! - The observation protocol is intentionally lossy-but-honest (LANG.OBSERVATION.FIREWALL):
//!   an `ExactScalar` is observed as a *marked* rational approximation. That
//!   is correct for a display/inspection
//!   surface — it must never present a hidden truncation as exact.
//! - Persistence must be **lossless**: reloading a saved session must return
//!   the identical value. Reusing the observation protocol for save/restore
//!   silently changed values on reload — `√2` came back as the rational
//!   `768398401/543339720`.
//!
//! This codec guarantees `decode(encode(v)) == v` for every `Value`, enforced
//! by the property tests in `value_persist_tests.rs`, under `Value` equality
//! (LANG.VALUES.DENOTATION): the data, plus a NIL's reason and detail.

use crate::error::NilReason;
use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;
use crate::types::{DenseTensor, RecordData, Value, ValueData};
use num_bigint::BigInt;
use num_traits::One;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

// ---- Fraction <-> decimal string pair ----

fn frac_to_parts(f: &Fraction) -> (String, String) {
    (f.numerator().to_string(), f.denominator().to_string())
}

fn frac_from_parts(num: &str, den: &str) -> Result<Fraction, String> {
    let numerator = BigInt::from_str(num).map_err(|e| format!("bad numerator: {e}"))?;
    let denominator = BigInt::from_str(den).map_err(|e| format!("bad denominator: {e}"))?;
    // A zero denominator is a pair like any other: `Fraction::new` reduces it
    // to one of the two points over zero, so an untrusted `100/0` decodes
    // as the `1/0` it spells.
    Ok(Fraction::new(numerator, denominator))
}

// ---- Wire structures ----

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct PersistTerm {
    /// Monomial: a squarefree subset-product of the value's radical basis
    /// (`"1"` keys the rational part), as a decimal `BigInt`.
    m: String,
    /// Coefficient numerator / denominator (decimal `BigInt`).
    n: String,
    d: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t")]
enum PersistData {
    Bool {
        v: bool,
    },
    Scalar {
        n: String,
        d: String,
    },
    ExactRat {
        n: String,
        d: String,
    },
    ExactAlg {
        terms: Vec<PersistTerm>,
    },
    Vector {
        items: Vec<PersistData>,
    },
    /// A Record, persisted as its two aligned sequences; decoding rebuilds the
    /// key index and refuses a payload whose keys are not distinct.
    Record {
        keys: Vec<PersistData>,
        values: Vec<PersistData>,
    },
    /// A String, persisted as its content.
    Text {
        s: String,
    },
    /// A dense tensor: its two columns as stored. A lane with denominator 0
    /// is one of the two points over zero, a number like any other lane.
    Tensor {
        nums: Vec<i64>,
        dens: Vec<i64>,
        dshape: Vec<usize>,
        pure_int: bool,
        shape: Vec<usize>,
    },
    Nil {
        /// The NIL's reason as its protocol string. `None` decodes a payload
        /// written before reasons were carried, and a NIL that genuinely has
        /// none.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        r: Option<String>,
        /// The text a `userDeclared` NIL was given by `ABSENT`. Absent for
        /// every other reason.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ud: Option<String>,
    },
    /// A Symbol, persisted as its bare name.
    Symbol {
        name: String,
    },
}

// ---- Value <-> wire ----

fn encode_data(data: &ValueData) -> PersistData {
    match data {
        ValueData::Truths(t) => encode_data(&t.to_vector().data),
        ValueData::Boolean(b) => PersistData::Bool { v: *b },
        ValueData::Scalar(f) => {
            let (n, d) = frac_to_parts(f);
            PersistData::Scalar { n, d }
        }
        ValueData::ExactScalar(er) => match &**er {
            ExactReal::Rational(f) => {
                let (n, d) = frac_to_parts(f);
                PersistData::ExactRat { n, d }
            }
            ExactReal::Algebraic(_) => PersistData::ExactAlg {
                terms: er
                    .algebraic_terms()
                    .expect("an algebraic value has normal-form terms")
                    .iter()
                    .map(|(m, c)| {
                        let (n, d) = frac_to_parts(c);
                        PersistTerm {
                            m: m.to_string(),
                            n,
                            d,
                        }
                    })
                    .collect(),
            },
        },
        ValueData::Text(text) => PersistData::Text {
            s: text.to_string(),
        },
        ValueData::Vector(items) => PersistData::Vector {
            items: items.iter().map(encode_value).collect(),
        },
        ValueData::Tensor { data, shape } => PersistData::Tensor {
            nums: data.numerators.to_vec(),
            dens: data.denominators.to_vec(),
            dshape: data.shape.to_vec(),
            pure_int: data.is_pure_integer,
            shape: (**shape).clone(),
        },
        ValueData::Nil(_) => PersistData::Nil { r: None, ud: None },
        ValueData::Record(record) => PersistData::Record {
            keys: record.keys().iter().map(encode_value).collect(),
            values: record.values().iter().map(encode_value).collect(),
        },
        ValueData::Symbol(name) => PersistData::Symbol {
            name: name.to_string(),
        },
    }
}

fn decode_data(data: &PersistData) -> Result<ValueData, String> {
    Ok(match data {
        PersistData::Bool { v } => ValueData::Boolean(*v),
        PersistData::Scalar { n, d } => ValueData::Scalar(frac_from_parts(n, d)?),
        PersistData::ExactRat { n, d } => {
            ValueData::ExactScalar(Box::new(ExactReal::Rational(frac_from_parts(n, d)?)))
        }
        PersistData::ExactAlg { terms } => {
            // Replay ∑ cₘ·√m through the public exact arithmetic. The
            // multiquadratic normal form is canonical, so the accumulated
            // value is the identical ExactReal.
            let mut acc = ExactReal::from_integer(0);
            for term in terms {
                let monomial =
                    BigInt::from_str(&term.m).map_err(|e| format!("bad monomial: {e}"))?;
                let coeff = frac_from_parts(&term.n, &term.d)?;
                let root = ExactReal::from_sqrt_rational(Fraction::new(monomial, BigInt::one()))
                    .ok_or_else(|| "invalid monomial for √".to_string())?;
                acc = acc.add(&root.mul(&ExactReal::from_fraction(coeff)));
            }
            ValueData::ExactScalar(Box::new(acc))
        }
        PersistData::Text { s } => ValueData::Text(Arc::from(s.as_str())),
        PersistData::Vector { items } => ValueData::Vector(Arc::new(
            items
                .iter()
                .map(decode_value)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        PersistData::Tensor {
            nums,
            dens,
            dshape,
            pure_int,
            shape,
        } => {
            if nums.len() != dens.len() {
                return Err("tensor numerator/denominator length mismatch".to_string());
            }
            // The value reads its length off `shape` and its lanes off the
            // columns, so both shapes must name exactly the lanes the columns
            // hold: a forged shape would otherwise decode and then index past
            // them.
            let lanes_of = |dims: &[usize]| {
                dims.iter()
                    .try_fold(1usize, |acc, &dim| acc.checked_mul(dim))
            };
            if shape.is_empty()
                || lanes_of(shape) != Some(nums.len())
                || lanes_of(dshape) != Some(nums.len())
            {
                return Err("tensor shape does not match its columns".to_string());
            }
            ValueData::Tensor {
                data: Arc::new(DenseTensor::from_untrusted_columns(
                    nums.clone(),
                    dens.clone(),
                    dshape.clone(),
                    *pure_int,
                )),
                shape: Arc::new(shape.clone()),
            }
        }
        PersistData::Nil { .. } => ValueData::Nil(crate::semantic::NilAbsence::None),
        PersistData::Record { keys, values } => {
            let keys = keys
                .iter()
                .map(decode_value)
                .collect::<Result<Vec<_>, _>>()?;
            let values = values
                .iter()
                .map(decode_value)
                .collect::<Result<Vec<_>, _>>()?;
            ValueData::Record(Arc::new(
                RecordData::new(keys, values).map_err(|e| format!("malformed record: {e:?}"))?,
            ))
        }
        PersistData::Symbol { name } => ValueData::Symbol(Arc::from(name.as_str())),
    })
}

fn encode_value(value: &Value) -> PersistData {
    let mut d = encode_data(&value.data);
    // The wire form names the reason in its own fields, read through the
    // value's accessors, so it is attached here rather than inside
    // `encode_data`.
    if let PersistData::Nil { r, ud } = &mut d {
        *r = value
            .nil_reason()
            .map(|reason| reason.as_protocol_str().to_string());
        *ud = value.absence_detail().map(str::to_string);
    }
    d
}

fn decode_value(value: &PersistData) -> Result<Value, String> {
    if let PersistData::Nil {
        r: Some(reason),
        ud,
    } = value
    {
        let reason = NilReason::from_protocol_str(reason)
            .ok_or_else(|| format!("unknown NIL reason: {}", reason))?;
        return Ok(match (reason, ud) {
            (NilReason::UserDeclared, Some(detail)) => Value::nil_user_declared(detail),
            _ => Value::nil_with_reason(reason),
        });
    }
    Ok(Value::new(decode_data(value)?, None))
}

// ---- Public stack codec (WASM boundary) ----

/// Serialize the stack values to the lossless JSON persistence string.
pub(crate) fn encode_stack<'a>(values: impl Iterator<Item = &'a Value>) -> String {
    let wire: Vec<PersistData> = values.map(encode_value).collect();
    serde_json::to_string(&wire).expect("the persistence wire format always serializes")
}

/// Deserialize a lossless persistence string back into stack values.
pub(crate) fn decode_stack(json: &str) -> Result<Vec<Value>, String> {
    let wire: Vec<PersistData> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    wire.iter().map(decode_value).collect()
}
