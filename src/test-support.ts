// Builders for protocol values the GUI tests hand to the code under test.
//
// Test-only: nothing under src/ that ships imports this file, so the bundle
// never carries it. Kept beside the sources because the tests are.

import type { Fraction, Value } from './wasm-interpreter-types';

export const frac = (numerator: string | number, denominator: string | number = 1): Fraction => ({
    numerator: String(numerator),
    denominator: String(denominator),
});

export const num = (numerator: string | number, denominator: string | number = 1): Value =>
    ({ type: 'number', value: frac(numerator, denominator) });

export const str = (s: string): Value => ({ type: 'string', value: s });

export const vec = (...items: Value[]): Value => ({ type: 'vector', value: items });

export const rec = (keys: Value[], values: Value[]): Value =>
    ({ type: 'record', value: { keys, values } });

// Arare's engine as the Playground loads it, compiled from the file
// scripts/build-engine.mjs writes (npm run build:engine). The tests that use
// it fail, rather than skip, when it is missing: CI builds it first.
import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

const ENGINE_WASM = resolve(import.meta.dirname, 'wasm/arare/arare_wasm.wasm');

export const loadArareModule = (): WebAssembly.Module => {
    if (!existsSync(ENGINE_WASM)) {
        throw new Error(`${ENGINE_WASM} is missing; build it with npm run build:engine`);
    }
    return new WebAssembly.Module(readFileSync(ENGINE_WASM));
};

// Ajisai's Node build, the one the oracle harness runs (scripts/lib/oracle.mjs).
export const ajisaiCompute = (): ((source: string) => Promise<string>) => {
    const require = createRequire(import.meta.url);
    const ajisai = require(resolve(import.meta.dirname, '../tools/mcp-server/wasm/generated/ajisai_core.js')) as {
        agent_compute: (source: string) => Promise<string>;
    };
    return (source) => ajisai.agent_compute(source);
};
