//! AR-LLR-001. Mirrors `formal/Arare/Truth.lean` (AR-HLR-012).

use core::fmt;

/// Strong Kleene truth, ordered `False < Unknown < True`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Truth {
    False,
    Unknown,
    True,
}

impl Truth {
    pub const ALL: [Truth; 3] = [Truth::False, Truth::Unknown, Truth::True];

    pub fn rank(self) -> u8 {
        match self {
            Truth::False => 0,
            Truth::Unknown => 1,
            Truth::True => 2,
        }
    }

    pub fn and(self, other: Truth) -> Truth {
        match (self, other) {
            (Truth::False, _) | (_, Truth::False) => Truth::False,
            (Truth::Unknown, _) | (_, Truth::Unknown) => Truth::Unknown,
            (Truth::True, Truth::True) => Truth::True,
        }
    }

    pub fn not(self) -> Truth {
        match self {
            Truth::False => Truth::True,
            Truth::Unknown => Truth::Unknown,
            Truth::True => Truth::False,
        }
    }

    pub fn or(self, other: Truth) -> Truth {
        self.not().and(other.not()).not()
    }
}

impl fmt::Display for Truth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Truth::False => "F",
            Truth::Unknown => "U",
            Truth::True => "T",
        })
    }
}
