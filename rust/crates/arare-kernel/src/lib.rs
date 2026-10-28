//! Value codes, arithmetic, strong Kleene truth and Arrow-layout columns.
//!
//! Quality level QL-A (docs/quality/PLAN_OVERVIEW.md). Empty until Phase 2 of
//! WORK_ORDER.md §7: no code lands here without the requirement it implements
//! (CLAUDE.md, rule 1). The crate is `no_std` from the start so the kernels
//! stay usable on targets without an allocator or an operating system.
#![no_std]
