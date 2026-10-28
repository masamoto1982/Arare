// AQ-VER-003-C: native verification of the pure serialization mapping
// `value_to_protocol`. These tests pin the type/value decision for every
// ValueData kind: each is a function of the value's domain alone
// (LANG.VALUES.DENOTATION, LANG.VALUES.DISJOINT).
//
// Trace: docs/quality/TRACEABILITY_MATRIX.md, requirement AQ-REQ-003.

use crate::types::fraction::Fraction;
use crate::types::value_protocol::{exact_terms, value_to_protocol, ProtocolNode, ProtocolValue};
use crate::types::{DenseTensor, Value, ValueData};
use std::sync::Arc;

fn frac(n: i64) -> Fraction {
    Fraction::from(n)
}

fn scalar(n: i64) -> Value {
    Value::from_fraction(frac(n))
}

fn vector(children: Vec<Value>) -> Value {
    Value::from_children(children)
}

fn tensor(nums: &[i64], shape: &[usize]) -> Value {
    let fracs: Vec<Fraction> = nums.iter().map(|n| frac(*n)).collect();
    let dense =
        DenseTensor::from_fractions(fracs, shape.to_vec()).expect("rectangular tensor for test");
    Value::new(
        ValueData::Tensor {
            data: Arc::new(dense),
            shape: Arc::new(shape.to_vec()),
        },
        None,
    )
}

fn num(numerator: &str, denominator: &str) -> ProtocolValue {
    ProtocolValue::Number {
        numerator: numerator.to_string(),
        denominator: denominator.to_string(),
    }
}

fn children_of(node: &ProtocolNode) -> &[ProtocolNode] {
    match &node.value {
        ProtocolValue::Children(kids) => kids,
        other => panic!("expected Children, got {:?}", other),
    }
}
#[test]
fn plain_nil_is_still_nil_not_unknown() {
    let node = value_to_protocol(&Value::nil());
    assert_eq!(node.type_str, "nil");
    assert_eq!(node.value, ProtocolValue::Null);
}

/// A Symbol is its own domain on the wire, carrying its bare name. It used
/// to serialize as `nil` (as part of the pre-unification CodeBlock domain),
/// so every host drew `NIL` for a value `NIL?` answers FALSE for and `EXEC`
/// runs — the internal representation observed as the wrong domain, which
/// LANG.OBSERVATION.FIREWALL rules out.
#[test]
fn a_symbol_serializes_as_its_own_domain_with_its_bare_name() {
    let node = value_to_protocol(&Value::from_symbol("MUL"));
    assert_eq!(node.type_str, "symbol");
    assert_eq!(node.value, ProtocolValue::Text("MUL".to_string()));
}

// --- ExactScalar approximation marker (LANG.OBSERVATION.FIREWALL) ---

/// √2 as an exact irrational (AlgebraicSqrt), the canonical ExactScalar.
fn sqrt2() -> Value {
    use crate::types::exact::ExactReal;
    let er = ExactReal::from_sqrt_rational(frac(2)).expect("√2 is a valid exact real");
    let v = Value::from_exact_real(er);
    assert!(
        matches!(v.data, ValueData::ExactScalar(_)),
        "√2 must remain an ExactScalar, not collapse to a rational"
    );
    v
}

/// An ExactScalar serializes as a `number` (its best rational approximation)
/// but its `semantics` block must carry the original exact value, so the GUI
/// can reference the exact source rather than a silent truncation
/// (LANG.OBSERVATION.FIREWALL).
#[test]
fn exact_scalar_carries_exact_source_in_semantics() {
    let node = value_to_protocol(&sqrt2());
    assert_eq!(node.type_str, "number", "ExactScalar -> number");
    assert!(
        matches!(node.value, ProtocolValue::Number { .. }),
        "value is the rational approximation, got {:?}",
        node.value
    );
    let semantics = &node.semantics;
    assert!(
        matches!(semantics.data, ValueData::ExactScalar(_)),
        "semantics must preserve the exact ExactScalar source, got {:?}",
        semantics.data
    );
}

/// A truth column is a representation, not a domain: it crosses the wire as
/// the Vector it stands for, node for node.
#[test]
fn a_truth_column_serializes_as_its_vector() {
    use crate::error::NilReason;
    use crate::types::TruthColumn;
    let lanes = vec![
        Value::from_bool(true),
        Value::from_bool(false),
        Value::nil_with_reason(NilReason::DomainMiss),
    ];
    let column = Value::from_truth_column(TruthColumn::pack(&lanes).unwrap());
    let vector = Value::from_vector(lanes);
    assert_eq!(
        format!("{:?}", value_to_protocol(&column)),
        format!("{:?}", value_to_protocol(&vector))
    );
}

