//! LANG.VALUES.SHAPE, held against the implementation.
//!
//! A number read as whether its numerator and its denominator are nonzero
//! takes one of four zero shapes — `0` (Z), the point `1/0` (P), `0/0` (N)
//! and every other number (F, finite and nonzero) — and the wheel's
//! arithmetic is graded by them.
//! Each law is checked here by running programs, never by consulting the
//! implementation's own representation:
//!
//! 1. the shape of a product is the halfwise AND of the operands' shapes,
//!    and the reciprocal exchanges the halves, swapping Z and P;
//! 2. each shape is a group under `MUL` — the field's units, `{1/0}`, `{0}`
//!    and `{0/0}` — so the numbers form a Clifford monoid over the shapes;
//! 3. the shape of a sum is the Boolean-fraction sum `(ad ∨ bc, bd)`, exact
//!    except where two F operands cancel to `0`;
//! 4. every axiom of a wheel holds at every point, the commutative monoids
//!    of addition and multiplication, the reciprocal's involution and its
//!    respect for products included;
//! 5. the distributive law fails only with `1/0` as the multiplier;
//! 6. so a sum or a product does not depend on the order its operands are
//!    taken in, the points over zero included.

use crate::interpreter::Interpreter;
use crate::types::{Value, ValueData};
use num_traits::Zero;

/// Every zero shape, both signs of a finite number, a proper fraction, an
/// irrational, and `-1/0`, which is spelled with a sign and is `1/0`.
const SAMPLE: &[&str] = &["0", "1", "-2", "1/2", "2 SQRT", "1/0", "-1/0", "0/0"];

/// `(numerator ≠ 0, denominator ≠ 0)`: Z = (false, true), P = (true, false),
/// N = (false, false), F = (true, true).
type Shape = (bool, bool);

const Z: Shape = (false, true);
const P: Shape = (true, false);
const N: Shape = (false, false);
const F: Shape = (true, true);

fn meet(a: Shape, b: Shape) -> Shape {
    (a.0 && b.0, a.1 && b.1)
}

fn swap(a: Shape) -> Shape {
    (a.1, a.0)
}

/// The pair sum read over the Boolean semiring.
fn boolean_sum(a: Shape, b: Shape) -> Shape {
    ((a.0 && b.1) || (a.1 && b.0), a.1 && b.1)
}

fn shape_of(value: &Value) -> Shape {
    match &value.data {
        ValueData::Scalar(f) => (!f.numerator().is_zero(), f.is_finite()),
        // An irrational is a nonzero finite number.
        ValueData::ExactScalar(_) => F,
        other => panic!("expected a number, got {other:?}"),
    }
}

async fn run(code: &str) -> Vec<Value> {
    let mut interp = Interpreter::new();
    interp
        .execute(code)
        .await
        .unwrap_or_else(|e| panic!("`{code}` must run: {e:?}"));
    interp.get_stack().to_vec()
}

async fn number(code: &str) -> Value {
    let stack = run(code).await;
    assert_eq!(stack.len(), 1, "`{code}` must leave one value");
    stack.into_iter().next().unwrap()
}

async fn holds(code: &str) -> bool {
    match run(code).await.as_slice() {
        [v] => matches!(v.data, ValueData::Boolean(true)),
        other => panic!("`{code}` must leave one truth value, left {other:?}"),
    }
}

async fn shape(code: &str) -> Shape {
    shape_of(&number(code).await)
}

#[tokio::test]
async fn product_shape_is_the_knowledge_meet_and_reciprocal_is_not() {
    for x in SAMPLE {
        let sx = shape(x).await;
        assert_eq!(
            shape(&format!("1 {x} DIV")).await,
            swap(sx),
            "the reciprocal of {x} exchanges its halves' shape"
        );
        for y in SAMPLE {
            let sy = shape(y).await;
            assert_eq!(
                shape(&format!("{x} {y} MUL")).await,
                meet(sx, sy),
                "shape({x} · {y}) is the meet of the operands' shapes"
            );
        }
    }
}

#[tokio::test]
async fn each_shape_is_a_group_under_mul() {
    // P is the one point at infinity: `-1/0` is spelled with a sign and is it.
    assert_eq!(shape("1/0").await, P);
    assert!(holds("-1/0 1/0 EQ").await);
    assert!(holds("1/0 1/0 MUL 1/0 EQ").await);
    // Z and N are trivial groups, idempotent under MUL.
    assert!(holds("0 0 MUL 0 EQ").await);
    assert!(holds("0/0 0/0 MUL 0/0 EQ").await);
    // F is the field's multiplicative group: closed, with inverses.
    for x in ["1", "-2", "1/2", "2 SQRT"] {
        assert!(
            holds(&format!("{x} 1 {x} DIV MUL 1 EQ")).await,
            "{x} has an inverse in F"
        );
    }
    // A shape's identity acts on a lower shape as the meet says: F's identity
    // fixes every number, P's sends all of F to P, whatever its sign.
    assert!(holds("2 1/0 MUL 1/0 EQ").await);
    assert!(holds("-2 1/0 MUL 1/0 EQ").await);
}

#[tokio::test]
async fn sum_shape_is_the_boolean_fraction_sum_except_for_cancellation() {
    for x in SAMPLE {
        let sx = shape(x).await;
        for y in SAMPLE {
            let sy = shape(y).await;
            let actual = shape(&format!("{x} {y} ADD")).await;
            if sx == F && sy == F {
                assert!(
                    actual == F || actual == Z,
                    "two finite nonzero numbers sum to F or, by cancellation, Z: {x} + {y}"
                );
            } else {
                assert_eq!(actual, boolean_sum(sx, sy), "shape({x} + {y})");
            }
        }
    }
    // P + P is N, as the table says: `(1·0 + 1·0)/(0·0)`.
    assert_eq!(shape("1/0 1/0 ADD").await, N);
    assert_eq!(shape("1 -1 ADD").await, Z);
}

