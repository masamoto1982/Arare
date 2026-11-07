// AR-VER-305: the Reference (AR-LLR-038). It is current with spec/words.json;
// every sample runs in the Playground's Arare engine and leaves exactly the
// result and budget the Reference prints; every "Open in Playground" link
// carries its sample to the Playground; and the Playground's word catalog is
// the registry's.

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, test } from 'vitest';
import { ArareEngine } from './arare-engine';
import catalog from './words.generated.json';
import { formatValue } from '../gui/output-display-renderer';
import { loadArareModule } from '../test-support';

interface Example { source: string; result: string[]; budget: Array<string | null> }
interface Entry { name: string; syntax: string; summary: { en: string }; examples: Example[] }

const root = resolve(import.meta.dirname, '../..');
const { words } = JSON.parse(readFileSync(resolve(root, 'spec/words.json'), 'utf8')) as { words: Entry[] };
const module = loadArareModule();

describe('AR-VER-305: the Reference', () => {
    test('is current with spec/words.json', () => {
        const out = execFileSync('node', [resolve(root, 'scripts/generate-word-reference.mjs'), '--check'], { encoding: 'utf8' });
        expect(out).toContain('current');
    });

    test.each(words.flatMap((w) => w.examples.map((e) => [w.name, e] as const)))(
        '%s: %o runs and leaves what the Reference prints',
        async (_, example) => {
            const engine = new ArareEngine(module);
            const result = await engine.execute(example.source);
            expect(result.status, result.message).toBe('OK');
            const stack = engine.collect_stack();
            expect(stack.map((v) => formatValue(v, 0))).toEqual(example.result);
            expect(stack.map((v) => v.budget)).toEqual(example.budget);
        }
    );

    test.each(['en', 'ja'])('every sample of the %s page opens in the Playground', (lang) => {
        const html = readFileSync(resolve(root, `public/docs/${lang}/index.html`), 'utf8');
        const links = [...html.matchAll(/class="open-playground" href="([^"]+)"/g)].map((m) => m[1]!.replace(/&amp;/g, '&'));
        const sources = words.flatMap((w) => w.examples.map((e) => e.source));
        expect(links).toHaveLength(sources.length);
        links.forEach((href, i) => {
            const [page, hash] = href.split('#');
            expect(page).toBe('../../index.html');
            // The marker gui-application.ts reads (PLAYGROUND_CODE_HASH_MARKER).
            expect(hash!.startsWith('code=')).toBe(true);
            expect(decodeURIComponent(hash!.slice('code='.length))).toBe(sources[i]);
        });
    });

    test("the Playground's catalog is the registry's", () => {
        expect(catalog.words.map((w) => [w.name, w.summary, w.syntax])).toEqual(
            words.map((w) => [w.name, w.summary.en, w.syntax])
        );
    });
});
