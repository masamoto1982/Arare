# ADR-0013: The Playground's engine boundary

Status: Accepted (Phase 5, 2026-11-07). Extends ADR-0012; supersedes the
wasm-bindgen boundary WORK_ORDER.md §8 sketched for the Playground (§8 is a
plan, not a §2 decision).
Requirements: AR-HLR-033 to AR-HLR-036, AR-LLR-031 to AR-LLR-038.
Verification: AR-VER-230 to AR-VER-232, AR-VER-301 to AR-VER-306.

## Context

WORK_ORDER.md §4 asks Phase 5 to turn `src/wasm-interpreter-types.ts` into an
interface with two implementations, Ajisai and Arare, and to run Arare's
engine in the browser. Ajisai crosses into JavaScript through wasm-bindgen: a
stateful object whose methods are exported functions. Arare's workspace
forbids `unsafe` code (`forbid(unsafe_code)`), and exporting functions by hand
needs `#[no_mangle]`, which that refuses; wasm-bindgen's generated glue needs
the same. ADR-0012 met this for the timing program by building a
`wasm32-wasip1` command.

Arare's interpreter also has no session of its own: `Machine` runs one
program, and a Playground in which a name bound by one Run is gone by the next
— or by the next step of step mode — would give programs another meaning than
they have when run whole.

## Decision

- **A command, not a library.** `rust/crates/arare-wasm` is a `wasm32-wasip1`
  binary with `main`: it reads a request on standard input, the command
  (`run`, `items`, `version`) from its first argument, and writes one JSON
  document (AR-LLR-032). It is safe Rust like every other crate. The browser
  runs it with a WASI preview 1 host of seven imports (`src/engine/wasi.ts`,
  AR-LLR-033), instantiating the module compiled once per page for each
  request; an instance costs about 0.15 ms (measured under Node, the V8 a
  browser runs), negligible against the wall-clock
  guard the shell already applies.
- **A session is its programs.** The request is the session's programs
  separated by NUL, the last one new. The engine runs them in order on one
  machine; Arare's programs are deterministic, so the stack and the names the
  earlier ones leave are those they left before. The adapter
  (`src/engine/arare-engine.ts`, AR-LLR-034) keeps the list of programs that
  succeeded, and its snapshot — what the worker hands back and what is saved —
  is that list with the stack it left, so restoring runs nothing. The cost is
  that a long session reruns its history; a Reset empties it.
- **The engine says where its items are.** Step mode steps by the engine's
  top-level items (AR-LLR-031), not by whitespace or brackets, because a
  quantized literal is four tokens and one item. The same spans place an
  error.
- **The engine is chosen per page load.** `?engine=`, else the last choice,
  else Arare (AR-LLR-035). Switching reloads the page, because the stack, the
  saved session and the worker pool all belong to one engine; persistence is
  keyed by engine so neither restores the other's session.
- **One comparison.** The Playground's comparison with Ajisai is
  `src/engine/oracle-compare.mjs`, which `scripts/lib/oracle.mjs` now imports,
  so the panel and `npm run check:oracle` cannot disagree on a verdict
  (AR-LLR-037).
- **The Reference is generated, in English and Japanese, from
  `spec/words.json`** (AR-LLR-038), which holds both languages side by side.
  `docs/i18n/` (held over by WORK_ORDER.md §4 for this phase) is not revived:
  the Japanese summary it was for is the Japanese Reference, generated from
  the same entries, so the two cannot drift.

## Consequences

- `npm run build` builds the engine first (`scripts/build-engine.mjs`); the
  `.wasm` is a build product, not committed, and CI adds the `wasm32-wasip1`
  target before the TypeScript tests, which run the real engine.
- Arare has no User Words, so the dictionary's User sheet, Export and Import
  are hidden on Arare; they stay on Ajisai.
- A session's rerun cost grows with its length. No performance requirement
  binds the Playground's interpreter route (AR-SYS-004); if one comes, the
  session can be kept in a long-lived instance instead without changing the
  protocol's answers.

## Alternatives

- **wasm-bindgen.** Needs `unsafe` in generated code the workspace forbids,
  and a second build tool (`wasm-pack`) to pin.
- **A long-lived WASI instance reading requests in a loop.** Keeps the session
  in memory, but WASI preview 1 has no way to block on input in a browser, so
  the host would have to resume a suspended instance; rerunning is simpler and
  exact.
- **Stepping by whitespace or brackets in the shell.** Ajisai's way; wrong for
  Arare's quantized literals.