/// Only an irrational has a normal form to send. A restored session can hold
/// an `ExactScalar` that is rational (the wire's `ExactRat`), and a plain
/// number is not an `ExactScalar` at all; neither has exact terms.
#[test]
fn only_an_irrational_has_exact_terms() {
    let rational = Value::new(
        ValueData::ExactScalar(Box::new(crate::types::exact::ExactReal::Rational(frac(3)))),
        None,
    );
    assert!(exact_terms(&rational).is_none());
    assert!(exact_terms(&scalar(3)).is_none());
    assert!(exact_terms(&sqrt2()).is_some());
}

#[test]
fn algebraic_exact_terms_are_lossless_decimal_strings() {
    use crate::types::value_protocol::exact_terms;

    assert_eq!(
        exact_terms(&sqrt2()).expect("sqrt(2) has an algebraic normal form"),
        vec![crate::types::value_protocol::ProtocolExactTerm {
            numerator: "1".to_string(),
            denominator: "1".to_string(),
            radicand: "2".to_string(),
        }]
    );
}

/// The stack display of an algebraic value writes the same normal form
/// `exactTerms` carries, as one short token; the node's own `value` is an
/// approximation. There is no second rendering field: every host shows the
/// stack display, so this pins it.
#[test]
fn algebraic_stack_display_writes_the_normal_form_short() {
    use crate::types::exact::ExactReal;
    use crate::types::value_protocol::exact_terms;

    let display = |value: &Value| value.to_string();
    let sqrt_of = |n: i64| {
        Value::from_exact_real(
            ExactReal::from_sqrt_rational(frac(n)).expect("a supported algebraic square root"),
        )
    };

    // A unit coefficient is left unwritten: `1/1*sqrt(2)` says nothing more.
    assert_eq!(display(&sqrt2()), "sqrt(2)");

    // A non-unit coefficient keeps Ajisai's own `numerator/denominator`
    // rendering. `2*sqrt(2)` would be the only place in the language where a
    // number is written without its denominator.
    let two_sqrt2 = match sqrt2().data {
        ValueData::ExactScalar(exact) if matches!(*exact, ExactReal::Algebraic(_)) => {
            let ExactReal::Algebraic(algebraic) = *exact else {
                unreachable!()
            };
            match algebraic.mul_fraction(&Fraction::new(2.into(), 1.into())) {
                crate::types::exact::AlgebraicResult::Irrational(scaled) => {
                    Value::from_exact_real(ExactReal::Algebraic(Box::new(scaled)))
                }
                other => panic!("2·√2 stays irrational, got {other:?}"),
            }
        }
        ref other => panic!("√2 is an algebraic ExactScalar, got {other:?}"),
    };
    assert_eq!(display(&two_sqrt2), "2/1*sqrt(2)");

    // Only an algebraic value carries terms.
    for value in [scalar(3), vector(vec![scalar(1)]), Value::nil()] {
        assert_eq!(exact_terms(&value), None, "{value:?} has no normal form");
    }

    // Two equal values hold the same terms, so the display and the
    // `exactTerms` beside it agree and neither depends on how the value was
    // built (LANG.VALUES.DENOTATION): √8 is 2√2.
    assert_eq!(display(&sqrt_of(8)), "2/1*sqrt(2)");
    assert_eq!(exact_terms(&sqrt_of(8)), exact_terms(&two_sqrt2));
}

// --- scalar and truth domains ---

#[test]
fn scalar_is_number() {
    let node = value_to_protocol(&scalar(7));
    assert_eq!(node.type_str, "number");
    assert_eq!(node.value, num("7", "1"));
}

/// LANG.VALUES.DISJOINT: TRUE is not scalar one and FALSE is not scalar zero,
/// on the wire as anywhere else.
#[test]
fn zero_and_one_are_numbers_and_booleans_are_booleans() {
    for n in [0, 1] {
        let node = value_to_protocol(&scalar(n));
        assert_eq!(node.type_str, "number");
        assert_eq!(node.value, num(&n.to_string(), "1"));
    }
    for b in [true, false] {
        let node = value_to_protocol(&Value::from_bool(b));
        assert_eq!(node.type_str, "boolean");
        assert_eq!(node.value, ProtocolValue::Bool(b));
    }
}

