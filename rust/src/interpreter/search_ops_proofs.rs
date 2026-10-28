//! `BSEARCH`'s binary search (LANG.VALUES.ORDER), proved by bounded model
//! checking.
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take. `lower_bound` asks
//! [`partition_point`] where the key would stand among the present elements,
//! then reads the one element there: so the search finds a key exactly when
//! the halving lands on its first occurrence. The harness takes every
//! ascending sequence of up to [`LEN`] elements, each length a concrete case,
//! with duplicates, and every key.

use super::partition_point;
use std::cmp::Ordering;

/// The longest sequence the harness searches.
const LEN: usize = 5;

/// Over an ascending sequence, `partition_point` answers how many elements
/// lie below the key — so the element there, when there is one, is the key's
/// first occurrence or greater than it — asks only about positions in the
/// sequence, and asks no more than ⌊log₂ n⌋ + 1 times.
#[kani::proof]
#[kani::unwind(7)]
fn partition_point_is_the_count_below_the_key() {
    let elements: [u8; LEN] = kani::any();
    let key: u8 = kani::any();
    for len in 0..=LEN {
        let elements = &elements[..len];
        kani::assume(elements.windows(2).all(|pair| pair[0] <= pair[1]));
        let mut asked = 0u32;
        let answer = partition_point(len, |at| {
            assert!(at < len);
            asked += 1;
            Ok::<_, ()>(Some(elements[at].cmp(&key)))
        });
        let below = elements.iter().filter(|&&e| e < key).count();
        assert_eq!(answer, Ok(Some(below)));
        assert!(asked <= usize::BITS - len.leading_zeros());
        // So reading the element there decides the search.
        let found = elements.get(below).is_some_and(|&e| e == key);
        assert_eq!(found, elements.contains(&key));
    }
}

/// A pair the order cannot place stops the search with no answer, and an
/// error from the comparison is the search's.
#[kani::proof]
#[kani::unwind(7)]
fn an_unordered_pair_or_an_error_stops_the_search() {
    let len: usize = kani::any();
    kani::assume(len <= LEN);
    let at_fault: usize = kani::any();
    let fails: bool = kani::any();
    let answer = partition_point(len, |at| {
        if at == at_fault {
            if fails {
                Err(at)
            } else {
                Ok(None)
            }
        } else {
            Ok(Some(if at < at_fault {
                Ordering::Less
            } else {
                Ordering::Greater
            }))
        }
    });
    if at_fault < len {
        assert_eq!(answer, if fails { Err(at_fault) } else { Ok(None) });
    } else {
        assert_eq!(answer, Ok(Some(len)));
    }
}
