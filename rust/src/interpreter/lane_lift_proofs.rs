//! LANG.COLLECTIONS.LIFT's alignment rule, proved by bounded model checking.
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take. "Paired axes combine when
//! their lengths are equal, and when one of them is 1 that operand's single
//! lane is reused across the other's length": [`vector_extent`] is which
//! lengths set an extent, and [`common_extent`] is how the operands' extents
//! meet. The lift (`lift_lanes_dyn`) and the truth-column connectives
//! (`truth_ops`) both align by these two. The harness takes every sequence of
//! up to [`OPERANDS`] operands — `SELECT` has three — each a scalar or a
//! Vector of up to [`LEN`] lanes.

use super::{common_extent, vector_extent};

/// The most operands the harness aligns.
const OPERANDS: usize = 4;

/// The longest Vector the harness takes, 0 included.
const LEN: usize = 3;

/// A single lane sets no extent; every other length, empty included, does.
#[kani::proof]
fn one_lane_sets_no_extent() {
    let len: usize = kani::any();
    assert_eq!(vector_extent(len), if len == 1 { None } else { Some(len) });
}

/// The operands align when every extent set is the same: that is the
/// answer, `None` when none is set. Otherwise the refusal names the first
/// extent and the first that disagrees with it. Reversing the operands moves
/// neither whether they align nor the width.
#[kani::proof]
#[kani::unwind(6)]
fn common_extent_is_the_one_shared_width() {
    let extents: [Option<usize>; OPERANDS] = std::array::from_fn(|_| {
        let vector: bool = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= LEN);
        if vector {
            vector_extent(len)
        } else {
            None
        }
    });
    for count in 0..=OPERANDS {
        let extents = &extents[..count];
        let first = extents.iter().flatten().next().copied();
        let disagreeing = extents
            .iter()
            .flatten()
            .find(|&&e| Some(e) != first)
            .copied();
        let answer = common_extent(extents.iter().copied());
        match disagreeing {
            None => assert_eq!(answer, Ok(first)),
            Some(other) => assert_eq!(answer, Err((first.expect("set"), other))),
        }
        let reversed = common_extent(extents.iter().rev().copied());
        assert_eq!(reversed.is_ok(), answer.is_ok());
        if let Ok(width) = answer {
            assert_eq!(reversed, Ok(width));
        }
    }
}
