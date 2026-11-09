# Arrow input and output

The design behind AR-LLR-039 to AR-LLR-043: how `rust/crates/arare-arrow`
writes and reads the Arrow IPC formats without third-party code, how numbers
and truth values become Arrow columns, and how `arare run` uses it. The
decisions themselves are ADR-0014 (and ADR-0005 for the wheel points).

## Modules

| Module | Requirement | What it does |
|---|---|---|
| `fb.rs` | AR-LLR-039, AR-LLR-041 | The FlatBuffers subset: a front-to-back builder (`Obj`, `finish`) and a checked reader (`Table`) |
| `ipc.rs` | AR-LLR-039 | The stream and file framing, the `Schema`, `Message`, `RecordBatch` and `Footer` tables |
| `columns.rs` | AR-LLR-040 | `Column`, `NumColumn`, and each column's Arrow type, metadata and buffers (`layout`) |
| `decimal.rs` | AR-LLR-040 | Unscaled decimal values as a sign and a 256-bit magnitude, precision, base64 |
| `read.rs` | AR-LLR-039 to AR-LLR-041 | Reading either format, Arare's decimals exactly and foreign columns per ADR-0004 |
| `values.rs` | AR-LLR-042 | The interpreter's stack as columns and back |

## FlatBuffers without a library (AR-LLR-039, AR-LLR-041)

The official builders write back to front. `fb::finish` writes front to back
instead: an object is laid out before the objects it points to, so every
unsigned offset is positive, and the patch is a subtraction. A table's
vtable goes immediately before it; the table starts on an 8-byte boundary
with its fields sorted by size, so each scalar sits at a multiple of its
size relative to the buffer and therefore to the 8-byte-aligned message.
The C++ verifier checks that alignment, and its reader refuses a `Field`
without a `children` vector, so one is always written.

The reader never trusts a number it reads: `fb::Table` checks the vtable's
length and evenness, that each field lies inside the table's inline size,
and that every offset, string and vector lies inside the buffer; `read.rs`
checks every body buffer against the body and against the bytes its lanes
need before any lane is read. A length is used for an allocation only after
the buffer it describes has been found long enough, so a forged length cannot
ask for more memory than the input's size implies.

## Columns (AR-LLR-040, AR-LLR-042)

A number column is written as a decimal because no Arrow type is a binary
fixed point; code × 2^e with e < 0 equals code × 5^(−e) / 10^(−e), so the
decimal is exact, and SQL engines read the number, not the code. The width
and exponent in the field's metadata let the reader divide back. An int64
code times 5^31 is about 2^135, so `decimal.rs` computes on a 256-bit
magnitude; Decimal256 is needed only at width 64 with e ≤ −28.

The wheel points are null, as ADR-0005 decided, and the two metadata bitmaps
say which nulls were which. A reader that does not know Arare sees NULL,
which is what SQL can say about a value outside the range.

A stack vector's lanes need not share a width or an exponent (`[ 1 1000 ]`
holds an int8 and an int16). `to_columns` aligns them to the smallest
exponent, an exact left shift, and picks the narrowest width that holds the
aligned codes and is no narrower than any lane's own, so a vector whose lanes
agree keeps its width and exponent through the round trip. Truth columns use
the kernel's `TruthColumn` bitmaps (AR-LLR-009) unchanged.

A bracketed literal is a block until something reads it (AR-HLR-021), so the
stack is read through `Machine::data_stack`, which reads each block as data
in the scope the program ended in, as a Word there would.

## The command line (AR-LLR-043)

`arare run --arrow-in FILE` pushes the columns before the program, the first
at the bottom, so a program sees them as it would see literals written in
that order. `--arrow-out FILE` writes the file format and still prints the
stack; `--arrow-out -` writes the stream format to standard output in place
of the stack, for a pipe into another Arrow reader.

## Verification

- AR-VER-233 (`tests/roundtrip.rs`): every width and every exponent from −31
  to 31 with random lanes, points and NIL, in both formats; stack values
  through columns.
- AR-VER-234 (`tests/malformed.rs`): every prefix and 40,000 seeded byte flips
  of valid input read to an answer or an error.
- AR-VER-235 (`scripts/check-arrow-interop.mjs`): `apache-arrow` reads what
  Arare writes, including the Decimal256 case and the metadata; Arare reads
  what `apache-arrow` writes (Int32, Boolean, Decimal at `--arrow-decimal`,
  Float64 refused); and Arare's file rewritten by `apache-arrow` reads back
  the same.
- AR-VER-236 (`arare-cli/tests/arrow.rs`): the flags, exit statuses and
  column order.
