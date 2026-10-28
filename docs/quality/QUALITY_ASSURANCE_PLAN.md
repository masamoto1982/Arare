# Quality Assurance Plan

Quality assurance checks that the other plans are followed. It does not
replace verification; it confirms verification happened as planned.

## What is assured, and how

| Objective | Mechanism |
|---|---|
| Every change implements a requirement | Pull request template asks for the IDs; `npm run check:traceability` fails on an ID the matrix lacks |
| Traceability is complete in both directions | `npm run check:traceability`: every ID in the source has a row, every live row has a subject, every path exists |
| The evidence a level requires is present | Review against `VERIFICATION_PLAN.md` |
| No coverage is lost on QL-A and QL-B files | `npm run check:coverage-ratchet` |
| Proofs are checked, not trusted | The `formal-verification-kani` and `formal-verification-lean` CI jobs |
| Tools and versions are the pinned ones | `CONFIGURATION_INDEX.md`, reviewed on every pin change |
| Problems are closed with evidence | Problem reports link the fix and its verification |
| Phase transitions meet their criteria | The phase-end review records the result in `CONFIGURATION_INDEX.md` before the phase tag is set |

## Gates are blocking

Every CI step is blocking. Ajisai once ran a staged rollout in which the gate
was advisory by default, and learned that an advisory gate produces green
checkmarks that certify nothing; Arare starts blocking.

## Prohibited ways to reach green

- Disabling, skipping or quarantining a test.
- Lowering the coverage baseline without a reviewed reason in the same diff.
- Editing a generated file by hand.
- Writing in `FORMAL_VERIFICATION.md` that something is proved when it is not.
