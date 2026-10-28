# Third-Party Licenses

Ajisai itself is licensed under the MIT License (see [`LICENSE`](LICENSE)).

This document lists the third-party software that Ajisai **redistributes** —
i.e. components that ship inside a built artifact (the web bundle, the
self-hosted `public/vendor/` assets, the WebAssembly module, or the Tauri
desktop binary). Build-time-only tools that are **not** redistributed are
listed separately at the end.

Almost all redistributed dependencies are under permissive licenses (MIT,
Apache-2.0, BSD, ISC, Zlib, Unicode) and impose no source-disclosure
obligation on Ajisai. There are no strong-copyleft (GPL/LGPL) dependencies.

---

## Web / front-end (shipped in the web bundle and `public/vendor/`)

### KaTeX 0.17.0 — MIT License

Self-hosted at [`public/vendor/katex/`](public/vendor/katex/) and also bundled
into the web build via `import katex from 'katex'`
(`src/gui/output-display-renderer.ts`). This includes `katex.min.js`,
`katex.min.css`, `contrib/auto-render.min.js`, and the `KaTeX_*.woff2` webfonts.

> The MIT License (MIT)
>
> Copyright (c) 2013-2020 Khan Academy and other contributors
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

Canonical text: <https://github.com/KaTeX/KaTeX/blob/main/LICENSE>.
A copy also ships next to the assets at
[`public/vendor/katex/LICENSE`](public/vendor/katex/LICENSE).

### coi-serviceworker technique — MIT License

[`public/coi-serviceworker.js`](public/coi-serviceworker.js) is an original,
header-only reimplementation of the cross-origin-isolation service-worker
technique popularised by Guido Zuidhof's
[`coi-serviceworker`](https://github.com/gzuidhof/coi-serviceworker) (MIT).
It is not a copy of the upstream source; the attribution is retained in the
file's header comment.

---

## Desktop application (shipped in the Tauri binary)

The desktop build links the following crates. Each is dual-licensed
**MIT OR Apache-2.0** unless noted; the canonical license texts are those
published with each crate on <https://crates.io> / its source repository.

| Component | License (SPDX) |
| --- | --- |
| `tauri`, `tauri-build` | MIT OR Apache-2.0 |
| `tauri-plugin-fs` | MIT OR Apache-2.0 |
| `tauri-plugin-dialog` | MIT OR Apache-2.0 |
| `tauri-plugin-store` | MIT OR Apache-2.0 |
| `@tauri-apps/api`, `@tauri-apps/plugin-*` (JS) | MIT OR Apache-2.0 |



---

## Core interpreter (shipped in the WebAssembly module and the desktop binary)

Rust crates linked into `ajisai-core` (`rust/Cargo.toml`):

| Crate | License (SPDX) |
| --- | --- |
| `serde`, `serde_json` | MIT OR Apache-2.0 |
| `num-bigint`, `num-traits`, `num-integer` | MIT OR Apache-2.0 |
| `lazy_static` | MIT OR Apache-2.0 |
| `smallvec` | MIT OR Apache-2.0 |
| `wasm-bindgen`, `wasm-bindgen-futures`, `js-sys`, `web-sys` | MIT OR Apache-2.0 |
| `serde-wasm-bindgen` | MIT OR Apache-2.0 |
| `console_error_panic_hook` | MIT OR Apache-2.0 |
| `getrandom` | MIT OR Apache-2.0 |
| `mimalloc`, `libmimalloc-sys` (and the bundled mimalloc C library, © Microsoft) — native builds only, not the WebAssembly module | MIT |

The full text of the two licenses referenced above is available at:

- MIT: <https://opensource.org/license/mit>
- Apache-2.0: <https://www.apache.org/licenses/LICENSE-2.0>

To regenerate an authoritative, transitive attribution list for the Rust
side, run [`cargo-about`](https://github.com/EmbarkStudios/cargo-about) or
`cargo-bundle-licenses` against `rust/Cargo.lock` and `src-tauri/Cargo.lock`.

---

## Build-time only (NOT redistributed)

These are `devDependencies` / build tooling. Their own code is not included in
any shipped artifact, so they impose no attribution obligation on Ajisai's
distributed output. Listed here for completeness only.

| Tool | License |
| --- | --- |
| Vite | MIT |
| Vitest | MIT |
| TypeScript | Apache-2.0 |
| ESLint, `@typescript-eslint/*` | MIT |
| `ts-prune` | MIT |
| `@tauri-apps/cli` | MIT OR Apache-2.0 |
| `@types/node` | MIT |
