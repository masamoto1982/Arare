import {
    describe,
    expect,
    test,
    it
} from 'vitest';
import type {
    ProtocolAbsence,
    Value
} from './wasm-interpreter-types';
import { parseHostProfile } from './wasm-interpreter-types';

const absence: ProtocolAbsence = {
    reason: 'domainMiss'
};

describe('Semantic Firewall protocol payload types', () => {
    test('Value carries structured semantics and absence metadata', () => {
        const value: Value = {
            type: 'nil',
            value: null,
            semantics: {
                absence
            }
        };

        expect(value.type).toBe('nil');
        expect(value.semantics?.absence?.reason).toBe('domainMiss');
        expect(Object.hasOwn(value, ['nil', 'Reason'].join(''))).toBe(false);
        expect(Object.hasOwn(value, ['error', 'Category'].join(''))).toBe(false);
    });
});

// ── merged from host-profile-parse.test.ts ──

// parseHostProfile: the shape guard between `host_profile()` and the labels
// that display it. Every rejected row below is a text that JSON.parse accepts
// and the display code would otherwise throw on.
//
// DUT: src/wasm-interpreter-types.ts, parseHostProfile()


describe('parseHostProfile', () => {
    it('accepts the shape the interpreter reports', () => {
        const text = JSON.stringify({
            profile: 'browser-playground',
            limits: { maxExecutionSteps: 1_000_000, materializedElements: 1_000_000 },
        });
        expect(parseHostProfile(text)).toEqual({
            profile: 'browser-playground',
            limits: { maxExecutionSteps: 1_000_000, materializedElements: 1_000_000 },
        });
    });

    it('accepts an empty ceiling table', () => {
        expect(parseHostProfile('{"profile":"p","limits":{}}')).toEqual({ profile: 'p', limits: {} });
    });

    it('rejects text that is not JSON', () => {
        expect(parseHostProfile('')).toBeNull();
        expect(parseHostProfile('not json')).toBeNull();
    });

    it('rejects JSON that is not an object', () => {
        expect(parseHostProfile('null')).toBeNull();
        expect(parseHostProfile('42')).toBeNull();
        expect(parseHostProfile('"browser-playground"')).toBeNull();
        expect(parseHostProfile('[]')).toBeNull();
    });

    it('rejects a missing or non-string profile name', () => {
        expect(parseHostProfile('{"limits":{}}')).toBeNull();
        expect(parseHostProfile('{"profile":1,"limits":{}}')).toBeNull();
    });

    it('rejects a missing, null, array or non-object limits table', () => {
        expect(parseHostProfile('{"profile":"p"}')).toBeNull();
        expect(parseHostProfile('{"profile":"p","limits":null}')).toBeNull();
        expect(parseHostProfile('{"profile":"p","limits":[1]}')).toBeNull();
        expect(parseHostProfile('{"profile":"p","limits":"5"}')).toBeNull();
    });

    it('rejects a ceiling that is not a finite number', () => {
        expect(parseHostProfile('{"profile":"p","limits":{"a":"5"}}')).toBeNull();
        expect(parseHostProfile('{"profile":"p","limits":{"a":null}}')).toBeNull();
        expect(parseHostProfile('{"profile":"p","limits":{"a":1e999}}')).toBeNull();
    });
});
