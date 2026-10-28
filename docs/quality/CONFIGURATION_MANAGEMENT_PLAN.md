# Configuration Management Plan

## Configuration items

Everything under version control is a configuration item: requirements,
design, source, the Lean model, tests, harnesses, plans, CI workflows,
generated files and the pinned oracle bundle. Build outputs (`rust/target/`,
`formal/.lake/`, `dist/`, `coverage/`) are not; CI regenerates them.

## Baselines

| Baseline | Identification | Content |
|---|---|---|
| Baseline | Commit `481dc6cac9e70df776b4cf0dec3b224bb1b47629` on `main` (untagged, by the owner's choice) | The content of Ajisai `4d5f693` plus `WORK_ORDER.md`, `CLAUDE.md` and `UPSTREAM.md` (`UPSTREAM.md`) |
| End of each phase | The merge commit on `main` recorded in `CONFIGURATION_INDEX.md`; a tag `arare-phase-N` when the owner sets one | The state that met the phase's exit criteria |
| Each release | A tag `v<version>` on `main` | The released state, with its `RELEASE_SUMMARY.md` entry |

## Versions

One version number, held in `package.json` and repeated exactly by the Rust
workspace, the Tauri crate and the Tauri configuration; `npm run
check:version-sync` enforces it.

## Toolchains

Every tool that builds or verifies Arare is pinned, and the pins are listed in
`CONFIGURATION_INDEX.md`. Changing a pin is a change like any other: one pull
request that updates the pin and the index together (CLAUDE.md, rule 6).

## Reproducibility

- proptest runs with a fixed seed in CI (`PROPTEST_RNG_SEED`).
- Generated files are never edited by hand; their generator's `--check` mode
  runs in CI.
- The Ajisai oracle is pinned by commit, never followed by branch.

## Change control

Changes reach `main` only through pull requests (`DEVELOPMENT_PLAN.md`).
Problem reports are GitHub issues opened from the quality template; each one
links the report, the fix and the verification that closes it.
