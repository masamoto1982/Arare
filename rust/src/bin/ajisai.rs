//! `ajisai` — headless CLI for running and checking Ajisai programs.
//!
//! Thin entry point only; all behavior lives in `ajisai_core::cli` so it can
//! be unit-tested in-crate. Output contract:
//! `docs/dev/agent-cli-output-contract.md`.

#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(ajisai_core::cli::run(&args));
}

// The bin target is still type-checked when the crate is built for
// wasm32 (CI: `cargo check --features wasm --target wasm32-unknown-unknown`);
// there is no terminal there, so the entry point compiles to a no-op.
#[cfg(not(all(feature = "std", not(target_arch = "wasm32"))))]
fn main() {}
