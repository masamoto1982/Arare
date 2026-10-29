# Architecture Decision Records

A decision with alternatives is recorded here, numbered, before the
requirement or code that depends on it (`docs/quality/STANDARDS.md`, design
standard). An accepted record is not edited to change its decision; a later
record supersedes it and says so.

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-value-representation.md) | Value representation: widths 8/16/32/64, power-of-two scales, the code layout | Accepted |
| [0002](0002-rounding-points.md) | Arithmetic widens; RESCALE, NARROW and DIV are the only rounding points; overflow is 1/0 | Accepted |
| [0003](0003-wheel-on-fixed-point.md) | The wheel on fixed point: which of Ajisai's wheel laws Arare keeps | Accepted |
| [0004](0004-where-widths-and-scales-are-written.md) | Widths and scales are written where a value enters, inferred elsewhere | Accepted |
| [0005](0005-sql-export-of-wheel-points.md) | Exporting 1/0 and 0/0 to SQL | Accepted |
| [0006](0006-requirements-registry.md) | The requirements registry and its generated surfaces | Accepted |
| [0007](0007-oracle-comparison.md) | Comparing Arare with the Ajisai oracle, divergences recorded as cases | Accepted |
| [0008](0008-error-budget-checker.md) | The error-budget checker: its domain, proved in Lean, computed in Rust with checked 128-bit rationals | Accepted |
