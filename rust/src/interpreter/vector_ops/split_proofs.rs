//! The split `TAKE` and `DROP` share, proved by bounded model checking.
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take: here every `i64` count,
//! `i64::MIN` included, against every length a Vector can have.

use super::{compute_drop_bounds, compute_take_bounds, Split};

/// A count addresses `|count|` elements from the front, or from the back when
/// it is negative, and is past the end — both Words projecting — exactly when
/// `|count|` exceeds the length. Otherwise `TAKE` and `DROP` answer slices in
/// bounds that are each other's complement, and the elements priced are the
/// elements copied.
#[kani::proof]
fn take_and_drop_split_the_vector_in_two() {
    let len: usize = kani::any();
    let count: i64 = kani::any();
    // An in-memory Vector is never longer than an allocation can be.
    kani::assume(len <= isize::MAX as usize);
    let magnitude = u128::from(count.unsigned_abs());
    let take = compute_take_bounds(len, count);
    let drop = compute_drop_bounds(len, count);
    if magnitude > len as u128 {
        assert!(take.is_none() && drop.is_none());
        return;
    }
    let ((t0, t1), (d0, d1)) = (take.expect("in range"), drop.expect("in range"));
    assert!(t0 <= t1 && t1 <= len && d0 <= d1 && d1 <= len);
    assert_eq!((t1 - t0) as u128, magnitude);
    // The front for a count of zero or more, the back for a negative one.
    assert_eq!(
        if count < 0 { t1 } else { t0 },
        if count < 0 { len } else { 0 }
    );
    // Complements: one ends where the other starts, and together they cover
    // the Vector.
    if count < 0 {
        assert!(d0 == 0 && d1 == t0);
    } else {
        assert!(d0 == t1 && d1 == len);
    }
    let wanted = magnitude as usize;
    assert_eq!(Split::Take.copied(len, wanted), t1 - t0);
    assert_eq!(Split::Drop.copied(len, wanted), d1 - d0);
}
