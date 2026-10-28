//! Phase C: end-to-end verification of the real WASM serialization boundary.
//!
//! Phases A and B verify the Value -> protocol mapping and the
//! interpreter's NIL-projection behavior natively, on the host target -- those
//! never cross the `wasm-bindgen` glue. This crate closes the last gap: it
//! drives the public `AjisaiInterpreter` API compiled to `wasm32`, executes
//! code, and reads back the actual `JsValue` the GUI receives from
//! `collect_stack`, so the wasm-bindgen codegen and `js_sys::Reflect`
//! plumbing are exercised for real, not merely compile-checked.
//!
//! Run: `cd rust/wasm-tests && wasm-pack test --node`.
//!
//! Trace: docs/quality/TRACEABILITY_MATRIX.md (AQ-REQ-003, WASM boundary).
#![cfg(target_arch = "wasm32")]

use ajisai_core::AjisaiInterpreter;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

fn field(obj: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(obj, &JsValue::from_str(key)).expect("field present")
}

fn type_of(node: &JsValue) -> String {
    field(node, "type").as_string().expect("type is a string")
}

fn children(node: &JsValue) -> js_sys::Array {
    js_sys::Array::from(&field(node, "value"))
}

async fn stack_of(code: &str) -> js_sys::Array {
    let mut interp = AjisaiInterpreter::new();
    interp.execute(code).await.expect("execution succeeds");
    js_sys::Array::from(&interp.collect_stack())
}

/// Regression for #972 at the real boundary: `[ TRUE ]` is a promoted dense
/// boolean tensor; its element must cross the wasm boundary as a boolean,
/// not a `1/1` number.
#[wasm_bindgen_test]
async fn boolean_vector_serializes_as_booleans() {
    let stack = stack_of("[ TRUE ]").await;
    assert_eq!(stack.length(), 1);
    let node = stack.get(0);
    assert_eq!(type_of(&node), "vector");
    let kids = children(&node);
    assert_eq!(kids.length(), 1);
    let kid = kids.get(0);
    assert_eq!(type_of(&kid), "boolean");
    assert_eq!(field(&kid, "value").as_bool(), Some(true));
}

#[wasm_bindgen_test]
async fn false_vector_serializes_as_boolean_false() {
    let stack = stack_of("[ FALSE ]").await;
    let kid = children(&stack.get(0)).get(0);
    assert_eq!(type_of(&kid), "boolean");
    assert_eq!(field(&kid, "value").as_bool(), Some(false));
}

#[wasm_bindgen_test]
async fn number_vector_serializes_as_numbers() {
    let stack = stack_of("[ 1 2 3 ]").await;
    let node = stack.get(0);
    assert_eq!(type_of(&node), "vector");
    let kids = children(&node);
    assert_eq!(kids.length(), 3);
    for i in 0..3 {
        let kid = kids.get(i);
        assert_eq!(type_of(&kid), "number");
        let value = field(&kid, "value");
        assert!(field(&value, "numerator").as_string().is_some());
        assert!(field(&value, "denominator").as_string().is_some());
    }
}

/// A scalar comparison result crosses the boundary as a top-level boolean
/// (TruthValue role on a bare scalar), distinct from the vector case above.
#[wasm_bindgen_test]
async fn scalar_comparison_serializes_as_boolean() {
    let stack = stack_of("3 5 LT").await;
    assert_eq!(stack.length(), 1);
    let node = stack.get(0);
    assert_eq!(type_of(&node), "boolean");
    assert_eq!(field(&node, "value").as_bool(), Some(true));
}

/// A reasoned NIL (a negative radicand) crosses the boundary as a `nil`-typed node.
#[wasm_bindgen_test]
async fn projected_nil_serializes_as_nil() {
    let stack = stack_of("-1 SQRT").await;
    assert_eq!(stack.length(), 1);
    assert_eq!(type_of(&stack.get(0)), "nil");
}

