//! Scalar strong Kleene truth (AR-LLR-008, refining AR-HLR-012 and
//! AR-HLR-013). Implemented on the truth order's ranks rather than as a
//! table, so it is a second statement of the law the specification's tables
//! state.

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Truth {
    False,
    Unknown,
    True,
}

impl Truth {
    pub const ALL: [Truth; 3] = [Truth::False, Truth::Unknown, Truth::True];

    const fn rank(self) -> u8 {
        match self {
            Truth::False => 0,
            Truth::Unknown => 1,
            Truth::True => 2,
        }
    }

    const fn from_rank(r: u8) -> Truth {
        match r {
            0 => Truth::False,
            1 => Truth::Unknown,
            _ => Truth::True,
        }
    }

    /// AND is the meet of the truth order.
    pub fn and(self, other: Truth) -> Truth {
        Truth::from_rank(if self.rank() < other.rank() {
            self.rank()
        } else {
            other.rank()
        })
    }

    /// NOT reverses the truth order.
    pub fn not(self) -> Truth {
        Truth::from_rank(2 - self.rank())
    }

    /// OR, written from AND and NOT as Ajisai writes it.
    pub fn or(self, other: Truth) -> Truth {
        self.not().and(other.not()).not()
    }

    /// A NIL read in truth position is UNKNOWN (AR-HLR-013).
    pub fn of(t: Option<Truth>) -> Truth {
        match t {
            Some(t) => t,
            None => Truth::Unknown,
        }
    }
}
