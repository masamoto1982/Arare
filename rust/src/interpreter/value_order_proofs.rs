//! LANG.VALUES.ORDER's placement rule, proved by bounded model checking.
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take. `SORT` and `ORDER` answer
//! the permutation [`nulls_last_order`] builds, so its law is theirs: every
//! present element first, ascending and stable, then every NIL in its own
//! place — SQL's `NULLS LAST`, so nothing is dropped and a payload permuted by
//! the answer stays aligned.
//!
//! The harness takes every Vector of up to [`LANES`] elements, every pattern
//! of NILs among them and every order on the present ones, ties included (a
//! key per element from as many values as there are lanes, so every strict
//! order occurs as well as every pattern of ties). What the
//! comparator itself answers for two numbers is `small_rational::order` on
//! machine-word pairs, proved over every `i64` in `small_rational_proofs`; a
//! `Value` is too wide a type for the solver to take whole, so the order
//! between Values is held by the property and conformance tests.

use super::nulls_last_order;

/// The longest Vector the harness takes.
const LANES: usize = 4;

/// `nulls_last_order` is a permutation of `0..len` with the present indices
/// first, ascending by `cmp` and in source order on ties, and the NILs after
/// them in source order.
///
/// Every length up to [`LANES`] and every pattern of NILs is run in turn, as a
/// concrete case: a symbolic length would hand the solver every branch the
/// standard sort has for long slices. The keys stay symbolic, so each case is
/// proved for every order on its present elements.
#[kani::proof]
#[kani::unwind(33)]
fn nulls_last_order_is_the_stable_nulls_last_sort() {
    let keys: [u8; LANES] = kani::any();
    kani::assume(keys.iter().all(|&k| usize::from(k) < LANES));
    for len in 0..=LANES {
        for mask in 0..1u8 << len {
            let nil: [bool; LANES] = std::array::from_fn(|i| mask >> i & 1 == 1);
            check_case(len, &nil, &keys);
        }
    }
}

fn check_case(len: usize, nil: &[bool; LANES], keys: &[u8; LANES]) {
    let perm = nulls_last_order(len, |i| nil[i], |i, j| keys[i].cmp(&keys[j]));

    // A permutation of 0..len.
    assert_eq!(perm.len(), len);
    let mut seen = [false; LANES];
    for &index in &perm {
        assert!(index < len && !seen[index]);
        seen[index] = true;
    }
    // The present indices, then the NILs.
    let present = (0..len).filter(|&i| !nil[i]).count();
    for (at, &index) in perm.iter().enumerate() {
        assert_eq!(nil[index], at >= present);
    }
    // Each run in the order the law says.
    for pair in perm.windows(2) {
        let (i, j) = (pair[0], pair[1]);
        if nil[i] || nil[j] {
            if nil[i] && nil[j] {
                assert!(i < j, "the NILs keep their own order");
            }
            continue;
        }
        assert!(keys[i] <= keys[j], "the present elements ascend");
        if keys[i] == keys[j] {
            assert!(i < j, "equal elements keep their order");
        }
    }
}
