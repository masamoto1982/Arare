## Summary

<!-- Describe the change and intent. -->

## Quality Classification

- Highest impacted level: <!-- QL-A / QL-B / QL-C / QL-D -->

## Traceability

- Requirement(s): <!-- e.g., AQ-REQ-00X -->
- Verification evidence: <!-- tests, CI jobs, checklists -->

## Quality Checklist

- [ ] Relevant requirements/objectives are identified.
- [ ] Traceability links were added/updated.
- [ ] `cargo fmt --check` (in `rust/`)
- [ ] `cargo clippy --all-targets -- -D warnings` (in `rust/`)
- [ ] `cargo test --all-targets --verbose` (in `rust/`)
- [ ] `npm run check`, if applicable
- [ ] `cargo llvm-cov --branch --workspace`, if applicable
- [ ] `cargo kani` (in `rust/`), if a proved kernel changed
- [ ] MC/DC-like checklist reviewed for modified boolean logic
- [ ] Release checklist impact considered

## Notes

This repository uses a DO-178B-inspired internal process and does not claim formal DO-178B certification.
