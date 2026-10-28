//! Value codes, arithmetic, strong Kleene truth and Arrow-layout columns.
//!
//! Quality level QL-A (docs/quality/PLAN_OVERVIEW.md). The crate is `no_std`
//! (with `alloc` for columns) so the kernels stay usable on targets without an
//! operating system. Its design is docs/design/kernel.md; each module names the
//! low-level requirement it implements, and is tested against the executable
//! specification in `arare-spec`, which mirrors the Lean model.
#![no_std]
// `add`, `mul`, `not` and their kin are named after the Lean definitions and
// the Words they implement, and `mul` can fail; operator traits would hide both.
#![allow(clippy::should_implement_trait)]

extern crate alloc;

mod codes;
mod column;
mod num;
mod truth;
mod width;

pub use codes::{decode, encode, fit, fit_val, is_code, Val};
pub use column::TruthColumn;
pub use num::{Num, ScaleOverflow};
pub use truth::Truth;
pub use width::Width;

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod proofs;
