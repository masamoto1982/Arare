//! Widths (AR-LLR-002, refining AR-HLR-001 and AR-HLR-003).

/// A number's width. 8, 16 and 32 are storage widths; 64 is the widest width
/// arithmetic widens into (ADR-0001).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Width {
    W8,
    W16,
    W32,
    W64,
}

impl Width {
    pub const ALL: [Width; 4] = [Width::W8, Width::W16, Width::W32, Width::W64];

    pub const fn bits(self) -> u32 {
        match self {
            Width::W8 => 8,
            Width::W16 => 16,
            Width::W32 => 32,
            Width::W64 => 64,
        }
    }

    /// The half-range `M = 2^(N-1)`, as `i128` because `2^63` exceeds `i64`.
    pub const fn half_range(self) -> i128 {
        1i128 << (self.bits() - 1)
    }

    /// The largest finite code, `M − 2`; the smallest is its negation.
    pub const fn max_finite(self) -> i64 {
        (self.half_range() - 2) as i64
    }

    /// The code of `1/0`, `M − 1`.
    pub const fn inf_code(self) -> i64 {
        (self.half_range() - 1) as i64
    }

    /// The code of `0/0`, `−M`.
    pub const fn bot_code(self) -> i64 {
        (-self.half_range()) as i64
    }

    /// The narrowest width whose finite range holds every integer of
    /// magnitude at most `bound`; the widest width when none does, where a
    /// result that does not fit becomes `1/0` at run time (AR-HLR-004).
    pub fn narrowest_for(bound: u128) -> Width {
        for w in Width::ALL {
            if bound <= w.max_finite() as u128 {
                return w;
            }
        }
        Width::W64
    }
}
