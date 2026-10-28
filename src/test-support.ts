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
