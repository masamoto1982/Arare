// AR-VER-306: which engine the Playground runs, and what each can do
// (AR-LLR-035): `?engine=` first, then the remembered choice, then Arare;
// Arare's descriptor says it has budgets and names that outlive a step, and
// no User Words; Ajisai's, the reverse.

import { describe, expect, test } from 'vitest';
import { ENGINES, ENGINE_STORAGE_KEY, selectEngineKind } from './registry';

describe('AR-VER-306: choosing the engine', () => {
    test('the URL decides, then the remembered choice, then Arare', () => {
        expect(selectEngineKind('?engine=ajisai', 'arare')).toBe('ajisai');
        expect(selectEngineKind('?engine=arare', 'ajisai')).toBe('arare');
        expect(selectEngineKind('', 'ajisai')).toBe('ajisai');
        expect(selectEngineKind('?engine=python', 'ajisai')).toBe('ajisai');
        expect(selectEngineKind('?x=1', 'nonsense')).toBe('arare');
        expect(selectEngineKind('', null)).toBe('arare');
        expect(ENGINE_STORAGE_KEY).toBe('arare-playground-engine');
    });

    test('each descriptor says what its engine can do', () => {
        expect(ENGINES.arare).toMatchObject({ kind: 'arare', userWords: false, namesAcrossSteps: true, budgets: true });
        expect(ENGINES.ajisai).toMatchObject({ kind: 'ajisai', userWords: true, namesAcrossSteps: false, budgets: false });
        expect(ENGINES.arare.badge).toBe('Playground · Arare');
        expect(ENGINES.ajisai.badge).toBe('Playground · Ajisai oracle');
    });

    test("each engine's Reference: Arare's own, Ajisai's at the oracle's commit", () => {
        expect(ENGINES.arare.reference('ja')).toBe('docs/ja/index.html');
        expect(ENGINES.arare.reference('en')).toBe('docs/en/index.html');
        expect(ENGINES.ajisai.reference('en')).toBe(
            'https://github.com/masamoto1982/Ajisai/blob/dd5d1b222f11b72e8aa1664ad68b890dcafc06c4/public/docs/en/index.html'
        );
    });
});