/// A quotient by zero is a number (LANG.VALUES.EXACT): it crosses the boundary
/// as a `number` node whose denominator is `"0"`, never as a `nil`.
#[wasm_bindgen_test]
async fn quotient_by_zero_serializes_as_a_number_over_zero() {
    let stack = stack_of("100 0 DIV").await;
    assert_eq!(stack.length(), 1);
    let node = stack.get(0);
    assert_eq!(type_of(&node), "number");
    let value = field(&node, "value");
    assert_eq!(field(&value, "numerator").as_string().as_deref(), Some("1"));
    assert_eq!(
        field(&value, "denominator").as_string().as_deref(),
        Some("0")
    );
}

/// An ExactScalar (√2) under the default `RawNumber` role crosses the boundary
/// as a `number` (its best rational approximation) carrying an explicit
/// `semantics.approximate === true` marker, so the GUI never mistakes the
/// approximation for an exact rational (LANG.OBSERVATION.FIREWALL firewall; P1).
#[wasm_bindgen_test]
async fn exact_scalar_rawnumber_marks_approximate_at_boundary() {
    let stack = stack_of("2 SQRT").await;
    assert_eq!(stack.length(), 1);
    let node = stack.get(0);
    assert_eq!(
        type_of(&node),
        "number",
        "√2 (RawNumber) serializes as number"
    );
    // The numeric value is present (the rational approximation).
    let value = field(&node, "value");
    assert!(field(&value, "numerator").as_string().is_some());
    assert!(field(&value, "denominator").as_string().is_some());
    // The approximation marker rides on the semantics metadata bag.
    let semantics = field(&node, "semantics");
    assert_eq!(
        field(&semantics, "approximate").as_bool(),
        Some(true),
        "ExactScalar rendered lossily must be marked approximate"
    );
}

/// A genuinely exact rational (not an ExactScalar) is NOT marked approximate:
/// the marker is specific to exact irrationals collapsed to an approximation.
#[wasm_bindgen_test]
async fn exact_rational_is_not_marked_approximate() {
    let stack = stack_of("3 4 DIV").await;
    assert_eq!(stack.length(), 1);
    let node = stack.get(0);
    assert_eq!(type_of(&node), "number");
    let semantics = field(&node, "semantics");
    // `approximate` is absent (undefined) for exact rationals.
    assert!(
        field(&semantics, "approximate").as_bool().is_none(),
        "exact rational must not carry an approximate marker"
    );
}

// ---------------------------------------------------------------------------
// Inbound boundary: restore_stack_snapshot with untrusted / malformed payloads.
//
// The lossless snapshot string is the one stack format persistence accepts, and
// it is untrusted: it can be tampered with in IndexedDB or arrive across the
// worker boundary. A malformed payload must surface a recoverable error rather
// than panic the module into an unrecoverable trap. The two inputs that trap
// most easily were a zero denominator (`Fraction::new` used to panic on one;
// it is now the point over zero it spells, LANG.VALUES.EXACT) and a deeply
// nested vector (unbounded recursion overflows the wasm stack).
// ---------------------------------------------------------------------------

/// One stack slot in the persistence wire format: the given value payload.
fn snapshot_of(data: &str) -> String {
    format!("[{data}]")
}

#[wasm_bindgen_test]
fn restore_stack_snapshot_rejects_malformed_json_without_panicking() {
    let mut interp = AjisaiInterpreter::new();
    let result = interp.restore_stack_snapshot("{ not a snapshot");
    assert!(
        result.is_err(),
        "a malformed snapshot must be a recoverable error, not a panic"
    );
}

#[wasm_bindgen_test]
fn restore_stack_snapshot_accepts_valid_rational() {
    let mut interp = AjisaiInterpreter::new();
    let snapshot = snapshot_of("{\"t\":\"Scalar\",\"n\":\"3\",\"d\":\"4\"}");
    let result = interp.restore_stack_snapshot(&snapshot);
    assert!(
        result.is_ok(),
        "a valid rational must restore successfully: {result:?}"
    );
    let stack = js_sys::Array::from(&interp.collect_stack());
    assert_eq!(stack.length(), 1, "one slot restores to one stack value");
}

