//! `Hash` must agree with `PartialEq`, or `UNIQUE` silently keeps duplicates.
//!
//! The collection-word de-quadraticization made `UNIQUE` / `TALLY` / `GROUP`
//! find an element's bucket by hashing it rather than by comparing it against
//! every distinct value found so far. That trade is only sound while
//! `a == b` implies `hash(a) == hash(b)`: a `HashMap` never compares keys that
//! landed in different buckets, so a violation does not raise — it answers a
//! *wrong value*, with two equal elements both surviving `UNIQUE`. In a
//! language whose whole claim is exactness that is the worst failure shape
//! available, and it is invisible to every test that only checks a limit
//! fired.
//!
//! `Value`'s equality is not structural, which is what makes this worth
//! pinning rather than assuming. Three separate relations have to be
//! respected, and each had to be built into the hash rather than derived:
//!
//!  * a rectangular numeric `Vector` equals the `Tensor` holding the same
//!    lanes (LANG.AUTHORITY.FREEDOM makes the representation unobservable);
//!  * two `Algebraic` values equal each other by *rebasing*, so `√12` built
//!    directly (basis `{12}`) equals `2·√3` (basis `{3}`) while sharing no
//!    structure;
//!  * a `Fraction` is held canonically, so equal values are equal pairs
//!    however they were built.

use crate::error::NilReason;
use crate::test_support::{as_exact, exact_mul, frac, hash_of, sqrt_of};
use crate::types::fraction::Fraction;
use crate::types::Value;
use num_bigint::BigInt;

/// The contract, as an assertion: equal values must hash equally.
fn assert_agrees(label: &str, left: &Value, right: &Value) {
    if left == right {
        assert_eq!(
            hash_of(left),
            hash_of(right),
            "`{label}`: these two values are EQUAL but hash differently, so a \
             HashMap would put them in different buckets and `UNIQUE` would \
             keep both. left={left:?} right={right:?}"
        );
    }
}

// ── the three relations that are not structural ────────────────────────

#[test]
fn an_algebraic_value_hashes_by_its_number_not_its_basis() {
    // `√12` keeps the coarse basis {12}; `2·√3` carries {3}. They are the
    // same real number and `==` says so by rebasing. Hashing the term map
    // would answer differently for each, which is precisely the trap the
    // interval-refinement hash key exists to avoid — and the reason a
    // cheap structural hash was not an option.
    let direct = sqrt_of(12);
    let built = exact_mul(&Value::from_int(2), &sqrt_of(3));
    assert_eq!(direct, built, "√12 and 2·√3 are the same number");
    assert_agrees("√12 vs 2·√3", &direct, &built);
}

#[test]
fn algebraic_sums_hash_by_their_number_too() {
    // The multi-term case, where the bases genuinely differ in
    // granularity: √8 + √18 = 2√2 + 3√2 = 5√2.
    let sum = {
        let a = as_exact(&sqrt_of(8));
        let b = as_exact(&sqrt_of(18));
        Value::from_exact_real(a.add(&b))
    };
    let direct = exact_mul(&Value::from_int(5), &sqrt_of(2));
    assert_eq!(sum, direct, "√8 + √18 = 5√2");
    assert_agrees("√8+√18 vs 5√2", &sum, &direct);
}

#[test]
fn a_rectangular_vector_hashes_like_the_tensor_holding_it() {
    // Whether small integers live as boxed children or dense lanes is an
    // optimization decision the language promises is unobservable. A hash
    // that could tell them apart would make it observable through
    // `UNIQUE`.
    let boxed = Value::from_vector(vec![
        Value::from_int(1),
        Value::from_int(2),
        Value::from_int(3),
    ]);
    let dense = Value::from_int_tensor(vec![1, 2, 3]);
    assert_eq!(boxed, dense, "same lanes, different storage");
    assert_agrees("boxed vector vs dense tensor", &boxed, &dense);
}

#[test]
fn a_nested_vector_hashes_like_the_rank_2_tensor_holding_it() {
    let nested = Value::from_vector(vec![
        Value::from_vector(vec![Value::from_int(1), Value::from_int(2)]),
        Value::from_vector(vec![Value::from_int(3), Value::from_int(4)]),
    ]);
    let dense = Value::from_tensor(
        vec![frac(1, 1), frac(2, 1), frac(3, 1), frac(4, 1)],
        vec![2, 2],
    );
    assert_eq!(nested, dense, "same lanes and shape, different storage");
    assert_agrees("nested vector vs rank-2 tensor", &nested, &dense);
}

#[test]
fn a_fraction_hashes_by_its_value_not_its_written_form() {
    // `==` cross-multiplies, so 2/4 equals 1/2 whether or not the gcd
    // normalizer ever ran on it.
    let reduced = Value::from_fraction(frac(1, 2));
    let unreduced = Value::from_fraction(Fraction::new(BigInt::from(2), BigInt::from(4)));
    assert_eq!(reduced, unreduced, "2/4 is 1/2");
    assert_agrees("1/2 vs 2/4", &reduced, &unreduced);
}

#[test]
fn a_small_fraction_hashes_like_the_big_one_holding_the_same_value() {
    // The `Small`/`Big` split is a storage decision. The hash has a fast
    // path for `Small` that must land on the same bytes the `Big` path
    // produces once it reduces back into machine-word range — the exact
    // seam that fast path introduced.
    let small = Value::from_fraction(frac(3, 7));
    let big_reducing_to_small = Value::from_fraction(Fraction::new(
        BigInt::from(3) * BigInt::from(1_000_000_007i64),
        BigInt::from(7) * BigInt::from(1_000_000_007i64),
    ));
    assert_eq!(small, big_reducing_to_small, "same rational");
    assert_agrees("Small 3/7 vs Big 3n/7n", &small, &big_reducing_to_small);
}

