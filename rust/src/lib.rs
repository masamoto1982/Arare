// The crate is `unsafe`-free, enforced by the compiler. `deny` rather than
// `forbid` because the `wasm`-gated bindings module must re-permit it:
// `wasm-bindgen` expands to generated glue that contains `unsafe`.
#![deny(unsafe_code)]

/// Every native build of the Core — the CLI, the tests, the examples — runs on
/// mimalloc (see its entry in Cargo.toml for why). The WebAssembly build is
/// excluded and keeps Rust's default allocator.
#[cfg(not(target_arch = "wasm32"))]
#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod builtins;
pub mod coreword_registry;
pub(crate) mod diagnostic_hint;
mod error;
mod fast_hash;
/// Word-name canonicalization, at the path every caller names it by; it lives
/// beside the Core Word registry (`coreword_registry::canonical_word_name`).
pub mod word_name {
    pub use crate::coreword_registry::canonical_word_name;
}
pub use error::{AjisaiError, ErrorCategory, NilReason};
pub mod interpreter;
pub mod kernel;
pub mod semantic;
pub mod surface_forms;
mod tokenizer;
mod tokenizer_scan;
pub mod types;

// Host-neutral agent boundary (pure computation, no filesystem/terminal I/O):
// shared by the native CLI below and the WASM one-shot entry point in
// `wasm_interpreter_bindings`, so every host renders the identical schema-1
// envelope (`docs/dev/agent-cli-output-contract.md`).
#[cfg(feature = "std")]
pub mod agent;

// Headless agent-facing CLI (the `ajisai` bin target). Native-only: it is
// host-adapter plumbing (file I/O, terminal rendering, REPL) over
// `crate::agent`.
#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
pub mod cli;

#[cfg(feature = "wasm")]
mod wasm_interpreter_bindings;

#[cfg(feature = "wasm")]
pub use wasm_interpreter_bindings::AjisaiInterpreter;

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tokenizer_regression_tests;

#[cfg(test)]
mod tokenizer_regression_tests_2;

#[cfg(test)]
mod tokenizer_mcdc_tests;

#[cfg(test)]
mod tokenizer_number_mcdc_tests;

#[cfg(test)]
mod lexical_grammar_laws;

#[cfg(test)]
mod identity_laws;

#[cfg(test)]
mod field_closure_laws;

#[cfg(test)]
mod zero_shape_laws;

#[cfg(test)]
mod arithmetic_operation_tests;

#[cfg(test)]
mod dimension_limit_tests;

#[cfg(test)]
mod materialization_limit_tests;

#[cfg(test)]
mod runtime_limits_tests;

#[cfg(test)]
mod conformance_tests;
