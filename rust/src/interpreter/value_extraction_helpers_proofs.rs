//! The index rule `GET`, `PUT` and the other positional Words read through
//! (`normalize_index`), proved by bounded model checking.
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take: here every `i64` index and
//! every length a Vector can have.

use super::normalize_index;

/// A non-negative index counts from the front and a negative one from the
/// back (`-1` the last element); the answer is a position inside the Vector
/// exactly when the index names one, for every index and every length — the
/// narrowing to `usize` loses nothing. Kani runs on the 64-bit host, so this
/// is proved for a 64-bit `usize`; wasm32's rests on the same code.
#[kani::proof]
fn normalize_index_counts_from_either_end() {
    let index: i64 = kani::any();
    let length: usize = kani::any();
    // An in-memory Vector is never longer than an allocation can be.
    kani::assume(length <= isize::MAX as usize);
    let (index_wide, length_wide) = (i128::from(index), length as i128);
    let position = if index < 0 {
        index_wide + length_wide
    } else {
        index_wide
    };
    let expected = (0 <= position && position < length_wide).then(|| position as usize);
    assert_eq!(normalize_index(index, length), expected);
}
