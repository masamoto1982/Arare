# ADR-0014: Arrow input and output

Status: Proposed (Phase 6, 2026-11-09). Builds on ADR-0001, ADR-0004 and
ADR-0005, whose decisions stand; does not change WORK_ORDER.md D-06.
Requirements: AR-SYS-006, AR-HLR-019, AR-HLR-026, AR-HLR-031. The requirements
for the export and the import are written with the code, after this record is
accepted (as ADR-0005 left them).
Verification: planned below; IDs are assigned with the requirements.

## Context

Phase 6's exit condition is that values and NIL survive a round trip through
Arrow (WORK_ORDER.md §7). D-06 lays truth values and NIL out as Arrow does, so
that they cross into Arrow-based SQL engines without a copy; AR-HLR-019 and
AR-LLR-009 already keep a truth column as Arrow's validity and value bitmaps.
Four questions are still open:

1. **The route.** Arrow data crosses a boundary in one of two ways. The C Data
   Interface hands over pointers inside one process; the IPC format
   (stream or file) is bytes. The C Data Interface needs raw pointers and an
   `extern "C"` release callback, that is, `unsafe` code, which the workspace
   forbids (`docs/quality/STANDARDS.md`: a crate that needs it records the
   exception in an ADR and in `.github/CODEOWNERS`). Arare has no in-process
   host today: its hosts are `arare-cli`, a separate process, and the
   Playground, a WebAssembly command (ADR-0013), and neither can share memory
   with DuckDB or Polars.
2. **The implementation.** The workspace has no third-party crate.
   `arrow-ipc` 60.1.0, the Rust implementation's IPC reader and writer,
   brings 28 crates with it with default features off (among them
   `flatbuffers`, `chrono`, `half`, `num-bigint`, `hashbrown`, `zerocopy` and
   the `syn` proc-macro stack), and arrow-rs publishes a new major version
   every few months.
3. **The numbers.** An Arare number is a code times 2^e (ADR-0001). Arrow has
   no binary fixed point: its decimals are scaled by powers of ten, and its
   integers carry no scale.
4. **The wheel points and NIL.** ADR-0005 already turns 1/0 and 0/0 into SQL
   NULL with a record of which lanes were which; how that record is written in
   Arrow is open.

## Decision

- **IPC first, in a new safe crate.** `rust/crates/arare-arrow` (QL-C, as
  `docs/quality/PLAN_OVERVIEW.md` places Arrow and SQL I/O) reads and writes
  the Arrow IPC stream and file formats for the subset Arare uses: a schema of
  Boolean and Decimal fields (Int fields on import only, below), record
  batches, little-endian, no compression, no dictionaries, no nesting. It is
  safe Rust under `forbid(unsafe_code)` like every other crate, and it builds
  for `wasm32-wasip1`, so the Playground can load and save Arrow files too.
  `arare-cli` gains an Arrow input and output; Polars reads an IPC file by
  memory map, and DuckDB through its Arrow support.
- **Written by hand, without arrow-rs.** The subset needs the FlatBuffers
  tables of `Schema`, `Field`, `Message` and `RecordBatch`, and the buffers
  after them. The reader treats its input as untrusted: every offset and
  length is checked before use, and a malformed file is an error, never a
  panic.
- **The C Data Interface later, in its own crate.** When Arare gains an
  in-process host (a Python module or a C library), `arare-arrow-ffi` exports
  and imports through the C Data Interface. It is then the one crate allowed
  `unsafe`, recorded in a further ADR and in `.github/CODEOWNERS`. The buffers
  `arare-arrow` builds are already laid out as Arrow lays them out, so that
  crate hands them over without a copy; D-06 is met by the layout now and by
  the pointer exchange then.
- **Numbers as decimals.** A number column of width N and exponent e is
  exported as an Arrow decimal with scale s = max(0, −e). For e < 0 its
  unscaled value is code × 5^(−e), since code × 2^e = code × 5^(−e) / 10^(−e);
  for e ≥ 0 it is code × 2^e. Both are exact. The precision is the number of
  decimal digits of (M − 2) × 5^(−e) or (M − 2) × 2^e; the field is
  Decimal128 when that is at most 38 and Decimal256 otherwise, which happens
  only at width 64 with e ≤ −28. The field's metadata records `arare:width`
  and `arare:exponent`, from which the import recovers every code exactly
  (an exact division by 5^(−e) or 2^e).
- **Wheel points as ADR-0005 says.** A lane holding 1/0 or 0/0 is null in the
  validity bitmap. When a column holds a point, its field metadata carries
  `arare:one-over-zero` and `arare:zero-over-zero`, each an Arrow-ordered
  bitmap of the column's length in base64, so a reader that knows Arare
  restores them and any other reader sees NULL. A NIL lane is null and in
  neither bitmap. NIL's reason is not exported (D-11: only the reference
  interpreter keeps it).
- **Truth columns as they are.** A truth column is an Arrow Boolean field
  whose validity and value bitmaps are AR-LLR-009's, written without change;
  UNKNOWN is null.
- **Foreign input says its scale where it enters (ADR-0004).** A field
  carrying `arare:width` and `arare:exponent` is read back exactly. Otherwise
  an IntN field is read at width N and exponent 0, and a value outside that
  width's finite range is 1/0; a Decimal field needs a width and an exponent
  from the caller, and is quantized as `QUANTIZE` does (AR-HLR-026); a Boolean
  field is a truth column; any other type is refused, naming the field.

## Alternatives

- **arrow-rs (`arrow-ipc`, or `arrow` with `ffi`).** Rejected for now: it
  would be the first third-party code in the workspace, 28 crates to pin and
  review as software of unknown pedigree in `CONFIGURATION_INDEX.md`, with
  frequent major versions, for a subset of a few hundred lines. Reconsidered if
  the subset grows to compression, dictionaries or nested types.
- **The C Data Interface first.** Rejected for now: it brings `unsafe` into
  the workspace for a host that does not exist yet, and works neither across
  the CLI's process boundary nor in the Playground.
- **Integer fields with the binary exponent in metadata (an extension type).**
  This hands the code buffer over without a copy, but every SQL engine that
  does not know the extension reads the codes as the values, silently wrong
  whenever e ≠ 0. Kept as a possible opt-in between two Arare processes, not
  as the export.
- **Float64.** Rejected: codes of width 64 beyond 2^53 are not exact in it,
  and the wheel points would have to become Infinity and NaN, which ADR-0005
  rejected.
- **Decimal128 only, refusing what does not fit.** Rejected: a width-64
  column at a fine exponent is an ordinary result of widening (ADR-0002), and
  refusing it would block a whole table, the argument ADR-0005 made for the
  points.

## Consequences

- The round trip the exit condition asks for becomes a property test over
  generated columns: all four widths, exponents −31 to 31, wheel points and
  NIL lanes, written and read back to the same codes and the same validity.
- The reader and the writer are checked against an independent Arrow
  implementation in CI, used only in tests: the `apache-arrow` npm package
  reads what Arare writes and writes what Arare reads, so the hand-written
  FlatBuffers code is not checked only against itself. Malformed inputs are
  generated from valid ones by truncation and byte flips, with a fixed seed.
- AR-HLR-019's planned verification, "a round trip through the Arrow C Data
  Interface", becomes a round trip through the IPC format, changed with the
  requirements.
- A number column crosses as a decimal, so its unscaled values are computed
  (a multiplication per lane) rather than shared; truth columns and validity
  bitmaps are written as they are.
- SQL export (WORK_ORDER.md §3.5) reads the same decimals and the same
  metadata, so it is not a second encoding.
