// AR-VER-304: the comparison with Ajisai the Playground shows (AR-LLR-037):
// the same comparison the oracle harness makes (src/engine/oracle-compare.mjs),
// with the distance between the two values and a verdict per value; and, end
// to end, Arare's engine and Ajisai's build on the same session.

import { describe, expect, test } from 'vitest';
import { compareStacks, difference, norm, PRELUDE } from './oracle-compare.mjs';
import { oracleSource, readOracleAnswer, runOracle } from './ajisai-oracle';
import { buildComparisonView, describeField, describeVerdict } from '../gui/oracle-panel';
import { ArareEngine } from './arare-engine';
import { ajisaiCompute, loadArareModule, num, vec } from '../test-support';
import type { Value } from '../wasm-interpreter-types';

const withBudget = (v: Value, budget: string | null): Value => ({ ...v, budget });

describe('AR-VER-304: comparing two stacks', () => {
    test('a value within its budget, outside it, without one, or missing', () => {
        const rows = compareStacks(
            [withBudget(num(63, 64), '1/64'), withBudget(num(3), '0/1'), withBudget(num(1, 0), null), withBudget(num(2), '0/1')],
            [num(1), num(4), num(1, 0)]
        );
        expect(rows).toEqual([
            { arare: '63/64', ajisai: '1/1', budget: '1/64', difference: '1/64', verdict: 'within' },
            { arare: '3/1', ajisai: '4/1', budget: '0/1', difference: '1/1', verdict: 'outside' },
            { arare: '1/0', ajisai: '1/0', budget: null, difference: null, verdict: 'noBudget' },
            { arare: '2/1', ajisai: null, budget: '0/1', difference: null, verdict: 'missing' }
        ]);
        expect(compareStacks([], [num(1)])[0]).toMatchObject({ arare: null, ajisai: '1/1', verdict: 'missing' });
    });

    test('the distance between vectors is the largest lane distance; NIL lanes are skipped', () => {
        const a = norm(vec(num(1, 2), { type: 'nil', value: null }, num(3)));
        const j = norm(vec(num(1, 4), { type: 'nil', value: null }, num(3)));
        expect(difference(a, j)).toEqual({ p: 1n, q: 4n });
        expect(difference(norm(vec(num(1))), norm(vec(num(1), num(2))))).toBeNull();
        expect(difference(norm(vec(num(1, 0))), norm(vec(num(1, 0))))).toBeNull();
        expect(difference(norm(num(1)), norm({ type: 'boolean', value: true }))).toBeNull();
        expect(difference(norm({ type: 'boolean', value: true }), norm({ type: 'boolean', value: true }))).toBeNull();
    });

    test('Ajisai runs the prelude, then every program of the session', () => {
        expect(oracleSource(['1 2', 'ADD'])).toBe(`${PRELUDE}\n1 2\nADD`);
        expect(readOracleAnswer('{"stack":[]}')).toEqual({ ok: true, stack: [] });
        expect(readOracleAnswer('{"error":{"category":"x"},"message":"no"}')).toEqual({ ok: false, error: 'no' });
        expect(readOracleAnswer('{"error":"bad"}')).toEqual({ ok: false, error: '"bad"' });
    });

    test('a refusal or a failure to load is an answer, not an exception', async () => {
        await expect(runOracle(['1'], async () => { throw new Error('offline'); })).resolves.toEqual({ ok: false, error: 'offline' });
        await expect(runOracle(['1'], async () => '{"stack":[]}')).resolves.toEqual({ ok: true, stack: [] });
    });
});

describe('AR-VER-304: the panel', () => {
    test('the field contract and a summary of the verdicts', () => {
        expect(describeField(null)).toBe('field: —');
        expect(describeField({ field: 'closed', open: [] })).toContain('field=closed');
        expect(describeField({ field: 'open', open: [{ word: 'DIV', why: 'wheelPoint' }, { word: null, why: 'wheelPoint' }] }))
            .toBe('field=open: DIV (wheelPoint), literal (wheelPoint)');
        const view = buildComparisonView([withBudget(num(3), '0/1')], { field: 'closed', open: [] }, { ok: true, stack: [num(4)] });
        expect(view.summary).toContain('1 value(s) outside their budget');
        expect(buildComparisonView([], null, { ok: true, stack: [] }).summary).toBe('The stack is empty.');
        const refused = buildComparisonView([withBudget(num(1), '0/1')], null, { ok: false, error: 'Unknown word' });
        expect(refused).toMatchObject({ oracleError: 'Unknown word', summary: 'Ajisai refused the session: Unknown word' });
        expect(refused.rows[0]!.verdict).toBe('missing');
        expect(['within', 'outside', 'noBudget', 'missing'].map((v) => describeVerdict(v as 'within')))
            .toEqual(['within budget', 'outside budget', 'no budget', 'missing']);
    });
});

describe('AR-VER-304: Arare and Ajisai on the same session', () => {
    const compute = ajisaiCompute();

    test.each([
        ['an exact program', ['2 3 ADD', '4 MUL'], 'within'],
        ['a rounded literal and a product', ['1/3 8 -6 QUANTIZE [ 1 2 ] MUL SUM'], 'within'],
        ['a division, in two runs', ["7 'N' BIND", 'N 2 DIV'], 'within'],
        ['UNKNOWN takes ELSE in both (D-07)', ['1 2 NIL SELECT'], 'within'],
        ['SUM skips NIL in both (D-08)', ['[ 1 NIL 3 ] SUM'], 'within']
    ])('%s', async (_, programs, verdict) => {
        const e = new ArareEngine(loadArareModule());
        for (const p of programs) expect((await e.execute(p)).status).toBe('OK');
        const view = buildComparisonView(e.collect_stack(), e.budget_report(), await runOracle(e.session_programs(), compute));
        expect(view.rows.map((r) => r.verdict)).toEqual(view.rows.map(() => verdict));
    });
});
