// The Tauri host with its plugins replaced by fakes: the save dialog answers
// with a path the user picked, and the state file holds whatever a test puts
// in it.

import { expect, it, vi } from 'vitest';

const fsState = vi.hoisted(() => ({ written: [] as string[], file: null as string | null }));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    save: async () => '/home/u/Documents/my-words-backup.json',
    open: async () => null
}));
vi.mock('@tauri-apps/plugin-fs', () => ({
    BaseDirectory: { AppData: 1 },
    exists: async () => fsState.file !== null,
    readTextFile: async () => fsState.file,
    writeTextFile: async (path: string) => { fsState.written.push(path); }
}));

import { TauriFileIO, TauriPersistence } from './tauri';

it('reports an export under the name the user chose, not the one offered', async () => {
    const saved = await new TauriFileIO().saveJson('user-words.json', { words: [] });
    expect(fsState.written).toContain('/home/u/Documents/my-words-backup.json');
    expect(saved.filename).toBe('my-words-backup.json');
});

it('reads a state file holding `null` or no JSON at all as no saved state', async () => {
    for (const content of ['null', '{"interpreterState": {"stateVer']) {
        fsState.file = content;
        await expect(new TauriPersistence().loadInterpreterState()).resolves.toBeNull();
    }
});
