//! The value digest's laws: equal values digest alike however they were
//! built, and distinct values (a NIL's reason included) apart.

use super::value_digest;
use crate::error::NilReason;
use crate::test_support::{exact_add, exact_mul, frac, sqrt_of};
use crate::types::exact::ExactReal;
use crate::types::fraction::Fraction;
use crate::types::Value;
use num_bigint::BigInt;

fn digest_of(value: &Value) -> String {
    value_digest(value)
}

/// The central invariant: `a == b` must imply `encode(a) ==
/// encode(b)`, over a corpus mixing every relation the digest has to
/// respect (algebraic rebasing, Vector/Tensor cross-equality, Fraction
/// reduction) with the ones it must not collapse (NIL reasons, domain
/// disjointness).
#[test]
fn equal_values_digest_equally() {
    let corpus: Vec<Value> = vec![
        Value::from_int(0),
        Value::from_int(1),
        Value::from_int(-1),
        Value::from_fraction(frac(1, 2)),
        Value::from_fraction(Fraction::new(BigInt::from(2), BigInt::from(4))),
        Value::from_fraction(frac(-3, 7)),
        Value::from_bool(true),
        Value::from_bool(false),
        Value::from_string(""),
        Value::from_string("A"),
        Value::nil_with_reason(NilReason::DomainMiss),
        Value::nil_with_reason(NilReason::NotFound),
        Value::from_fraction(Fraction::infinity()),
        Value::from_fraction(Fraction::nullity()),
        sqrt_of(2),
        sqrt_of(3),
        sqrt_of(12),
        exact_mul(&Value::from_int(2), &sqrt_of(3)),
        Value::from_vector(Vec::new()),
        Value::from_vector(vec![Value::from_int(65)]),
        Value::from_vector(vec![
            Value::from_int(1),
            Value::from_int(2),
            Value::from_int(3),
        ]),
        Value::from_int_tensor(vec![1, 2, 3]),
        Value::from_int_tensor(vec![1, 2, 4]),
        Value::from_vector(vec![
            Value::from_vector(vec![Value::from_int(1), Value::from_int(2)]),
            Value::from_vector(vec![Value::from_int(3), Value::from_int(4)]),
        ]),
        Value::from_tensor(
            vec![frac(1, 1), frac(2, 1), frac(3, 1), frac(4, 1)],
            vec![2, 2],
        ),
        Value::from_vector(vec![
            Value::from_int(1),
            Value::from_vector(vec![Value::from_int(2), Value::from_int(3)]),
        ]),
        Value::from_vector(vec![Value::from_int(1), Value::from_string("x")]),
        Value::from_vector(vec![sqrt_of(12)]),
        Value::from_vector(vec![exact_mul(&Value::from_int(2), &sqrt_of(3))]),
    ];

    for (i, left) in corpus.iter().enumerate() {
        for (j, right) in corpus.iter().enumerate() {
            if left == right {
                assert_eq!(
                    digest_of(left),
                    digest_of(right),
                    "corpus[{i}] == corpus[{j}] but digested differently: \
                     left={left:?} right={right:?}"
                );
            }
        }
    }
}

/// A rectangular numeric `Vector` and the `Tensor` holding the same lanes
/// are the same value (pitfall B); nested vs rank-2 too.
#[test]
fn tensor_and_nested_vector_digest_equally() {
    let boxed = Value::from_vector(vec![
        Value::from_int(1),
        Value::from_int(2),
        Value::from_int(3),
    ]);
    let dense = Value::from_int_tensor(vec![1, 2, 3]);
    assert_eq!(digest_of(&boxed), digest_of(&dense));

    let nested = Value::from_vector(vec![
        Value::from_vector(vec![Value::from_int(1), Value::from_int(2)]),
        Value::from_vector(vec![Value::from_int(3), Value::from_int(4)]),
    ]);
    let rank2 = Value::from_tensor(
        vec![frac(1, 1), frac(2, 1), frac(3, 1), frac(4, 1)],
        vec![2, 2],
    );
    assert_eq!(digest_of(&nested), digest_of(&rank2));
}

/// The NIL reason *is* meaning
/// (LANG.VALUES.NIL), so two differently-caused NILs must digest apart.
#[test]
fn nil_reasons_separate_digests() {
    let domain = Value::nil_with_reason(NilReason::DomainMiss);
    let missing = Value::nil_with_reason(NilReason::NotFound);
    assert_ne!(domain, missing);
    assert_ne!(digest_of(&domain), digest_of(&missing));
}

/// Two ABSENT NILs are the same value exactly when their Texts are equal
/// (LANG.VALUES.NIL), so the Text is part of the digest too — as it is
/// of `Value::hash` and `PartialEq`.
#[test]
fn absent_texts_separate_digests() {
    let a = Value::nil_user_declared("a");
    let b = Value::nil_user_declared("b");
    let a_again = Value::nil_user_declared("a");
    assert_ne!(a, b);
    assert_ne!(digest_of(&a), digest_of(&b));
    assert_eq!(digest_of(&a), digest_of(&a_again));
}

/// Regression: keying the algebraic digest at a much larger precision
/// than `HASH_KEY_BITS` made a 256-term value (reachable in practice —
/// `max_algebraic_terms` allows up to 512) cost tens of milliseconds per
/// value in a release build and hundreds in a debug one; multiplied
/// across a stack of them, enough to time out an unrelated CI job. This
/// pins the *budget*, not the mechanism, so a future precision change
/// remains free as long as it stays cheap.
#[test]
fn many_term_algebraic_digest_stays_fast() {
    let pairs = [
        (2, 3),
        (5, 7),
        (11, 13),
        (17, 19),
        (23, 29),
        (31, 37),
        (41, 43),
        (47, 53),
    ];
    let mut factors = pairs
        .into_iter()
        .map(|(p, q)| exact_add(&sqrt_of(p), &sqrt_of(q)));
    let mut value = factors.next().expect("at least one factor");
    for factor in factors {
        value = exact_mul(&value, &factor);
    }
    let terms = match &value.data {
        crate::types::ValueData::ExactScalar(exact) => match &**exact {
            ExactReal::Algebraic(alg) => alg.normal_form_terms().len(),
            other => panic!("expected an algebraic scalar: {other:?}"),
        },
        other => panic!("expected an algebraic scalar: {other:?}"),
    };
    assert!(terms > 100, "expected a many-term value, got {terms} terms");

    let start = std::time::Instant::now();
    let digest = digest_of(&value);
    let elapsed = start.elapsed();
    assert!(digest.starts_with('#'));
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "digesting a {terms}-term algebraic value took {elapsed:?} (debug build); \
         expected well under 2s"
    );
}