/// UNKNOWN is a NIL (LANG.VALUES.TRUTH), so it is observed as one: there is
/// no truth-valued NIL on the wire.
#[test]
fn a_nil_in_truth_position_is_observed_as_a_nil() {
    let unknown = Value::nil_with_reason(crate::error::NilReason::DomainMiss);
    let node = value_to_protocol(&unknown);
    assert_eq!(node.type_str, "nil");
    assert_eq!(node.value, ProtocolValue::Null);
    assert_eq!(unknown.truth_value(), None);
}

#[test]
fn a_string_projects_to_a_string_leaf() {
    let node = value_to_protocol(&Value::from_string("A"));
    assert_eq!(node.type_str, "string");
    assert_eq!(node.value, ProtocolValue::Text("A".to_string()));
}

#[test]
fn a_scalar_never_projects_as_a_string() {
    let node = value_to_protocol(&scalar(65));
    assert_eq!(node.type_str, "number");
}

// --- Vector branch ---

#[test]
fn vector_structural_renders_number_children() {
    let node = value_to_protocol(&vector(vec![scalar(1), scalar(2)]));
    assert_eq!(node.type_str, "vector");
    let kids = children_of(&node);
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0].type_str, "number");
    assert_eq!(kids[1].value, num("2", "1"));
}

#[test]
fn a_vector_of_booleans_has_boolean_children() {
    let node = value_to_protocol(&vector(vec![
        Value::from_bool(true),
        Value::from_bool(false),
        Value::nil(),
    ]));
    assert_eq!(node.type_str, "vector");
    let kids = children_of(&node);
    assert_eq!(kids[0].type_str, "boolean");
    assert_eq!(kids[0].value, ProtocolValue::Bool(true));
    assert_eq!(kids[1].value, ProtocolValue::Bool(false));
    assert_eq!(kids[2].type_str, "nil");
}

#[test]
fn a_codepoint_vector_projects_as_a_vector() {
    // `[ 65 66 ]` is a Vector of two numbers, not the string `'AB'`. This is
    // the protocol-side face of `'A' [ 65 ] EQ` answering false.
    let node = value_to_protocol(&vector(vec![scalar(65), scalar(66)]));
    assert_eq!(node.type_str, "vector");
}

// --- Tensor branch: the regression that motivated this layer ---

#[test]
fn tensor_1d_default_renders_numbers() {
    let node = value_to_protocol(&tensor(&[1, 2, 3], &[3]));
    let kids = children_of(&node);
    assert_eq!(kids.len(), 3);
    assert!(kids.iter().all(|k| k.type_str == "number"));
    assert_eq!(kids[2].value, num("3", "1"));
}

#[test]
fn tensor_2d_numbers_nest() {
    let node = value_to_protocol(&tensor(&[1, 2, 3, 4], &[2, 2]));
    let rows = children_of(&node);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].type_str, "vector");
    assert_eq!(
        rows[0].semantics,
        Value::from_vector(Vec::new()),
        "an interior tensor row derives its semantics as any Vector does"
    );
    let leaves = children_of(&rows[0]);
    assert_eq!(leaves[0].value, num("1", "1"));
    assert_eq!(leaves[1].value, num("2", "1"));
}

#[test]
fn a_dense_tensor_projects_as_a_vector() {
    // Strings are never densified now, so a dense numeric tensor has no text
    // reading available to it at all.
    let node = value_to_protocol(&tensor(&[72, 105], &[2]));
    assert_eq!(node.type_str, "vector");
}

// --- remaining ValueData kinds ---
#[test]
fn a_nested_vector_reads_the_same_dense_or_boxed() {
    // Storage is unobservable (LANG.AUTHORITY.FREEDOM): a rank-2 tensor and
    // the same rows held as boxed Vectors render one protocol node.
    let dense = crate::agent::report::protocol_node_json(&value_to_protocol(&tensor(
        &[1, 2, 3, 4],
        &[2, 2],
    )));
    let boxed =
        crate::agent::report::protocol_node_json(&value_to_protocol(&Value::from_children(vec![
            Value::from_children(vec![scalar(1), scalar(2)]),
            Value::from_children(vec![scalar(3), scalar(4)]),
        ])));
    assert_eq!(dense, boxed);
}

// --- the decisions the rows above leave out ---
//
// `value_to_protocol` arms with no row above: Record.
// `number_protocol_value`, `if let Small(n, d)`: T (the pair from the words),
//   F (the pair from the `BigInt`s) — one spelling either way.
// `tensor_to_protocol`, `shape.is_empty() || shape.len() == 1` (E, O):
//   (T, *) leaves for every lane;  (F, T) leaves;  (F, F) a row per index.

