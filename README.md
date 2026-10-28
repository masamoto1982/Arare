![Rust](docs/assets/badges/rust.svg) ![WebAssembly](docs/assets/badges/webassembly.svg) ![TypeScript](docs/assets/badges/typescript.svg) ![Tauri](docs/assets/badges/tauri.svg) [Build and Deploy status](https://github.com/masamoto1982/Ajisai/actions/workflows/build.yml)

![Ajisai QR Code](public/images/Ajisai_QR_Small.png "Ajisai QR Code")

# Ajisai

Ajisai is a stack-based, vector-oriented dataflow language whose true user is AI. Numbers are never rounded, dividing by zero never stops a program, and truth is three-valued, as in SQL. There is nowhere to write a type; rigor lives in the machine-readable contracts the Words carry.

```
[ 1 2 3 ] [ 1 0 2 ] DIV    # [ 1/1 1/0 3/2 ]
```

## What Ajisai can do

- **Exact arithmetic.** Integers and decimals alike are exact rationals, and `SQRT` keeps its results as algebraic irrationals. There is no choosing between float, double and decimal. `0.1 0.2 ADD 0.3 EQ` is `TRUE`, and a sum or a product is one value in whatever order its operands are taken, points over zero included, so reordering or regrouping a `FOLD` never changes its answer.
- **Division that never stops.** Dividing by zero answers a point over zero, `1/0` or `0/0`, which is a number. In a vector computation only the element divided by zero is affected.
- **Vector computation without loops.** Arithmetic Words lift over vectors automatically, and `SELECT` branches element by element. Collections are processed with `MAP`, `FILTER` and `FOLD`.
- **Failure with a reason.** An operation that cannot produce a value answers a `NIL` carrying its reason, and that `NIL` flows on downstream. Evaluation stops only when the operation or the shape of its input is itself wrong, or when a program exceeds the limits the host can support.
- **Checking before running.** Every Word carries a contract stating what it takes and how many, whether it is pure, and what it returns when it cannot produce a value. User-defined Words are checked against these contracts before anything runs.
- **Use from AI.** Through the [MCP server](tools/mcp-server/README.md#install-and-connect), an AI can run Ajisai code and check its contracts.

## Why Ajisai

I had given up partway on every programming language I tried, and I wanted to build the tool itself rather than something with it. **Forth** showed me that a language is allowed to be that small. I still wanted the rigor of types without writing them, so Ajisai carries that rigor in contracts on the Words instead of annotations on values.

**AI** made the design practical. With AI writing the code alongside me, a language needs no syntactic sugar for human convenience; it can hold nothing but strict machine-readable rules and plain dataflow.

The arithmetic is not new: it is Carlström's *wheel of fractions* over the integers. What Ajisai adds is that every contract says, before anything runs, whether the code can reach a point over zero (`field=closed` or `field=leaving`).

I began the work in June, in Japan's rainy season. Watching the rain, I pictured fractions poured into a vessel called a Vector: one substance inside, never rounded, and rich ripples on its surface. The hydrangea — *ajisai* in Japanese — takes its scientific name, *Hydrangea*, from the Greek for "water vessel".

## Status

<table>
<tr><td>Release stage</td><td>Beta</td></tr>
<tr><td>Implementation version</td><td>1.0.0-beta.3</td></tr>
<tr><td>Specification version</td><td>1.0.0-beta.3 — what changed in each version is in <a href="spec/CHANGELOG.md"><code>spec/CHANGELOG.md</code></a></td></tr>
<tr><td>Specification</td><td>Generated from the semantic sources in <code>spec/</code> — see <a href="spec/README.md"><code>spec/README.md</code></a></td></tr>
<tr><td>Compatibility promise</td><td>From 1.0.0. Until then a breaking change raises the specification version, and <code>npm run check:spec-version</code> refuses one that does not</td></tr>
</table>

## Documentation

| Document | Audience | Rendered at |
|---|---|---|
| Specification | Builders and porters | [SPECIFICATION.html](https://masamoto1982.github.io/Ajisai/SPECIFICATION.html) |
| Reference (English) | Ajisai users | [docs/en/index.html](https://masamoto1982.github.io/Ajisai/docs/en/index.html) — from a program down to its elements and every built-in Word with its contract (generated from `spec/words.json`), with runnable samples |
| Reference (Japanese) | Ajisai users | [docs/ja/index.html](https://masamoto1982.github.io/Ajisai/docs/ja/index.html) — the same content in Japanese |
| Phrasebook | Ajisai users | [docs/phrasebook/phrasebook.ajisai](docs/phrasebook/phrasebook.ajisai) — User Words for phrases programs write again and again (a remainder, a disjunction, a sum or mean, text joined with a separator), each with a verified contract and examples. Run it once, or copy the definitions you need |
| Showcase | Anyone weighing Ajisai | [docs/showcase/showcase.ajisai](docs/showcase/showcase.ajisai) — short programs beside what binary floating point would answer: decimals that add up, Fibonacci numbers from Binet's formula through exact square roots, division by zero inside a Vector, and contracts that say before running whether code can reach a point over zero. Every example is run by the test suite |
| Playground | Run it now | [masamoto1982.github.io/Ajisai](https://masamoto1982.github.io/Ajisai/) — its Reference button opens the Reference in the UI language, English or Japanese |

## Build and run

| Task | Command |
|---|---|
| Install dependencies | `npm ci` |
| Dev server | `npm run dev` |
| Build the WASM core | `npm run build:wasm` |
| Build for the browser | `npm run build` |
| Build the desktop app (Tauri) | `npm run tauri:build` |
| Run the Rust test suite | `cargo test --all-targets` (in `rust/`) |
| Prove the formal-verification harnesses | `cargo kani` (in `rust/`; setup and scope in [docs/quality/FORMAL_VERIFICATION.md](docs/quality/FORMAL_VERIFICATION.md)) |

The MCP server for AI agents lives in [`tools/mcp-server/`](tools/mcp-server/README.md#install-and-connect).

## License

MIT — see [`LICENSE`](LICENSE).