#[wasm_bindgen_test]
fn restore_stack_snapshot_survives_a_zero_denominator() {
    // A zero denominator is a number like any other pair (LANG.VALUES.EXACT):
    // `7/0` decodes as the point over zero it spells, `1/0`, instead of
    // panicking in `Fraction::new`.
    let mut interp = AjisaiInterpreter::new();
    let snapshot = snapshot_of("{\"t\":\"Scalar\",\"n\":\"7\",\"d\":\"0\"}");
    let result = interp.restore_stack_snapshot(&snapshot);
    assert!(
        result.is_ok(),
        "a zero denominator must decode as the point over zero, not panic: {result:?}"
    );
    let stack = js_sys::Array::from(&interp.collect_stack());
    assert_eq!(stack.length(), 1);
    let node = stack.get(0);
    assert_eq!(type_of(&node), "number");
    let value = field(&node, "value");
    assert_eq!(field(&value, "numerator").as_string().as_deref(), Some("1"));
    assert_eq!(
        field(&value, "denominator").as_string().as_deref(),
        Some("0")
    );
}

#[wasm_bindgen_test]
fn restore_stack_snapshot_rejects_deeply_nested_payload_without_overflow() {
    // Wrap a scalar in `{"t":"Vector","items":[ ... ]}` far beyond any depth a
    // real session reaches; decoding this must error rather than recurse until
    // the wasm stack overflows.
    let mut data = String::from("{\"t\":\"Scalar\",\"n\":\"1\",\"d\":\"1\"}");
    for _ in 0..1000 {
        data = format!("{{\"t\":\"Vector\",\"items\":[{data}]}}");
    }
    let mut interp = AjisaiInterpreter::new();
    let result = interp.restore_stack_snapshot(&snapshot_of(&data));
    assert!(
        result.is_err(),
        "a deeply nested snapshot must error, not overflow the stack"
    );
}

/// A saved word as the host hands it over: `{ name, definition }`.
fn saved_word(name: &str, definition: &str) -> JsValue {
    let word = js_sys::Object::new();
    js_sys::Reflect::set(&word, &"name".into(), &name.into()).expect("plain object");
    js_sys::Reflect::set(&word, &"definition".into(), &definition.into()).expect("plain object");
    word.into()
}

fn pairs_of(value: &JsValue) -> Vec<(String, String)> {
    js_sys::Array::from(value)
        .iter()
        .map(|pair| {
            let pair = js_sys::Array::from(&pair);
            (
                pair.get(0).as_string().expect("a name"),
                pair.get(1).as_string().expect("a string"),
            )
        })
        .collect()
}

/// The dictionary that crosses the boundary: `collect_user_words_info` is
/// `[name, hasDependents]` pairs, and `restore_user_words` names what it could
/// not restore — a Core name, and a redefinition of a word another word still
/// calls, which the dictionary afterwards does not show.
#[wasm_bindgen_test]
fn restore_user_words_names_what_it_could_not_restore() {
    let mut interp = AjisaiInterpreter::new();

    let words = js_sys::Array::new();
    words.push(&saved_word("ADD", "1"));
    words.push(&saved_word("QUAD", "DBL DBL"));
    words.push(&saved_word("DBL", "2 MUL"));
    let skipped = interp
        .restore_user_words(words.into())
        .expect("a well-formed list restores");
    assert_eq!(
        pairs_of(&skipped)
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        vec!["ADD"],
        "the Core name is the one entry left out"
    );

    let info = js_sys::Array::from(&interp.collect_user_words_info())
        .iter()
        .map(|pair| {
            let pair = js_sys::Array::from(&pair);
            (
                pair.get(0).as_string().expect("a name"),
                pair.get(1).as_bool().expect("a boolean"),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        info,
        vec![("DBL".to_string(), true), ("QUAD".to_string(), false)],
        "QUAD calls DBL, whichever order they were saved in"
    );

    let again = js_sys::Array::new();
    again.push(&saved_word("DBL", "3 MUL"));
    let skipped = interp
        .restore_user_words(again.into())
        .expect("a well-formed list restores");
    let skipped = pairs_of(&skipped);
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].0, "DBL");
    assert!(
        skipped[0].1.contains("referenced by QUAD"),
        "the reason names the locking word: {}",
        skipped[0].1
    );
}
