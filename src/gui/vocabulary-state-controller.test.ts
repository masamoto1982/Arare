// Dictionary Delete runs `DEL` on the session interpreter itself, not on a
// worker, so nothing else stands between a refused delete and the session.
// A refusal used to leave the word's name on the stack, where the next Run
// picked it up and saved it. Run against the real interpreter.

import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import * as wasm from '../wasm/generated/ajisai_core.js';
import type { AjisaiInterpreter } from '../wasm-interpreter-types';
import { deleteUserWord } from './vocabulary-state-controller';

wasm.initSync({ module: readFileSync(new URL('../wasm/generated/ajisai_core_bg.wasm', import.meta.url)) });

const createInterpreter = (): AjisaiInterpreter => new wasm.AjisaiInterpreter() as unknown as AjisaiInterpreter;

it('leaves the stack as it was when DEL refuses a word other words call', async () => {
    const interpreter = createInterpreter();
    await interpreter.execute("[ 'A' PRINT ] 'A' DEF [ A ] 'B' DEF [ 1 ] [ 2 ]");
    const stackBefore = interpreter.collect_stack();

    const result = await deleteUserWord(interpreter, 'A');

    expect(result.status).toBe('ERROR');
    expect(result.errorInfo?.category).toBe('definitionConflict');
    expect(interpreter.collect_stack()).toEqual(stackBefore);
});

it('deletes a word nothing calls and leaves the stack alone', async () => {
    const interpreter = createInterpreter();
    await interpreter.execute("[ 'A' PRINT ] 'A' DEF [ 1 ]");
    const stackBefore = interpreter.collect_stack();

    const result = await deleteUserWord(interpreter, 'A');

    expect(result.status).toBe('OK');
    expect(interpreter.collect_user_words_info()).toEqual([]);
    expect(interpreter.collect_stack()).toEqual(stackBefore);
});
