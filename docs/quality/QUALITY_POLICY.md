# Ajisai Quality Policy (DO-178B-Inspired)

## Purpose
This document defines Ajisai's internal quality baseline inspired by DO-178B principles.
It is a development quality framework and **not** a formal avionics certification claim.

## Policy Statements
1. **Requirements-first development**
   - Changes must be tied to documented intent (spec, issue, or design note).
2. **Traceability**
   - Requirements, implementation, and verification evidence must be linked,
     in `TRACEABILITY_MATRIX.md`.
3. **Independent verification mindset**
   - Reviewers validate tests and acceptance criteria, not only code style.
4. **Configuration integrity**
   - Release artifacts are built from version-controlled, reproducible workflows.
5. **Defect containment**
   - Quality issues are documented, triaged, and closed with objective evidence.
6. **Regression prevention**
   - CI quality gates (formatting, linting, tests, checks) are required for merge.

## Scope
Applies to:
- Rust interpreter/runtime (`rust/`)
- TypeScript/Web runtime (`src/`, `index.html`, `src-tauri/` frontend integration)
- CI workflows and release validation steps

## Enforcement
The PR checklist, issue template, verification plan, and CI workflows in this repository are the enforcement mechanisms for this policy.
