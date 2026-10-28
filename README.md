![Rust](docs/assets/badges/rust.svg) ![WebAssembly](docs/assets/badges/webassembly.svg) ![TypeScript](docs/assets/badges/typescript.svg) ![Tauri](docs/assets/badges/tauri.svg)

# Arare

Arare is a stack-based, vector-oriented language that quantizes by design and
accounts for the precision it gives up. Numbers are fixed-width — int8, int16
and int32 fixed point — and arithmetic widens until a program says where to
round; what it rounds away is a budget the compiler checks before anything
runs. Truth is three-valued, as in SQL, and laid out as Apache Arrow lays it out.

Arare is the counterpart of [Ajisai](https://github.com/masamoto1982/Ajisai),
which never rounds. Arare exists to be fast where Ajisai is exact: a value
whose digits grow without bound in Ajisai stays one machine word in Arare, and
Arare's results are checked against Ajisai's exact ones.

## Status

**Phase 0 — initialization.** Arare has no engine yet. The repository holds the
plans, the verification setup and the Playground shell, which runs a pinned
build of Ajisai as its oracle until Arare's engine is connected.

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
| `spec/` | The high-level requirements (from Phase 1) |
| `docs/quality/` | Plans, standards, traceability, configuration index |
| `src/`, `index.html`, `src-tauri/` | The Playground shell (web and desktop) |
| `src/wasm/ajisai-oracle/` | The pinned Ajisai build the Playground runs and Arare is checked against |
| `tools/mcp-server/` | Ajisai's MCP server, kept as the oracle for agent sessions |

## Checks

```sh
npm ci
npm run check:rust      # fmt, clippy -D warnings, tests
npm run check:formal    # lake build (needs elan)
npm run check:gates     # traceability, versions, file size, contrast
npm run check && npm run lint && npm test
```

## License

MIT — see [`LICENSE`](LICENSE). Third-party notices are in
[`THIRD-PARTY-LICENSES.md`](THIRD-PARTY-LICENSES.md).
