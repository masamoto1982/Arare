//! Arrow input and output (ADR-0014; AR-HLR-037, AR-HLR-038).
//!
//! Quality level QL-C (docs/quality/PLAN_OVERVIEW.md). The crate reads and
//! writes the Arrow IPC stream and file formats for the subset Arare needs,
//! in safe Rust and without third-party code: Boolean columns for truth
//! values, Decimal columns for numbers (Int columns on input only), one
//! record batch out, any number in. Its design is docs/design/arrow.md.
//!
//! - [`write_stream`] and [`write_file`] write a [`Batch`]; [`read`] reads
//!   either format back, treating its input as untrusted (AR-LLR-041).
//! - [`to_columns`] and [`to_values`] move between the interpreter's stack
//!   and columns (AR-LLR-042).

#![forbid(unsafe_code)]

mod columns;
mod decimal;
mod fb;
mod ipc;
mod read;
mod values;

pub use columns::{Column, NumColumn};
pub use ipc::{write_file, write_stream};
pub use read::{read, ReadOptions};
pub use values::{to_columns, to_values};

use core::fmt;

/// Named columns of one length.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Batch {
    pub columns: Vec<(String, Column)>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// The input is not Arrow IPC as the format defines it.
    Malformed(String),
    /// Well-formed Arrow that Arare does not read: a type, a feature or a
    /// value outside what ADR-0014 accepts. Names the field when there is one.
    Unsupported(String),
    /// A value on the stack that has no column (AR-LLR-042).
    NotAColumn(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Malformed(m) => write!(f, "not Arrow IPC: {m}"),
            Error::Unsupported(m) => write!(f, "unsupported Arrow: {m}"),
            Error::NotAColumn(m) => write!(f, "no Arrow column for {m}"),
        }
    }
}

impl std::error::Error for Error {}