#[test]
fn a_negative_fraction_hashes_by_its_value() {
    let a = Value::from_fraction(frac(-1, 2));
    let b = Value::from_fraction(Fraction::new(BigInt::from(-5), BigInt::from(10)));
    assert_eq!(a, b, "-1/2 either way");
    assert_agrees("-1/2 vs -5/10", &a, &b);
}

// ── the relations that must NOT collapse ───────────────────────────────

#[test]
fn nils_with_different_reasons_stay_distinct() {
    // LANG.VALUES.NIL makes the reason the entire observable content of a
    // NIL, so two NILs are the same value exactly when their reasons
    // agree. `Value::hash` folds `nil_reason` in for this — without it,
    // `UNIQUE` over a vector of differently-caused NILs would collapse
    // them into one.
    let domain = Value::nil_with_reason(NilReason::DomainMiss);
    let missing = Value::nil_with_reason(NilReason::NotFound);
    assert_ne!(domain, missing, "different reasons are different values");
    assert_ne!(
        hash_of(&domain),
        hash_of(&missing),
        "distinct NIL reasons should not share a bucket needlessly"
    );
}

#[test]
fn a_string_does_not_hash_like_the_codepoint_vector() {
    // LANG.VALUES.DISJOINT: `'A'` is not `[ 65 ]`. They are unequal, so
    // nothing is *required* of their hashes — but sharing a bucket for
    // values from disjoint domains is a needless collision on the exact
    // axis `ValueData::Text` was introduced to separate.
    let text = Value::from_string("A");
    let codepoints = Value::from_vector(vec![Value::from_int(65)]);
    assert_ne!(text, codepoints, "disjoint domains");
    assert_ne!(hash_of(&text), hash_of(&codepoints));
}

#[test]
fn a_boolean_does_not_hash_like_the_number_beside_it() {
    // `TRUE 1 EQ` is false (LANG.VALUES.TRUTH), and the hash keeps them
    // apart rather than relying on the bucket comparison to notice.
    let truth = Value::from_bool(true);
    let one = Value::from_int(1);
    assert_ne!(truth, one, "a Boolean is not a number");
    assert_ne!(hash_of(&truth), hash_of(&one));
}

// ── the sweep ──────────────────────────────────────────────────────────

#[test]
fn every_pair_in_a_mixed_corpus_agrees() {
    // The targeted cases above each pin one relation. This one is the
    // catch-all: build a corpus that mixes the domains and the storage
    // choices, and assert the contract over every pair — including the
    // pairs nobody thought to name.
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
        Value::from_string("hello"),
        Value::nil_with_reason(NilReason::DomainMiss),
        Value::nil_with_reason(NilReason::NotFound),
        Value::from_fraction(Fraction::infinity()),
        Value::from_fraction(Fraction::infinity()),
        Value::from_fraction(Fraction::nullity()),
        Value::from_fraction(Fraction::new(BigInt::from(100), BigInt::from(0))),
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
        // A ragged vector, which has no rectangular shape and therefore
        // takes the structural hash path rather than the dense one.
        Value::from_vector(vec![
            Value::from_int(1),
            Value::from_vector(vec![Value::from_int(2), Value::from_int(3)]),
        ]),
        // A vector holding a non-numeric leaf, same reason.
        Value::from_vector(vec![Value::from_int(1), Value::from_string("x")]),
        // A vector of algebraic elements: the dense path must refuse it
        // (an ExactScalar is not a tensor lane) and the structural path
        // must still respect algebraic rebasing elementwise.
        Value::from_vector(vec![sqrt_of(12)]),
        Value::from_vector(vec![exact_mul(&Value::from_int(2), &sqrt_of(3))]),
    ];

    for (i, left) in corpus.iter().enumerate() {
        for (j, right) in corpus.iter().enumerate() {
            assert_agrees(&format!("corpus[{i}] vs corpus[{j}]"), left, right);
        }
    }
}

#[test]
fn a_hashmap_deduplicates_a_mixed_corpus_the_way_equality_does() {
    // The property stated as the Word that depends on it: inserting into
    // a `HashMap` must produce exactly the distinct count that pairwise
    // `==` produces. This is `UNIQUE`'s correctness, isolated from the
    // interpreter.
    let corpus: Vec<Value> = vec![
        Value::from_int(1),
        Value::from_fraction(frac(1, 1)),
        Value::from_fraction(Fraction::new(BigInt::from(3), BigInt::from(3))),
        sqrt_of(12),
        exact_mul(&Value::from_int(2), &sqrt_of(3)),
        Value::from_vector(vec![Value::from_int(1), Value::from_int(2)]),
        Value::from_int_tensor(vec![1, 2]),
        Value::from_string("A"),
    ];

    // Deliberately the O(n²) pairwise scan `UNIQUE` used to be: it is the
    // reference answer the hashed one has to match.
    let mut by_equality: Vec<&Value> = Vec::new();
    for value in &corpus {
        if !by_equality.contains(&value) {
            by_equality.push(value);
        }
    }

    let by_hash: std::collections::HashSet<&Value> = corpus.iter().collect();

    assert_eq!(
        by_hash.len(),
        by_equality.len(),
        "a HashMap must find exactly the distinct values pairwise equality \
         finds — {} by hash against {} by equality means `UNIQUE` would \
         report a different answer depending on which one it used",
        by_hash.len(),
        by_equality.len()
    );
}
