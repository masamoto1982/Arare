# ADR-0010: Names, as Ajisai's BIND

Status: Accepted (Phase 4b, 2026-11-04).
Requirements: AR-HLR-032, AR-LLR-028, AR-LLR-029.
Verification: AR-VER-227, AR-VER-228, AR-VER-209.

## Context

The logistic map (AR-PERF-003) reads x twice a step, and Arare's Phase 2
vocabulary has no way to read a value twice: no DUP, no SWAP, no names. A FOLD
block gets its accumulator and its lane and must leave one value, so
x · (1 − x) cannot be written. Ajisai answers the same need with BIND and
names (Ajisai's `interpreter/bindings.rs`), and its Words are the ones Arare
keeps unless it has a reason of its own (AR-SYS-001).

## Decision

- **BIND and names are Ajisai's.** `value 'X' BIND` consumes both operands;
  X pushes the value afterwards, however often; binding again replaces it;
  `vector [ 'A' 'B' ] BIND` destructures. Names are case-sensitive, and a Word
  name, a number or a token holding whitespace, a bracket, a quote or `#` is
  not one.
- **Scopes as Ajisai's, without User Words.** The program is one scope; every
  run of a block by MAP, FILTER or FOLD opens one, which reads the names
  around it and whose own bindings end with it. Arare has no Word calls, so
  Ajisai's barrier between a Word and its caller does not arise.
- **The front end still refuses unknown Words.** A bare name is accepted only
  where the source writes it as text somewhere (`'X'`), so a misspelled Word
  is refused before anything runs, as before.
- **Text is a value only BIND reads.** `'X'` pushes the name; every other Word
  refuses it, and EQ compares it as a name. There are no strings beyond names.
- **A block bound to a name it reads is refused**, directly or through the
  blocks bound to the names it reads — Ajisai's `selfReferentialDefinition`.

## Alternatives

- **DUP, SWAP and OVER.** Smaller, but not Ajisai's: the same source would no
  longer run in both languages, and Ajisai's own notes (`bindings.rs`) record
  why stack shuffling does not reach a value used twice in a DAG.
- **Lexical capture of quotes.** Ajisai resolves a name in the frame a block
  was written in. Without User Words, a block runs within the scope it was
  written in except when a quote leaves its block as a value; Arare looks the
  name up where it runs. The difference is recorded as PR-0013 rather than
  paid for with closures now.

## Consequences

- The compiled route lowers names and BIND (AR-LLR-029), and FOLD with them;
  BIND of several names stays on the interpreter (PR-0011).
- The checker binds what is known of a value in the same scopes (AR-LLR-020).
- The Playground's highlighting and Reference learn BIND in Phase 5, with the
  rest of the vocabulary.
