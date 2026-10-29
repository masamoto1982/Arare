//! Static range and error-budget checker; decides the `field` contract
//! (AR-LLR-018 to AR-LLR-021, refining AR-HLR-027 to AR-HLR-029).
//!
//! Quality level QL-A (docs/quality/PLAN_OVERVIEW.md). For every value a
//! program leaves, the checker infers how far Arare's result may be from the
//! exact one Ajisai computes on the same source — its error budget — without
//! running the program, and decides whether the program is `field=closed`:
//! no overflow, no division by zero, no wheel point (WORK_ORDER.md D-12).
//!
//! The numeric core ([`info`]) is the Lean model's checker
//! (`formal/Arare/Budget.lean`), proved sound there; [`check`] carries it
//! through the rest of the language (docs/design/budget.md).

// `Q::add`, `Q::mul` and the rest answer `Option`: an operation whose exact
// result leaves 128 bits answers `None`. The operator traits would hide that
// check behind `+` and `*`, so the names stay methods.
#![allow(clippy::should_implement_trait)]

pub mod check;
pub mod info;
pub mod rat;
pub mod table;
pub mod value;
mod words;

pub use check::{analyze, Analysis, Open, Why};
pub use info::{Info, Range};
pub use rat::Q;
pub use value::{budget, Abs};
