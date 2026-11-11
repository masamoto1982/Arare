// AR-VER-301: Arare's engine behind the Playground's seam (AR-LLR-033,
// AR-LLR-034). The real engine, built for wasm32-wasip1, run under the
// Playground's own WASI host: a session's programs carry their stack and
// their names from one run to the next, a failed run changes nothing, the
// snapshot restores a session without running it, and step mode's pieces
// are the engine's items.
//
// AR-VER-302: what the engine answers is the Playground's protocol
// (spec/host-protocol.schema.json): every node's type and fields are the
// schema's, Arare's additions included.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, test } from 'vitest';
import { ArareEngine, describeReport, offsetOf } from './arare-engine';
import { runWasiCommand } from './wasi';
import { loadArareModule } from '../test-support';
import type { Value } from '../wasm-interpreter-types';

const module = loadArareModule();
const engine = (): ArareEngine => new ArareEngine(module);

const shown = (v: Value): string => {
    if (v.type === 'number') return `${v.value.numerator}/${v.value.denominator}`;
    if (v.type === 'vector') return `[ ${(v.value as Value[]).map(shown).join(' ')} ]`;
    if (v.type === 'boolean') return v.value ? 'TRUE' : 'FALSE';
    if (v.type === 'nil') return 'NIL';
    return String(v.value);
};

describe('AR-VER-301: the WASI host', () => {
    test('runs the command with its arguments and standard input', () => {
        const out = runWasiCommand(module, ['arare-wasm', 'run'], '2 3 ADD');
        expect(out.exitCode).toBe(0);
        expect(JSON.parse(out.stdout).stack[0].value).toEqual({ numerator: '5', denominator: '1' });
        expect(out.stderr).toBe('');
    });

    test('reports the exit status and standard error of a misuse', () => {
        const out = runWasiCommand(module, ['arare-wasm', 'compile'], '');
        expect(out.exitCode).toBe(2);
        expect(out.stdout).toBe('');
        expect(out.stderr).toContain('unknown command');
    });

    test('reads a request larger than one read and text beyond ASCII', () => {
        const big = Array.from({ length: 4000 }, (_, i) => String(i % 100)).join(' ');
        const out = JSON.parse(runWasiCommand(module, ['arare-wasm', 'items'], `${big} 'αβ'`).stdout);
        expect(out.items).toHaveLength(4001);
        expect(out.items[4000]).toEqual({ line: 1, column: big.length + 2, endLine: 1, endColumn: big.length + 6 });
    });

    test('refuses other descriptors with EBADF and answers ENOSYS for an import it lacks', () => {
        // A command, assembled here, whose _start traps unless
        // fd_read(5, …) is EBADF (8), fd_write(3, …) is EBADF and the
        // unknown import `poll` answers ENOSYS (52).
        const enc = (s: string): number[] => [s.length, ...new TextEncoder().encode(s)];
        const section = (id: number, body: number[]): number[] => [id, body.length, ...body];
        const wasi = enc('wasi_snapshot_preview1');
        const check = (call: number[], expected: number): number[] =>
            [...call, 0x41, expected, 0x47, 0x04, 0x40, 0x00, 0x0b];
        const four = (fd: number, fn: number): number[] =>
            [0x41, fd, 0x41, 0, 0x41, 0, 0x41, 0, 0x10, fn];
        const body = [0x00, ...check(four(5, 0), 8), ...check(four(3, 1), 8), ...check([0x10, 2], 52), 0x0b];
        const bytes = new Uint8Array([
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            ...section(1, [3, 0x60, 4, 0x7f, 0x7f, 0x7f, 0x7f, 1, 0x7f, 0x60, 0, 1, 0x7f, 0x60, 0, 0]),
            ...section(2, [3, ...wasi, ...enc('fd_read'), 0, 0, ...wasi, ...enc('fd_write'), 0, 0, ...wasi, ...enc('poll'), 0, 1]),
            ...section(3, [1, 2]),
            ...section(5, [1, 0, 1]),
            ...section(7, [2, ...enc('memory'), 2, 0, ...enc('_start'), 0, 3]),
            ...section(10, [1, body.length, ...body])
        ]);
        expect(runWasiCommand(new WebAssembly.Module(bytes), ['x'], '')).toEqual({ exitCode: 0, stdout: '', stderr: '' });
    });
});