#[test]
fn a_record_crosses_as_its_two_aligned_sequences() {
    let record = crate::types::RecordData::new(
        vec![Value::from_string("a"), scalar(2)],
        vec![scalar(1), Value::from_bool(false)],
    )
    .expect("distinct keys");
    let node = value_to_protocol(&Value::from_record(record));
    assert_eq!(node.type_str, "record");
    let ProtocolValue::Record { keys, values } = &node.value else {
        panic!("expected Record, got {:?}", node.value);
    };
    let key_types: Vec<_> = keys.iter().map(|k| k.type_str).collect();
    assert_eq!(key_types, ["string", "number"]);
    assert_eq!(keys[0].value, ProtocolValue::Text("a".to_string()));
    assert_eq!(values[0].value, num("1", "1"));
    assert_eq!(values[1].value, ProtocolValue::Bool(false));
}

#[test]
fn a_number_is_spelled_the_same_from_words_or_from_bigints() {
    use num_bigint::BigInt;
    let half = Value::from_fraction(Fraction::new(BigInt::from(-1), BigInt::from(2)));
    assert_eq!(value_to_protocol(&half).value, num("-1", "2"));
    let wide = Fraction::new(BigInt::from(1) << 70, BigInt::from(3));
    assert!(!wide.is_small());
    assert_eq!(
        value_to_protocol(&Value::from_fraction(wide)).value,
        num("1180591620717411303424", "3")
    );
    for (point, numerator) in [(Fraction::infinity(), "1"), (Fraction::nullity(), "0")] {
        let node = value_to_protocol(&Value::from_fraction(point));
        assert_eq!(node.type_str, "number");
        assert_eq!(node.value, num(numerator, "0"));
    }
}

#[test]
fn tensor_shape_decides_leaves_or_rows() {
    // (T, *): no axes, so the lanes themselves (here, none).
    let empty = value_to_protocol(&tensor(&[], &[]));
    assert_eq!(empty.type_str, "vector");
    assert!(children_of(&empty).is_empty());
    // (F, T): one axis, leaves.
    let flat = value_to_protocol(&tensor(&[4, 5], &[2]));
    assert!(children_of(&flat).iter().all(|k| k.type_str == "number"));
    // (F, F): a row per index, recursively — rank 3 as [2, 1, 2].
    let deep = value_to_protocol(&tensor(&[1, 2, 3, 4], &[2, 1, 2]));
    let rows = children_of(&deep);
    assert_eq!(rows.len(), 2);
    let inner = children_of(&rows[1]);
    assert_eq!(inner.len(), 1);
    assert_eq!(inner[0].type_str, "vector");
    let leaves = children_of(&inner[0]);
    assert_eq!(leaves[0].value, num("3", "1"));
    assert_eq!(leaves[1].value, num("4", "1"));
}

#[test]
fn a_dense_lane_keeps_a_fraction_and_a_point() {
    let lanes = vec![Fraction::new(1.into(), 2.into()), Fraction::infinity()];
    let dense = DenseTensor::from_fractions(lanes, vec![2]).expect("dense lanes");
    let value = Value::new(
        ValueData::Tensor {
            data: Arc::new(dense),
            shape: Arc::new(vec![2]),
        },
        None,
    );
    let node = value_to_protocol(&value);
    let kids = children_of(&node);
    assert_eq!(kids[0].value, num("1", "2"));
    assert_eq!(kids[1].value, num("1", "0"));
}

mod protocol_property_tests {
    use super::*;
    use proptest::prelude::*;

    fn tensor_1d(nums: &[i64]) -> Value {
        let fracs: Vec<Fraction> = nums.iter().map(|n| Fraction::from(*n)).collect();
        let len = fracs.len();
        let dense = DenseTensor::from_fractions(fracs, vec![len]).expect("1d tensor");
        Value::new(
            ValueData::Tensor {
                data: Arc::new(dense),
                shape: Arc::new(vec![len]),
            },
            None,
        )
    }

    proptest! {
        // A dense tensor holds numbers: every leaf is a number, never a
        // boolean, whatever produced it.
        #[test]
        fn default_tensor_leaves_are_number(nums in proptest::collection::vec(-5i64..5, 1..12)) {
            let node = value_to_protocol(&tensor_1d(&nums));
            let kids = match node.value {
                ProtocolValue::Children(k) => k,
                other => panic!("expected Children, got {:?}", other),
            };
            for kid in &kids {
                prop_assert_eq!(kid.type_str, "number");
                let is_number = matches!(kid.value, ProtocolValue::Number { .. });
                prop_assert!(is_number);
            }
        }
    }
}
