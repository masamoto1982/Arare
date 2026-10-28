//! The truth column's packing rule (LANG.VALUES.TRUTH), proved by bounded
//! model checking.
//!
//! `cargo kani` (docs/quality/FORMAL_VERIFICATION.md) checks each harness for
//! every input its `kani::any()` values can take. A column holds a flat
//! Vector of truth values a byte a lane, every UNKNOWN lane being one NIL kept
//! beside the bytes, so packing is lossless only if it refuses exactly what
//! one NIL cannot stand for. [`pack_codes`] is that rule over plain lanes —
//! [`TruthColumn::pack`](super::TruthColumn::pack) hands it each lane's
//! domain and, for a NIL, its absence metadata — and is proved on every
//! sequence of up to [`LANES`] lanes, each length a concrete case, with NILs
//! drawn from [`KEYS`] different absences.

use super::{pack_codes, PackLane, FALSE, TRUE, UNKNOWN};

/// The longest sequence the harness takes.
const LANES: usize = 4;

/// How many different NILs a lane can be.
const KEYS: u8 = 2;

/// A lane: 0 and 1 the truth values, 2 and 3 a NIL of one key or the other,
/// 4 anything else.
fn any_lane() -> u8 {
    let lane: u8 = kani::any();
    kani::assume(lane <= 2 + KEYS);
    lane
}

fn lane_of(lane: u8) -> PackLane<u8> {
    match lane {
        0 => PackLane::Truth(false),
        1 => PackLane::Truth(true),
        l if l < 2 + KEYS => PackLane::Nil(l - 2),
        _ => PackLane::Other,
    }
}

/// `pack_codes` answers exactly when every lane is a truth value or a NIL and
/// every NIL is the same one; its codes are the lanes', each NIL UNKNOWN; and
/// it names the first NIL, the one the column keeps, when there is one.
#[kani::proof]
#[kani::unwind(6)]
fn pack_codes_is_lossless_or_refuses() {
    let lanes: [u8; LANES] = [any_lane(), any_lane(), any_lane(), any_lane()];
    for len in 0..=LANES {
        let lanes = &lanes[..len];
        let first_nil = lanes.iter().position(|&l| (2..2 + KEYS).contains(&l));
        let packable = lanes.iter().all(|&l| {
            l < 2 || ((2..2 + KEYS).contains(&l) && Some(l) == first_nil.map(|i| lanes[i]))
        });
        match pack_codes(lanes.iter().map(|&l| lane_of(l))) {
            None => assert!(!packable),
            Some((codes, nil)) => {
                assert!(packable);
                assert_eq!(nil, first_nil);
                assert_eq!(codes.len(), len);
                for (i, &l) in lanes.iter().enumerate() {
                    let expected = match l {
                        0 => FALSE,
                        1 => TRUE,
                        _ => UNKNOWN,
                    };
                    assert_eq!(codes[i], expected);
                }
            }
        }
    }
}
