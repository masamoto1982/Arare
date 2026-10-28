//! LANG.COLLECTIONS.LIFT's broadcast, proved of the index algebra by bounded
//! model checking.
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take. Every element-wise binary
//! Word over two Tensors runs through these four functions: the shapes meet
//! in `broadcast_shape` (its shape, [`broadcast_dims`]; its error, the axis), each output lane is located with [`unravel_index`]
//! and [`ravel_index`] over [`compute_strides`], and each operand's lane is
//! found by [`project_broadcast_index`]. The harnesses take every rank up to
//! [`RANK`], each as a concrete case, and every length up to [`DIM`] on each
//! axis.
//!
//! The specification is NumPy's broadcasting rule, written from the right:
//! the `k`-th axis from the end of each shape meets the `k`-th of the other.

use super::{
    aligned_dim, broadcast_dims, compute_strides, project_broadcast_index, ravel_index,
    unravel_index,
};

/// The highest rank the harnesses take.
const RANK: usize = 2;

/// The longest axis the harnesses take, 0 included.
const DIM: usize = 3;

fn any_shape(rank: usize) -> Vec<usize> {
    (0..rank)
        .map(|_| {
            let d: usize = kani::any();
            kani::assume(d <= DIM);
            d
        })
        .collect()
}

/// The length `shape` has `k` axes from its end, 1 past its start.
fn from_end(shape: &[usize], k: usize) -> usize {
    if k < shape.len() {
        shape[shape.len() - 1 - k]
    } else {
        1
    }
}

/// `aligned_dim` is the right-aligned length.
#[kani::proof]
#[kani::unwind(4)]
fn aligned_dim_reads_from_the_right() {
    for len in 0..=RANK {
        let shape = any_shape(len);
        for rank in len..=RANK {
            for axis in 0..rank {
                assert_eq!(
                    aligned_dim(&shape, rank, axis),
                    from_end(&shape, rank - 1 - axis)
                );
            }
        }
    }
}

/// Two shapes broadcast exactly when every pair of axes that meet is equal or
/// holds a 1; the answer takes the length that is not 1 on each axis (an
/// empty axis against a 1 stays empty); a refusal names the first aligned
/// axis that does not meet; and the order of the operands changes nothing.
#[kani::proof]
// Comparing two Vecs is a `memcmp` over their bytes: 8 a lane.
#[kani::unwind(17)]
fn broadcast_dims_is_numpy_broadcasting() {
    for ra in 0..=RANK {
        for rb in 0..=RANK {
            let (a, b) = (any_shape(ra), any_shape(rb));
            let rank = ra.max(rb);
            let meets = |axis: usize| {
                let k = rank - 1 - axis;
                let (x, y) = (from_end(&a, k), from_end(&b, k));
                x == y || x == 1 || y == 1
            };
            match broadcast_dims(&a, &b) {
                Ok(out) => {
                    assert!((0..rank).all(meets));
                    assert_eq!(out.len(), rank);
                    for k in 0..rank {
                        let (x, y) = (from_end(&a, k), from_end(&b, k));
                        assert_eq!(from_end(&out, k), if x == 1 { y } else { x });
                    }
                    assert_eq!(broadcast_dims(&b, &a), Ok(out));
                }
                Err(axis) => {
                    assert!(axis < rank && !meets(axis));
                    assert!((0..axis).all(meets));
                    assert_eq!(broadcast_dims(&b, &a), Err(axis));
                }
            }
        }
    }
}

/// Every index into a broadcast's answer projects to an index into each
/// operand that is in bounds, reading a length-1 axis at 0 and every other
/// axis where the answer is.
#[kani::proof]
#[kani::unwind(4)]
fn projection_lands_in_the_operand() {
    for rank in 0..=RANK {
        for ra in 0..=rank {
            let input = any_shape(ra);
            let mut output = any_shape(rank);
            for k in 0..ra {
                // The answer's axis is the operand's, or anything over a 1.
                let i = rank - 1 - k;
                kani::assume(from_end(&input, k) == 1 || output[i] == from_end(&input, k));
            }
            let index: Vec<usize> = output
                .iter_mut()
                .map(|&mut d| {
                    let i: usize = kani::any();
                    kani::assume(i < d);
                    i
                })
                .collect();
            let projected = project_broadcast_index(&index, &output, &input);
            assert_eq!(projected.len(), ra);
            for k in 0..ra {
                let at = ra - 1 - k;
                let expected = if input[at] == 1 {
                    0
                } else {
                    index[rank - 1 - k]
                };
                assert_eq!(projected[at], expected);
                assert!(projected[at] < input[at]);
            }
        }
    }
}

/// Over row-major strides, `ravel_index` is a bijection from the indices in
/// bounds onto `0..` the lane count, and `unravel_index` is its inverse.
#[kani::proof]
// Comparing two Vecs is a `memcmp` over their bytes: 8 a lane.
#[kani::unwind(17)]
fn ravel_and_unravel_are_inverse() {
    for rank in 0..=RANK {
        let shape = any_shape(rank);
        let strides = compute_strides(&shape);
        let lanes: usize = shape.iter().product();
        let index: Vec<usize> = shape
            .iter()
            .map(|&d| {
                let i: usize = kani::any();
                kani::assume(i < d);
                i
            })
            .collect();
        let linear = ravel_index(&index, &strides);
        if rank > 0 {
            assert!(linear < lanes);
        }
        assert_eq!(unravel_index(linear, &shape, &strides), index);
        let any_linear: usize = kani::any();
        kani::assume(any_linear < lanes);
        if rank > 0 {
            let back = unravel_index(any_linear, &shape, &strides);
            assert!(back.iter().zip(&shape).all(|(&i, &d)| i < d));
            assert_eq!(ravel_index(&back, &strides), any_linear);
        }
    }
}