describe('AR-VER-301: the engine adapter', () => {
    test('a Run answers the stack with widths, scales and budgets, and the field contract', async () => {
        const e = engine();
        const result = await e.execute('1/3 8 -6 QUANTIZE [ 1 2 ] MUL SUM');
        expect(result).toEqual({ status: 'OK', output: 'field=closed', budgetReport: { field: 'closed', fieldExits: [] } });
        const [v] = e.collect_stack();
        expect(v).toMatchObject({ type: 'number', value: { numerator: '63', denominator: '64' }, width: 32, scale: -6, budget: '1/64' });
        expect(e.budget_report()).toEqual({ field: 'closed', fieldExits: [] });
    });

    test('a session carries its stack and its names from one run to the next', async () => {
        const e = engine();
        await e.execute("5 'N' BIND 1");
        await e.execute('N N ADD');
        expect(e.collect_stack().map(shown)).toEqual(['1/1', '10/1']);
        expect(e.session_programs()).toEqual(["5 'N' BIND 1", 'N N ADD']);
        await e.execute('ADD');
        expect(e.collect_stack().map(shown)).toEqual(['11/1']);
    });

    test('a failed run reports where and why, and leaves the session as it was', async () => {
        const e = engine();
        await e.execute('1');
        const failed = await e.execute('2\n  SUB SUB');
        expect(failed).toEqual({
            status: 'ERROR',
            error: true,
            message: 'SUB: needs 2 operand(s), the stack has 1',
            errorInfo: { category: 'stackUnderflow', word: 'SUB', line: 2, column: 7 }
        });
        expect(e.collect_stack().map(shown)).toEqual(['1/1']);
        const syntax = await e.execute('FOO');
        expect(syntax.errorInfo).toEqual({ category: 'syntax', line: 1, column: 1 });
        expect(e.session_programs()).toEqual(['1']);
    });

    test('field=leaving names each Word and why', async () => {
        const e = engine();
        const result = await e.execute('1 0 DIV 300 8 NARROW');
        expect(result.output).toBe('field=leaving: DIV wheelPoint, NARROW mayOverflow');
        expect(describeReport({ field: 'leaving', fieldExits: [{ word: null, why: 'wheelPoint' }] })).toBe('field=leaving: literal wheelPoint');
    });

    test('the snapshot restores a session without running it, and refuses what is not one', async () => {
        const e = engine();
        await e.execute('[ 1 2 ] [ 3 4 ] ADD');
        const snapshot = e.snapshot_stack();
        const other = engine();
        other.restore_stack_snapshot(snapshot);
        expect(other.collect_stack()).toEqual(e.collect_stack());
        expect(other.session_programs()).toEqual(['[ 1 2 ] [ 3 4 ] ADD']);
        expect(other.budget_report()).toEqual({ field: 'closed', fieldExits: [] });
        await other.execute('SUM');
        expect(other.collect_stack().map(shown)).toEqual(['10/1']);
        expect(() => other.restore_stack_snapshot('{"stack":[]}')).toThrow('Not a snapshot');
        expect(() => other.restore_stack_snapshot('{"programs":[1],"stack":[]}')).toThrow('Not a snapshot');
        other.restore_stack_snapshot('{"programs":[],"stack":[]}');
        expect(other.budget_report()).toBeNull();
    });

    test('clearing and resetting empty the session', async () => {
        const e = engine();
        await e.execute('1 2');
        e.clear_stack();
        expect(e.collect_stack()).toEqual([]);
        await e.execute('3');
        expect(e.reset()).toEqual({ status: 'OK' });
        expect(e.session_programs()).toEqual([]);
        expect(e.budget_report()).toBeNull();
    });

    test("step mode's pieces are the engine's items, as offsets in the source", async () => {
        const e = engine();
        await e.execute("'X' 1");
        const code = "0.75 8 -2 QUANTIZE\n  [ 2 MUL ] MAP 'α' X";
        const split = e.step_pieces(code);
        expect(split.ok).toBe(true);
        if (!split.ok) return;
        expect(split.pieces.map(({ start, end }) => code.slice(start, end))).toEqual([
            '0.75 8 -2 QUANTIZE', '[ 2 MUL ]', 'MAP', "'α'", 'X'
        ]);
        const bad = e.step_pieces('1 ]');
        expect(bad).toMatchObject({ ok: false, error: { status: 'ERROR', errorInfo: { category: 'syntax', line: 1, column: 3 } } });
    });

    test('offsets count characters beyond the Basic Multilingual Plane as JavaScript does', () => {
        const source = "'𝑥' 1\n2";
        expect(offsetOf(source, 1, 5)).toBe(5);
        expect(offsetOf(source, 2, 1)).toBe(7);
        expect(offsetOf(source, 3, 1)).toBe(source.length);
    });

    test('the dictionary is the Core Words of spec/words.json; there are no User Words', () => {
        const e = engine();
        const core = e.collect_core_words_info();
        expect(core.map(([name]) => name)).toContain('QUANTIZE');
        expect(core.find(([name]) => name === 'DIV')).toEqual([
            'DIV', 'DIV — The quotient a / b, rounded: one of Arare\'s three rounding points.', 'a b DIV'
        ]);
        expect(e.resolve_host_lookup('RESCALE')?.text).toContain('Scale: Raised by s.');
        expect(e.resolve_host_lookup('DUP')).toBeNull();
        expect(e.collect_user_words_info()).toEqual([]);
        expect(e.collect_word_identities()).toEqual([]);
        expect(e.lookup_word_definition()).toBeNull();
        expect(e.lookup_word_description()).toBeNull();
        expect(e.restore_user_words([{ name: 'GREET', definition: '1' }])).toEqual([['GREET', 'Arare has no User Words']]);
        e.set_max_execution_steps();
        expect(JSON.parse(e.host_profile())).toEqual({ profile: 'arare-playground', limits: {} });
        expect(() => e.agent_host_profile()).toThrow('no agent host');
    });
});

