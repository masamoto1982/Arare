# The Playground

The design behind AR-LLR-031 to AR-LLR-038: how the Playground's shell
(`src/`) drives Arare's engine and Ajisai's through one interface, how step
mode and Run share the editor, how the error budget and the oracle
comparison are shown, and how the Reference is generated. The boundary itself
is ADR-0013.

## The engine interface (AR-HLR-033, AR-LLR-034, AR-LLR-035)

`PlaygroundEngine` (`src/wasm-interpreter-types.ts`) is what the shell asks
of an engine: execute a program, read the stack, snapshot and restore a
session, list the Words and their documentation, and, optionally, split a
program into steps. Ajisai's wasm-bindgen object implements it as it always
did. `ArareEngine` (`src/engine/arare-engine.ts`) implements it on Arare's
engine command, keeping the session as the programs that built it.

What differs between engines and is not a method — whether there are User
Words, whether a name lasts from one step to the next, whether budgets are
inferred, where the Reference is — is a field of the engine's descriptor
(`src/engine/registry.ts`). The shell reads the descriptor instead of probing
the object, so nothing is shown that the engine cannot do.

The page's engine is decided once, before anything else loads:
`selectEngineKind(location.search, stored)`. The module is compiled once
(`compileEngine`) and handed to the main thread's instance and to every
worker (`initEngine` in `src/workers/execution-worker-manager.ts`), which
instantiates the same engine on it. Persistence is keyed by engine
(`setPersistenceScope` in `src/platform/web.ts`; one file per engine in
`src/platform/tauri.ts`).

## The engine command (AR-LLR-031, AR-LLR-032, AR-LLR-033)

`arare-wasm` answers three commands; `spec/host-protocol.schema.json`
describes the values in its answers.

| Command | Request | Answer |
|---|---|---|
| `run` | programs, NUL-separated | `{ok, stack, field, open, items}` or `{ok: false, error, items?}` |
| `items` | programs, NUL-separated | `{ok, items}` — the spans of the last program's items |
| `version` | — | the crate's version |

An item's span comes from `parse_spanned` (AR-LLR-031), and an error's line
and column are those of the item that raised it, within the last program.
Earlier programs have already succeeded once, so an error in them cannot
happen; the engine still reports one rather than assume it.

`src/engine/wasi.ts` is the host: it gives the command its arguments and
standard input, collects standard output and error, and turns `proc_exit`
into an exit status. Every import a Rust command does not use answers ENOSYS.

## Run and Step (AR-HLR-034, AR-LLR-036)

The execute toolbar under the editor holds the mode switch and one execute
button; the choice is remembered (`src/gui/execution-mode.ts`). Shift+Enter
still runs and Ctrl+Enter still steps, whatever the switch says.

Step mode asks the engine for the program's items (`step_pieces`) and runs
them one by one through the same path as Run, highlighting the item about to
run. Because Arare's session replays its programs, each step is a program of
the session, and a name bound in one step is read in the next; on Ajisai,
which runs each piece on its own, a program using BIND is refused as before.
When the source does not parse, step mode runs it once, which reports the
error Run would.

## Budgets and the oracle (AR-HLR-035, AR-LLR-037)

Every number in Arare's answer carries its width, scale and inferred budget,
and the stack display labels it so (`describeQuantization`). The comparison
panel under the stack shows the field contract and, when opened, runs the
session's programs on Ajisai — loaded only then — after the oracle prelude,
and compares the two stacks with `compareStacks`, the function the oracle
gate uses. Each row shows Arare's value, its budget, Ajisai's exact value,
their distance, and the verdict.

## The Reference (AR-HLR-036, AR-LLR-038)

`spec/words.json` lists the Words in the interpreter's order, each with its
syntax, and in English and Japanese what it does to widths, scales, error and
the field, with examples. `scripts/generate-word-reference.mjs` writes
`public/docs/en/index.html`, `public/docs/ja/index.html` and the Playground's
word catalog (`src/engine/words.generated.json`). An example links to
`index.html#code=…`, which the Playground loads into the editor. Two tests
keep the file honest: `rust/crates/arare-wasm/tests/words.rs` (the names and
order are the interpreter's, AR-VER-232) and `src/engine/reference.test.ts`
(each example's result and budgets are what the engine answers, AR-VER-305).
