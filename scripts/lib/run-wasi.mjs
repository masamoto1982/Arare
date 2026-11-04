// Runs a wasm32-wasip1 program under Node's WASI (preview 1), with the
// program's standard output as this process's: how CI runs the WebAssembly
// build of rust/crates/arare-timing (AR-VER-226).
//
// Usage: node scripts/lib/run-wasi.mjs <program.wasm> [args…]

import { readFile } from 'node:fs/promises';
import { WASI } from 'node:wasi';

const [file, ...args] = process.argv.slice(2);
const wasi = new WASI({ version: 'preview1', args: [file, ...args], env: {}, returnOnExit: true });
const module = await WebAssembly.compile(await readFile(file));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
process.exitCode = wasi.start(instance);
