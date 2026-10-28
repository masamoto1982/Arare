//! Truth columns in Apache Arrow's layout (AR-LLR-009, refining AR-HLR-019
//! and AR-HLR-012).
//!
//! A column of `len` lanes holds two bitmaps: validity (a cleared bit is a NIL
//! lane, read as UNKNOWN) and values. Lane `i` is bit `i % 8` of byte `i / 8`
//! in each, least significant bit first, as Arrow numbers bits. The bitmaps
//! are stored as little-endian `u64` words, which is that byte order on every
//! host once written out by [`TruthColumn::validity_bytes`]; a buffer is padded
//! to a multiple of 64 bytes with zero bits, as Arrow recommends, and a value
//! bit is never set in a NIL lane.

use crate::truth::Truth;
use alloc::vec;
use alloc::vec::Vec;

/// Words per 64-byte block.
const BLOCK_WORDS: usize = 8;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TruthColumn {
    len: usize,
    validity: Vec<u64>,
    values: Vec<u64>,
}

fn words_for(len: usize) -> usize {
    len.div_ceil(64).div_ceil(BLOCK_WORDS) * BLOCK_WORDS
}

/// Kleene AND of 64 lanes at once: a lane is known when both are known or
/// either is a known FALSE, and true when both are known TRUE.
pub(crate) fn and_word(va: u64, xa: u64, vb: u64, xb: u64) -> (u64, u64) {
    let false_a = va & !xa;
    let false_b = vb & !xb;
    let valid = (va & vb) | false_a | false_b;
    (valid, va & xa & vb & xb)
}

/// Kleene NOT of 64 lanes: knowledge stays, a known value flips.
pub(crate) fn not_word(v: u64, x: u64) -> (u64, u64) {
    (v, v & !x)
}

impl TruthColumn {
    pub fn from_lanes(lanes: &[Truth]) -> TruthColumn {
        let n = words_for(lanes.len());
        let (mut validity, mut values) = (vec![0u64; n], vec![0u64; n]);
        for (i, t) in lanes.iter().enumerate() {
            let bit = 1u64 << (i % 64);
            match t {
                Truth::Unknown => {}
                Truth::False => validity[i / 64] |= bit,
                Truth::True => {
                    validity[i / 64] |= bit;
                    values[i / 64] |= bit;
                }
            }
        }
        TruthColumn {
            len: lanes.len(),
            validity,
            values,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn lane(&self, i: usize) -> Truth {
        assert!(i < self.len, "lane {i} of a column of {}", self.len);
        let bit = 1u64 << (i % 64);
        match (
            self.validity[i / 64] & bit != 0,
            self.values[i / 64] & bit != 0,
        ) {
            (false, _) => Truth::Unknown,
            (true, false) => Truth::False,
            (true, true) => Truth::True,
        }
    }

    /// Lane-wise Kleene AND; `None` when the lengths differ.
    pub fn and(&self, other: &TruthColumn) -> Option<TruthColumn> {
        if self.len != other.len {
            return None;
        }
        let mut out = TruthColumn {
            len: self.len,
            validity: self.validity.clone(),
            values: self.values.clone(),
        };
        for w in 0..out.validity.len() {
            let (v, x) = and_word(
                self.validity[w],
                self.values[w],
                other.validity[w],
                other.values[w],
            );
            out.validity[w] = v;
            out.values[w] = x;
        }
        Some(out)
    }

    /// Lane-wise Kleene NOT.
    pub fn not(&self) -> TruthColumn {
        let mut out = self.clone();
        for w in 0..out.validity.len() {
            let (v, x) = not_word(self.validity[w], self.values[w]);
            out.validity[w] = v;
            out.values[w] = x;
        }
        out
    }

    /// The validity bitmap as Arrow's bytes.
    pub fn validity_bytes(&self) -> Vec<u8> {
        self.validity.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    /// The value bitmap as Arrow's bytes.
    pub fn values_bytes(&self) -> Vec<u8> {
        self.values.iter().flat_map(|w| w.to_le_bytes()).collect()
    }
}
