//! Round-trip identity tests for the lossless persistence codec
//! (`crate::types::value_persist`). The oracle is `decode(encode(v)) == v`,
//! exercised through the `pub(crate)` stack boundary (`encode_stack` /
//! `decode_stack`) so the tests cover the exact path the WASM
//! `snapshot_stack` / `restore_stack_snapshot` methods take.

use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;
use crate::types::value_persist::{decode_stack, encode_stack};
use crate::types::{Value, ValueData};
use num_bigint::BigInt;
use num_traits::One;
use std::str::FromStr;

/// Round-trip one value as a single stack slot and return the decoded value.
fn roundtrip(value: &Value) -> Value {
    let json = encode_stack(std::iter::once(value));
    let mut decoded = decode_stack(&json).expect("decode_stack");
    assert_eq!(decoded.len(), 1, "single slot in, single slot out");
    decoded.pop().unwrap()
}

/// Assert a value survives the codec with its identity intact.
fn assert_value_roundtrip(value: Value) {
    let decoded = roundtrip(&value);
    assert_eq!(decoded, value, "value round-trip must preserve identity");
}

fn sqrt(n: i64) -> Value {
    Value::from_exact_real(ExactReal::from_sqrt_rational(Fraction::from(n)).expect("sqrt exists"))
}

#[test]
fn code_shaped_vector_survives_round_trip_instead_of_becoming_nil() {
    // Regression: the observation protocol used to map the pre-unification
    // CodeBlock domain to nil, so save/restore replaced a code block with a
    // genuine NIL. The equivalent code-shaped value post-unification is a
    // Vector holding a Symbol element, exercising the Vector/Symbol
    // persistence path `{ 42 ADD [ ] }`/`[ 42 ADD [ ] ]` both build.
    let value = Value::from_vector_promoted(vec![
        Value::from_number(Fraction::from(42)),
        Value::from_symbol("ADD"),
        Value::from_vector_promoted(vec![]),
    ]);
    assert!(matches!(value.data, ValueData::Vector(_)));
    assert_value_roundtrip(value);
}

#[test]
fn exact_sqrt_survives_round_trip_instead_of_becoming_rational() {
    // Regression: √2 was serialized as its rational approximation and
    // restored as that exact rational, changing the mathematical value.
    let value = sqrt(2);
    assert!(matches!(value.data, ValueData::ExactScalar(_)));
    assert_value_roundtrip(value);
}

#[test]
fn exact_algebraic_sum_round_trips() {
    // A multi-term multiquadratic value: √2 + √3.
    let a = ExactReal::from_sqrt_rational(Fraction::from(2)).unwrap();
    let b = ExactReal::from_sqrt_rational(Fraction::from(3)).unwrap();
    let value = Value::from_exact_real(a.add(&b));
    assert!(matches!(value.data, ValueData::ExactScalar(_)));
    assert_value_roundtrip(value);
}

#[test]
fn exact_algebraic_with_rational_part_round_trips() {
    // 1 + √2 exercises the monomial-1 (rational) term alongside √2.
    let one = ExactReal::from_integer(1);
    let root2 = ExactReal::from_sqrt_rational(Fraction::from(2)).unwrap();
    let value = Value::from_exact_real(one.add(&root2));
    assert_value_roundtrip(value);
}
#[test]
fn big_integer_scalar_round_trips() {
    // Beyond i64 range: the codec must not narrow through i64.
    let big = BigInt::from_str("340282366920938463463374607431768211457").unwrap();
    assert_value_roundtrip(Value::from_fraction(Fraction::new(big, BigInt::one())));
}
#[test]
fn nested_vector_round_trips() {
    let value = Value::from_vector(vec![
        Value::from_int(1),
        Value::from_vector(vec![Value::from_int(2), sqrt(5)]),
        Value::nil(),
    ]);
    assert_value_roundtrip(value);
}

#[test]
fn tensor_with_a_lane_over_zero_round_trips() {
    let tensor = Value::from_tensor(
        vec![
            Fraction::from(1),
            Fraction::nullity(),
            Fraction::infinity(),
            Fraction::from(4),
        ],
        vec![4],
    );
    assert!(
        matches!(&tensor.data, ValueData::Tensor { data, .. } if !data.all_finite()),
        "the fixture must actually carry a lane over zero"
    );
    assert_value_roundtrip(tensor);
    assert_value_roundtrip(Value::from_fraction(Fraction::infinity()));
}
#[test]
fn multi_slot_stack_round_trips_in_order() {
    let values = [
        Value::from_int(1),
        sqrt(2),
        Value::from_vector_promoted(vec![Value::from_number(Fraction::from(9))]),
    ];
    let json = encode_stack(values.iter());
    let decoded = decode_stack(&json).expect("decode");
    assert_eq!(decoded, values);
}

// ── a restored tensor's columns are normalized at the boundary ─────────────
//
// Every in-process path writes a tensor's columns from `Fraction`s that have
// already been normalized, so `DenseTensor::get_small_fraction` reads them back
// as they are. A restored session is the one place that guarantee does not come
// for free: the columns arrive off the wire. Reading a lane used to launder any
// payload through `Fraction::new`, which re-derived the normal form on *every*
// read — so a malformed payload never showed, and correct payloads paid a gcd
// and two `BigInt` allocations per lane per read.
//
// The check now runs once per lane per restore. These pin that it actually
// does, because the laundering that used to hide its absence is gone.