// ── AR-VER-302: the protocol ───────────────────────────────────────────────

interface Schema {
    $defs: {
        observedValue: { properties: Record<string, { enum?: unknown[] }> };
        semantics: { properties: Record<string, unknown> };
        absence: { properties: Record<string, unknown> };
    };
}

const schema = JSON.parse(
    readFileSync(resolve(import.meta.dirname, '../../spec/host-protocol.schema.json'), 'utf8')
) as Schema;

const conforms = (node: Value, top: boolean, path: string): void => {
    const props = schema.$defs.observedValue.properties;
    for (const key of Object.keys(node)) expect(props, `${path}: ${key}`).toHaveProperty(key);
    expect(props.type!.enum, path).toContain(node.type);
    if (node.type === 'number') {
        expect(props.width!.enum, path).toContain(node.width);
        expect(Number.isInteger(node.scale) && Math.abs(node.scale!) <= 31, path).toBe(true);
    }
    if (!top) expect(node.budget, `${path}: a budget only on a value of the stack`).toBeUndefined();
    if (top) expect(node.budget === null || /^-?[0-9]+\/[0-9]+$/.test(node.budget!), path).toBe(true);
    if (node.semantics) {
        for (const key of Object.keys(node.semantics)) expect(schema.$defs.semantics.properties, path).toHaveProperty(key);
        for (const key of Object.keys(node.semantics.absence ?? {})) expect(schema.$defs.absence.properties, path).toHaveProperty(key);
    }
    if (node.type === 'vector') (node.value as Value[]).forEach((v, i) => conforms(v, false, `${path}[${i}]`));
};

describe('AR-VER-302: the engine speaks the Playground protocol', () => {
    test('every kind of value Arare leaves conforms to spec/host-protocol.schema.json', async () => {
        const e = engine();
        await e.execute("TRUE NIL [ NIL ] SUM 1 0 DIV 0.5 16 -4 QUANTIZE [ 1 [ 2 3 ] ] [ 2 MUL ] 'X' [ 1 2 ] [ 3 4 ] MUL");
        const stack = e.collect_stack();
        expect(stack.map((v) => v.type)).toEqual(['boolean', 'nil', 'nil', 'number', 'number', 'block', 'block', 'string', 'vector']);
        stack.forEach((v, i) => conforms(v, true, `stack[${i}]`));
        expect(stack[2]!.semantics).toEqual({ absence: { reason: 'emptyAggregate' } });
        expect(stack[5]!.value).toBe('[ 1 [ 2 3 ] ]');
    });
});