/// Carlström's axioms for a wheel, each written as one program comparing its
/// two sides with `EQ`, over every triple of the sample.
#[tokio::test]
async fn every_wheel_axiom_holds_at_every_point() {
    for x in SAMPLE {
        for y in SAMPLE {
            for z in SAMPLE {
                let laws = [
                    ("x+y = y+x", format!("{x} {y} ADD  {y} {x} ADD EQ")),
                    ("xy = yx", format!("{x} {y} MUL  {y} {x} MUL EQ")),
                    (
                        "x+(y+z) = (x+y)+z",
                        format!("{x} {y} {z} ADD ADD  {x} {y} ADD {z} ADD EQ"),
                    ),
                    (
                        "x(yz) = (xy)z",
                        format!("{x} {y} {z} MUL MUL  {x} {y} MUL {z} MUL EQ"),
                    ),
                    (
                        "xz+yz = (x+y)z+0z",
                        format!(
                            "{x} {z} MUL {y} {z} MUL ADD  {x} {y} ADD {z} MUL 0 {z} MUL ADD EQ"
                        ),
                    ),
                    (
                        "(x+yz)/y = x/y+z+0y",
                        format!(
                            "{x} {y} {z} MUL ADD {y} DIV  {x} {y} DIV {z} ADD 0 {y} MUL ADD EQ"
                        ),
                    ),
                    (
                        "(x+0y)z = xz+0y",
                        format!("{x} 0 {y} MUL ADD {z} MUL  {x} {z} MUL 0 {y} MUL ADD EQ"),
                    ),
                    (
                        "/(x+0y) = /x+0y",
                        format!("1 {x} 0 {y} MUL ADD DIV  1 {x} DIV 0 {y} MUL ADD EQ"),
                    ),
                    ("//x = x", format!("1 1 {x} DIV DIV {x} EQ")),
                    (
                        "/(xy) = /x/y",
                        format!("1 {x} {y} MUL DIV  1 {x} DIV 1 {y} DIV MUL EQ"),
                    ),
                    ("x-x = 0x²", format!("{x} {x} SUB  0 {x} MUL {x} MUL EQ")),
                    (
                        "x/x = 1+0x/x",
                        format!("{x} {x} DIV  1 0 {x} MUL {x} DIV ADD EQ"),
                    ),
                    ("x+0/0 = 0/0", format!("{x} 0/0 ADD 0/0 EQ")),
                ];
                for (name, law) in laws {
                    assert!(holds(&law).await, "{name} must hold at x={x}, y={y}, z={z}");
                }
            }
        }
    }
    assert!(holds("0 0 MUL 0 EQ").await, "0·0 = 0");
}

#[tokio::test]
async fn distribution_fails_only_under_the_point_at_infinity() {
    let mut failures = 0;
    for x in SAMPLE {
        for y in SAMPLE {
            for z in SAMPLE {
                let law = format!("{x} {y} {z} ADD MUL  {x} {y} MUL {x} {z} MUL ADD EQ");
                if !holds(&law).await {
                    failures += 1;
                    assert!(
                        *x == "1/0" || *x == "-1/0",
                        "x(y+z) = xy+xz may fail only with x = 1/0, failed at x={x}, y={y}, z={z}"
                    );
                }
            }
        }
    }
    assert!(failures > 0, "the distributive law does give way at 1/0");
}

/// Every way to choose `k` of `items`, keeping their order.
fn selections<'a>(items: &[&'a str], k: usize) -> Vec<Vec<&'a str>> {
    if k == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for (i, head) in items.iter().enumerate() {
        for mut tail in selections(&items[i + 1..], k - 1) {
            tail.insert(0, *head);
            out.push(tail);
        }
    }
    out
}

/// Every ordering of `items`.
fn orderings<'a>(items: &[&'a str]) -> Vec<Vec<&'a str>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut out = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let head = rest.remove(i);
        for mut tail in orderings(&rest) {
            tail.insert(0, head);
            out.push(tail);
        }
    }
    out
}

/// `a b WORD c WORD …`: the operands combined left to right, as a FOLD does.
fn chain(word: &str, operands: &[&str]) -> String {
    operands[1..]
        .iter()
        .fold(operands[0].to_string(), |code, x| {
            format!("{code} {x} {word}")
        })
}

/// Law 6 follows from law 4, and is the one a reader of exact arithmetic
/// leans on: where rounded arithmetic answers a sum differently in each
/// order, a sum or a product here is one value in every order — so a FOLD of
/// `ADD` or `MUL` may be reordered or regrouped without changing its answer,
/// even once it has met a point over zero. Every ordering of every
/// four-operand selection of the sample is compared, as a chain and, for
/// operands a Vector literal can hold, as a FOLD over that Vector.
#[tokio::test]
async fn a_sum_or_product_does_not_depend_on_operand_order() {
    for (word, unit) in [("ADD", "0"), ("MUL", "1")] {
        for selection in selections(SAMPLE, 4) {
            let all = orderings(&selection);
            let first = chain(word, &all[0]);
            let literal = selection.iter().all(|x| !x.contains(' '));
            for other in &all[1..] {
                let law = format!("{first}  {} EQ", chain(word, other));
                assert!(holds(&law).await, "{word} must not depend on order: {law}");
                if literal {
                    let fold = format!("[ {} ] {unit} [ {word} ] FOLD", other.join(" "));
                    let law = format!("{first}  {fold} EQ");
                    assert!(holds(&law).await, "FOLD must not depend on order: {law}");
                }
            }
        }
    }
}
