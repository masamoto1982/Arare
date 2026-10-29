//! The reference interpreter (AR-LLR-013 to AR-LLR-016, refining AR-HLR-020
//! to AR-HLR-025).
//!
//! Quality level QL-B (docs/quality/PLAN_OVERVIEW.md). It runs a parsed
//! program one top-level item at a time ([`Machine::step`]) so that the
//! Playground can show each step, keeps the reason every NIL arose
//! (AR-HLR-020), and does every piece of arithmetic in `arare-kernel`. It is
//! the route that is easy to follow, not the fast one (AR-SYS-004).

#![forbid(unsafe_code)]

mod lift;
mod machine;
mod value;
mod words;

pub use machine::{run, Machine, RunError};
pub use value::{Error, ErrorKind, Reason, Value};

#[cfg(test)]
mod tests;
