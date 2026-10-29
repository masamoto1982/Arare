//! AR-VER-213: MC/DC tables for the interpreter's compound decisions.

use crate::lift::{and, lift2};
use crate::value::{ErrorKind, Reason, Value};
use arare_kernel::Num;

fn n(k: i64) -> Value {
    Value::Num(Num::int(k).unwrap())
}

/// What a value denotes, whatever width it took.
fn shown(v: Result<Value, ErrorKind>) -> String {
    v.map_or_else(|e| format!("{e:?}"), |v| v.to_string())
}

fn add(a: &Value, b: &Value) -> Result<Value, ErrorKind> {
    lift2(a, b, &|x, y| match (x, y) {
        (Value::Num(x), Value::Num(y)) => Ok(Value::Num(x.add(*y))),
        _ => Err(ErrorKind::Type("a number")),
    })
}

/// Pairing two vectors: equal lengths (A), or a one-lane left (B) or right
/// (C) operand; none of them refuses.
#[test]
fn ar_ver_213_a_pairing() {
    let v = |ks: &[i64]| Value::Vector(ks.iter().map(|&k| n(k)).collect());
    assert_eq!(shown(add(&v(&[1, 2]), &v(&[3, 4]))), "[ 4/1 6/1 ]"); //  (T, F, F)
    assert_eq!(shown(add(&v(&[1]), &v(&[3, 4]))), "[ 4/1 5/1 ]"); //     (F, T, F)
    assert_eq!(shown(add(&v(&[1, 2]), &v(&[3]))), "[ 4/1 5/1 ]"); //     (F, F, T)
    assert_eq!(
        add(&v(&[1, 2]), &v(&[3, 4, 5])),
        Err(ErrorKind::Shape { left: 2, right: 3 })
    ); // (F, F, F)
}

/// NIL passes through: a NIL left (A) or right (B) operand is the result,
/// the left when both are.
#[test]
fn ar_ver_213_b_passthrough() {
    let (l, o) = (
        Value::Nil(Reason::Literal),
        Value::Nil(Reason::OrderOfPoint),
    );
    assert_eq!(add(&l, &n(1)).unwrap(), l); //  (T, F)
    assert_eq!(add(&n(1), &o).unwrap(), o); //  (F, T)
    assert_eq!(add(&l, &o).unwrap(), l); //     (T, T): the left
    assert_eq!(shown(add(&n(1), &n(2))), "3/1"); // (F, F)
}

/// AND reads NIL as UNKNOWN and keeps its reason; FALSE settles it.
#[test]
fn ar_ver_213_c_and_unknown() {
    let (t, f, u) = (
        Value::Bool(true),
        Value::Bool(false),
        Value::Nil(Reason::OrderOfPoint),
    );
    assert_eq!(and(&t, &u).unwrap(), u);
    assert_eq!(and(&u, &f).unwrap(), f);
    assert_eq!(and(&u, &Value::Nil(Reason::Literal)).unwrap(), u); // the left reason
    assert_eq!(and(&t, &t).unwrap(), t);
}