/// Encode a tensor, then rewrite its columns on the wire the way a corrupted or
/// hand-edited payload would, and decode that.
fn decode_tampered_tensor(value: &Value, from: &str, to: &str) -> Value {
    let json = encode_stack(std::iter::once(value));
    assert!(
        json.contains(from),
        "payload did not contain `{from}`: {json}"
    );
    let tampered = json.replace(from, to);
    let mut decoded = decode_stack(&tampered).expect("decode_stack");
    decoded.pop().expect("one slot")
}

#[test]
fn a_restored_tensor_lane_is_reduced_to_lowest_terms() {
    // `[ 2 3 ]` on the wire, rewritten to the same values unreduced.
    let value = Value::from_int_tensor(vec![2, 3]);
    let restored = decode_tampered_tensor(
        &value,
        r#""nums":[2,3],"dens":[1,1]"#,
        r#""nums":[4,9],"dens":[2,3]"#,
    );
    assert_eq!(
        restored.child(0).map(|c| format!("{c}")),
        Some("2/1".to_string()),
        "4/2 must restore as the value it denotes"
    );
    assert_eq!(
        restored.child(1).map(|c| format!("{c}")),
        Some("3/1".to_string()),
        "9/3 must restore as the value it denotes"
    );
    assert_eq!(restored, value, "and the whole tensor equals the original");
}

#[test]
fn a_restored_tensor_lane_carries_its_sign_on_the_numerator() {
    // A negative denominator is a second spelling of the same rational, and the
    // one that would leave a malformed `Fraction` if it were trusted as stored.
    let value = Value::from_int_tensor(vec![-2, 5]);
    let restored = decode_tampered_tensor(
        &value,
        r#""nums":[-2,5],"dens":[1,1]"#,
        r#""nums":[2,-5],"dens":[-1,-1]"#,
    );
    assert_eq!(
        restored, value,
        "sign belongs on the numerator once restored"
    );
}

#[test]
fn a_restored_tensor_reduces_a_lane_over_zero_to_its_point() {
    // A 0 denominator is a pair like any other: an untrusted `100/0` is the
    // `1/0` it spells (LANG.VALUES.EXACT), reduced at the boundary.
    let value = Value::from_int_tensor(vec![7, 8]);
    let restored = decode_tampered_tensor(
        &value,
        r#""nums":[7,8],"dens":[1,1]"#,
        r#""nums":[7,100],"dens":[1,0]"#,
    );
    assert_eq!(
        restored.child(0).map(|c| format!("{c}")),
        Some("7/1".to_string()),
        "the present lane is untouched"
    );
    assert_eq!(
        restored.child(1).map(|c| format!("{c}")),
        Some("1/0".to_string()),
        "a pair over zero reduces to its point over zero"
    );
}

#[test]
fn a_restored_tensor_does_not_keep_a_false_claim_of_purity() {
    // `pure_int` is the payload's claim about the same columns it ships. A
    // tensor whose lanes are not all integers but which says it is pure would
    // otherwise send every integer fast path down a route its own guard had
    // cleared.
    let value = Value::from_int_tensor(vec![1, 2]);
    let restored = decode_tampered_tensor(
        &value,
        r#""nums":[1,2],"dens":[1,1],"dshape":[2],"pure_int":true"#,
        r#""nums":[1,1],"dens":[1,2],"dshape":[2],"pure_int":true"#,
    );
    assert_eq!(
        restored.child(1).map(|c| format!("{c}")),
        Some("1/2".to_string()),
        "the lane restores as the rational it is"
    );
    let crate::types::ValueData::Tensor { data, .. } = &restored.data else {
        panic!("a tensor restores as a tensor");
    };
    assert!(
        !data.is_pure_integer,
        "purity is recomputed from the columns, not believed"
    );
}

#[test]
fn a_restored_tensor_whose_shape_does_not_fit_its_columns_is_refused() {
    // `Value::len()` reads the shape and the lanes read the columns, so a shape
    // that disagrees with them would decode and then panic when displayed or
    // sliced. The decoder refuses it the way it refuses mismatched columns.
    for json in [
        // The shape names more lanes than the columns hold.
        r#"[{"t":"Tensor","nums":[1],"dens":[1],"dshape":[1],"pure_int":true,"shape":[2,2]}]"#,
        r#"[{"t":"Tensor","nums":[],"dens":[],"dshape":[0],"pure_int":true,"shape":[1000000]}]"#,
        // The columns' own shape names fewer lanes than they hold.
        r#"[{"t":"Tensor","nums":[1,2],"dens":[1,1],"dshape":[1],"pure_int":true,"shape":[2]}]"#,
        // Rank 0 is a Scalar, never a tensor.
        r#"[{"t":"Tensor","nums":[1],"dens":[1],"dshape":[],"pure_int":true,"shape":[]}]"#,
        // A product that overflows.
        r#"[{"t":"Tensor","nums":[],"dens":[],"dshape":[18446744073709551615,2],"pure_int":true,"shape":[18446744073709551615,2]}]"#,
    ] {
        assert!(decode_stack(json).is_err(), "`{json}` must be refused");
    }
    // The well-formed spelling still restores.
    let good = r#"[{"t":"Tensor","nums":[1,2,3,4],"dens":[1,1,1,1],"dshape":[2,2],"pure_int":true,"shape":[2,2]}]"#;
    let value = decode_stack(good).expect("decode_stack").pop().unwrap();
    assert_eq!(format!("{value}"), "[ [ 1/1 2/1 ] [ 3/1 4/1 ] ]");
}
