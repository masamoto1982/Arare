//! The ahead-of-time compiler (AR-LLR-023 to AR-LLR-026 and AR-LLR-029,
//! refining AR-HLR-030 to AR-HLR-032).
//!
//! Quality level QL-A (docs/quality/PLAN_OVERVIEW.md). A program and a
//! signature — the widths and scales of the inputs it is handed — are lowered
//! to a typed intermediate representation ([`ir`], [`lower`]) and emitted as
//! Rust ([`emit`]), which `rustc` compiles for the host or for WebAssembly
//! (ADR-0009). The emitted code runs a fast path on plain integer codes and
//! falls back to the reference interpreter for everything else ([`rt`]), so
//! it answers what the interpreter answers (docs/design/codegen.md).

pub mod emit;
pub mod ir;
pub mod lower;
mod lower_names;
mod lower_words;
mod node;
pub mod rt;

pub use emit::{emit, PRELUDE};
pub use ir::Input;
pub use lower::{lower, Unsupported};
