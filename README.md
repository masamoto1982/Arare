![Rust](docs/assets/badges/rust.svg) ![WebAssembly](docs/assets/badges/webassembly.svg) ![TypeScript](docs/assets/badges/typescript.svg) ![Tauri](docs/assets/badges/tauri.svg)

# Arare

Arare is a stack-based, vector-oriented language that quantizes by design and
accounts for the precision it gives up. Numbers are fixed-width — int8, int16
and int32 fixed point — and arithmetic widens until a program says where to
round; what it rounds away is a budget the compiler checks before anything
runs. Truth is three-valued, as in SQL, and laid out as Apache Arrow lays it out.

Arare and [Ajisai](https://github.com/masamoto1982/Ajisai) are a pair of
sister languages, each the other's antithesis. Ajisai fixes the value — it never
rounds — and counts what that costs; Arare fixes the cost — every value is one
machine word — and counts what that loses. Where a value's digits grow without
bound in Ajisai, it stays one machine word in Arare, with a bound on how far it
is from the exact value. The two share their syntax, their truth and the Words
they both have, and each checks the other on the programs they share.

## Status

**Phase 6a — Arrow input and output.** Arare has its own engine: an
interpreter, a compiled route (native and WebAssembly SIMD128), the
error-budget checker (`arare check`), and Apache Arrow IPC input and output.
The Playground runs Arare's engine and shows its results beside those of a
pinned build of Ajisai, its exact counterpart; the same comparison checks a
corpus and 10,000 generated programs, and the benchmarks hold Arare to the AR-PERF requirements.
The phase record is in
[`docs/quality/CONFIGURATION_INDEX.md`](docs/quality/CONFIGURATION_INDEX.md).

| | |
|---|---|
| Version | 0.1.0-alpha.0 |
| Work order and decisions | [`WORK_ORDER.md`](WORK_ORDER.md) (Japanese) |
| Quality framework | [`docs/quality/PLAN_OVERVIEW.md`](docs/quality/PLAN_OVERVIEW.md) — DO-178C-inspired; not a certification claim |
| Formal verification | [`docs/quality/FORMAL_VERIFICATION.md`](docs/quality/FORMAL_VERIFICATION.md) — Lean 4 and Kani |
| Upstream | [`UPSTREAM.md`](UPSTREAM.md) |

## Repository layout

| Path | Content |
|---|---|
| `rust/` | The engine workspace: specification, kernel, front end, budget checker, interpreter, code generator, WebAssembly boundary, CLI |
| `formal/` | The Lean 4 model of Arare's semantics and its proofs |
| `spec/` | The high-level requirements and the Word definitions |
| `docs/quality/` | Plans, standards, traceability, configuration index |
| `src/`, `index.html`, `src-tauri/` | The Playground (web and desktop), running Arare's engine beside Ajisai's |
| `src/wasm/ajisai-oracle/` | The pinned Ajisai build (1.0.0-beta.4) the Playground compares Arare with (the cross-check, ADR-0015) |
| `tools/mcp-server/` | Ajisai's MCP server, kept for the cross-check in agent sessions |

## Checks

```sh
npm ci
npm run check:rust      # fmt, clippy -D warnings, tests
npm run check:formal    # lake build (needs elan)
npm run check:gates     # traceability, versions, file size, contrast
npm run check:oracle         # Arare against Ajisai on the corpus
npm run check:oracle-random  # ... and on 10,000 generated programs
npm run check:arrow          # Arrow round trip against apache-arrow
npm run build:engine         # the Playground engine (needs wasm32-wasip1)
npm run check && npm run lint && npm test
```

The performance check (AR-PERF) runs in `rust/bench`:

```sh
cd rust/bench && RUSTFLAGS="-C target-cpu=x86-64-v3" cargo run --release -- --check
```

## License

MIT — see [`LICENSE`](LICENSE). Third-party notices are in
[`THIRD-PARTY-LICENSES.md`](THIRD-PARTY-LICENSES.md).
