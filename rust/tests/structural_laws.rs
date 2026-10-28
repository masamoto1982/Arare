//! Structural data (vector / tensor) algebraic laws (executable).
//!
//! The vector vocabulary of LANG.COLLECTIONS.LIFT is a free monoid under `CONCAT` with an
//! involutive `REVERSE`, and the shape vocabulary of LANG.COLLECTIONS.LIFT reads and
//! rewrites the index structure — `SHAPE` reads it, `RESHAPE` round-trips
//! through it, `FLATTEN` collapses it and `DEPTH` measures it. `SORT` (canonical home `ALGO`, LANG.DICTIONARY.RESOLUTION) is idempotent and
//! permutation-invariant on the decidable rational sub-domain (LANG.VALUES.TRUTH).
//!
//! Observation matches the conformance runner: whole-stack `Value::to_string`.

use ajisai_core::interpreter::Interpreter;
use proptest::prelude::*;

fn eval(src: &str) -> String {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio current-thread runtime");
    rt.block_on(async {
        let mut interp = Interpreter::new();
        interp
            .execute(src)
            .await
            .unwrap_or_else(|e| panic!("program failed: {src:?}: {e}"));
        interp
            .get_stack()
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    })
}

fn assert_law(name: &str, lhs: &str, rhs: &str) {
    let l = eval(lhs);
    let r = eval(rhs);
    assert_eq!(
        l, r,
        "law `{name}` broken:\n  {lhs:?} => {l}\n  {rhs:?} => {r}"
    );
}

fn small() -> impl Strategy<Value = i64> {
    -50i64..=50
}
fn vlit(xs: &[i64]) -> String {
    let body = xs
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    format!("[ {body} ]")
}
fn vec_ne() -> impl Strategy<Value = Vec<i64>> {
    prop::collection::vec(small(), 1..=6)
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// `REVERSE` is an involution: `REVERSE ∘ REVERSE = id`.
    #[test]
    fn reverse_is_involution(xs in vec_ne()) {
        let v = vlit(&xs);
        assert_law("reverse-involution", &format!("{v} REVERSE REVERSE"), &v);
    }

    /// `TAKE n` of the whole length is the identity.
    #[test]
    fn take_full_is_identity(xs in vec_ne()) {
        let v = vlit(&xs);
        let n = xs.len();
        assert_law("take-full", &format!("{v} {n} TAKE"), &v);
    }

    /// `SHAPE` of a flat Vector is its length; `RESHAPE` through that shape is
    /// the identity; `FLATTEN` of a flat Vector is itself and has depth 1.
    #[test]
    fn shape_words_agree_on_a_flat_vector(xs in vec_ne()) {
        let v = vlit(&xs);
        let n = xs.len();
        assert_law("shape-is-length", &format!("{v} SHAPE"), &format!("[ {n} ]"));
        assert_law("reshape-through-own-shape", &format!("{v} {v} SHAPE RESHAPE"), &v);
        assert_law("flatten-flat", &format!("{v} FLATTEN"), &v);
        assert_law("depth-flat", &format!("{v} DEPTH"), "1");
    }

    /// Nesting a Vector inside another raises its depth by one, prefixes its
    /// shape with 1, and leaves its leaves — so FLATTEN undoes the nesting.
    #[test]
    fn nesting_adds_one_axis(xs in vec_ne()) {
        let v = vlit(&xs);
        let n = xs.len();
        assert_law("depth-nested", &format!("[ {v} ] DEPTH"), "2");
        assert_law("shape-nested", &format!("[ {v} ] SHAPE"), &format!("[ 1 {n} ]"));
        assert_law("flatten-nested", &format!("[ {v} ] FLATTEN"), &v);
        assert_law("reshape-nested", &format!("{v} [ 1 {n} ] RESHAPE"), &format!("[ {v} ]"));
    }

    /// `DROP 0` is the identity, and `DROP n` of the whole length is empty.
    #[test]
    fn drop_none_is_identity_and_drop_all_is_empty(xs in vec_ne()) {
        let v = vlit(&xs);
        let n = xs.len();
        assert_law("drop-none", &format!("{v} 0 DROP"), &v);
        assert_law("drop-all", &format!("{v} {n} DROP"), "[ ]");
    }

    /// `TAKE k` and `DROP k` are the two halves of one cut: joined back with
    /// `CONCAT` they give the Vector they were cut from, for every `k` in
    /// range and from either end.
    #[test]
    fn take_and_drop_partition_the_vector(xs in vec_ne(), k in 0usize..=6) {
        let v = vlit(&xs);
        let k = k.min(xs.len());
        assert_law(
            "take-drop-partition",
            &format!("{v} {k} TAKE {v} {k} DROP CONCAT"),
            &v,
        );
        assert_law(
            "drop-take-partition-from-the-end",
            &format!("{v} -{k} DROP {v} -{k} TAKE CONCAT"),
            &v,
        );
    }

}

