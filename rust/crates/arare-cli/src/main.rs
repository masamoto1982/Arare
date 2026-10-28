//! Agent-facing command-line interface.
//!
//! Quality level QL-C (docs/quality/PLAN_OVERVIEW.md). Until the reference
//! interpreter exists (Phase 2 of WORK_ORDER.md §7) it answers its version
//! only, which `check:version-sync` holds equal to every other manifest.

fn main() {
    println!("arare {}", env!("CARGO_PKG_VERSION"));
}
