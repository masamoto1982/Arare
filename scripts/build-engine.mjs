#!/usr/bin/env node
// Build Arare's engine for the Playground (AR-LLR-032, ADR-0013): the
// arare-wasm command for wasm32-wasip1, release profile, copied to
// src/wasm/arare/arare_wasm.wasm, where the Playground (src/engine/registry.ts)
// and the tests (src/engine/arare-engine.test.ts) load it. The file is built,
// never committed (.gitignore): the Rust source is what is reviewed.
//
//   node scripts/build-engine.mjs
//
// Needs the target: `rustup target add wasm32-wasip1` (in rust/, so the
// pinned toolchain gets it).

import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { fatal, repoRoot } from './lib/common.mjs';

const TAG = 'build-engine';
const rust = resolve(repoRoot, 'rust');
const built = resolve(rust, 'target/wasm32-wasip1/release/arare-wasm.wasm');
const target = resolve(repoRoot, 'src/wasm/arare/arare_wasm.wasm');

const r = spawnSync('cargo', ['build', '--release', '--locked', '-p', 'arare-wasm', '--target', 'wasm32-wasip1'], {
  cwd: rust,
  stdio: 'inherit',
});
if (r.status !== 0) fatal(TAG, 'cargo build failed; is the target installed? (cd rust && rustup target add wasm32-wasip1)');
mkdirSync(resolve(target, '..'), { recursive: true });
copyFileSync(built, target);
console.log(`[${TAG}] wrote src/wasm/arare/arare_wasm.wasm`);