// ── Free-monoid laws of CONCAT / REVERSE (fixed operands) ──

#[test]
fn concat_is_associative() {
    // (a ++ b) ++ c == a ++ (b ++ c).
    //
    // Both sides used to be written with the count-prefixed `n CONCAT`, which
    // let the right-hand side be the single join `a b c 3 CONCAT` — a form the
    // specification never declared, and one that stated associativity by
    // assuming it. Two nested binary joins is the law itself, and `CONCAT` is
    // now only the declared `2 -> 1`.
    assert_law(
        "concat-assoc",
        "[ 1 2 ] [ 3 4 ] CONCAT [ 5 6 ] CONCAT",
        "[ 1 2 ] [ 3 4 ] [ 5 6 ] CONCAT CONCAT",
    );
}

#[test]
fn reverse_is_anti_homomorphism() {
    // reverse(a ++ b) == reverse(b) ++ reverse(a).
    assert_law(
        "reverse-concat",
        "[ 1 2 3 ] [ 4 5 ] CONCAT REVERSE",
        "[ 4 5 ] REVERSE [ 1 2 3 ] REVERSE CONCAT",
    );
}

// ── SORT (ALGO) on the decidable rational sub-domain (LANG.VALUES.TRUTH) ──

#[test]
fn sort_is_idempotent_and_permutation_invariant() {
    assert_law("sort-idempotent", "[ 3 1 2 ] SORT SORT", "[ 3 1 2 ] SORT");
    // Sorting is invariant under any prior permutation of the input.
    assert_law(
        "sort-permutation-invariant",
        "[ 3 1 2 ] SORT",
        "[ 3 1 2 ] REVERSE SORT",
    );
    assert_law("sort-rationals", "[ 3 1 2 ] SORT", "[ 1 2 3 ]");
}

/// `GET` with several indices is the selection each index makes, in the order
/// they are written — the gather law. Stated against `COLLECT` of the
/// single-index selections, which is the program a reader had to write before
/// the index operand accepted more than one position.
#[test]
fn get_gathers_in_the_order_its_indices_name() {
    assert_law(
        "gather = collect of selections",
        "[ 10 20 30 40 ] [ 2 0 3 ] GET",
        "[ 10 20 30 40 ] 2 GET [ 10 20 30 40 ] 0 GET \
         [ 10 20 30 40 ] 3 GET 3 COLLECT",
    );
    // Selecting every position in order is the vector itself, so a gather can
    // express the identity permutation.
    assert_law(
        "gather of the identity permutation",
        "[ 10 20 30 ] [ 0 1 2 ] GET",
        "[ 10 20 30 ]",
    );
    // A single index still answers with the element, not a one-element vector:
    // the generalization does not move the existing case.
    assert_law("one index selects a value", "[ 10 20 30 ] 1 GET", "20");
    // Reversal is a gather, which is the point of allowing one.
    assert_law(
        "gather can reverse",
        "[ 10 20 30 ] [ -1 -2 -3 ] GET",
        "[ 10 20 30 ] REVERSE",
    );
}
