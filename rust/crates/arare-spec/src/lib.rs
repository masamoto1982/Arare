//! Executable specification of Arare's value model.
//!
//! Quality level QL-A (docs/quality/PLAN_OVERVIEW.md). Every definition here
//! mirrors the Lean definition of the same name in `formal/Arare/`
//! (AR-LLR-001), on `i128` in place of Lean's unbounded `Int`: wide enough for
//! every value an int64 operation produces. Speed is not its concern; being
//! easy to hold against the Lean model is. The Lean job of CI prints a table
//! from both and diffs them (`src/table.rs`, `formal/Table.lean`),
//! and the kernel is tested against this crate.

#![forbid(unsafe_code)]
// `add`, `mul`, `not` and their kin are named after the Lean definitions and
// the Words they implement, and `mul` can fail; operator traits would hide both.
#![allow(clippy::should_implement_trait)]

mod rounding;
mod sql;
mod table;
mod truth;
mod wheel;

pub use rounding::{rescale, round_div, round_div_pos};
pub use sql::{eq_t, filter_t, lt_t, select_t, sum_t, truth_of};
pub use table::write_table;
pub use truth::Truth;
pub use wheel::{decode, encode, fit, in_range, W};
