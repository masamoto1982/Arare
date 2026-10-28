// The Tauri host: one JSON document in the app-data directory for
// persistence, and the native dialogs and filesystem for file I/O.

import type {
    FileIO,
    InterpreterStateSnapshot,
    OpenResult,
    Persistence,
    SaveResult,
    StoredInterpreterState
} from './index';
import { WEB_PERSISTENCE, formatJsonDocument, readInterpreterStateSnapshot } from './web';

// Resolved only at runtime inside the Tauri WebView; `@vite-ignore` keeps the
// web build from trying to bundle the Tauri SDK.
const dynamicImport = (specifier: string): Promise<any> =>
    import(/* @vite-ignore */ specifier);

// The file's one field. A `tables` array sat beside it for years without
// anything ever writing an entry; a file that still carries one is read for
// its state and rewritten without it.
interface StoredData {
    interpreterState: StoredInterpreterState | null;
}

const STATE_FILE = 'ajisai-state.json';

const cloneEmptyData = (): StoredData => ({
    interpreterState: null
});

async function readStoredData(): Promise<StoredData> {
    const { readTextFile, exists, BaseDirectory } = await dynamicImport('@tauri-apps/plugin-fs');

    const fileExists = await exists(STATE_FILE, { baseDir: BaseDirectory.AppData });
    if (!fileExists) {
        return cloneEmptyData();
    }

    // A file that is not a JSON object — truncated, or holding `null` — is no
    // saved state, as an unreadable document is on the web, rather than an
    // error that starts the session without the Example Words.
    const raw = await readTextFile(STATE_FILE, { baseDir: BaseDirectory.AppData });
    let parsed: unknown;
    try {
        parsed = JSON.parse(raw);
    } catch (error) {
        console.warn(`${STATE_FILE} is not JSON; starting without saved state:`, error);
        return cloneEmptyData();
    }
    if (typeof parsed !== 'object' || parsed === null) {
        return cloneEmptyData();
    }

    return {
        interpreterState: (parsed as Partial<StoredData>).interpreterState ?? null
    };
}

async function writeStoredData(data: StoredData): Promise<void> {
    const { writeTextFile, BaseDirectory } = await dynamicImport('@tauri-apps/plugin-fs');

    await writeTextFile(STATE_FILE, formatJsonDocument(data), { baseDir: BaseDirectory.AppData });
}

export class TauriPersistence implements Persistence {
    private opened = false;

    async open(): Promise<void> {
        if (this.opened) {
            return;
        }

        const { exists, BaseDirectory } = await dynamicImport('@tauri-apps/plugin-fs');

        const alreadyExists = await exists(STATE_FILE, { baseDir: BaseDirectory.AppData });
        if (!alreadyExists) {
            await writeStoredData(cloneEmptyData());
            // A first launch inherits whatever the web playground had stored in
            // this WebView's IndexedDB.
            await this.migrateFromIndexedDb(() => WEB_PERSISTENCE.exportInterpreterState()).catch((error) => {
                console.warn('Failed to migrate IndexedDB data into Tauri storage:', error);
            });
        }

        this.opened = true;
    }

    private async migrateFromIndexedDb(
        exportWebState: () => Promise<StoredInterpreterState | null>
    ): Promise<void> {
        const interpreterState = await exportWebState();
        if (!interpreterState) {
            return;
        }

        await writeStoredData({ interpreterState });
    }

    async saveInterpreterState(state: InterpreterStateSnapshot): Promise<void> {
        await this.open();
        await writeStoredData({
            interpreterState: {
                key: 'interpreter_state',
                ...state,
                updatedAt: new Date().toISOString()
            }
        });
    }

    async loadInterpreterState(): Promise<InterpreterStateSnapshot | null> {
        return readInterpreterStateSnapshot(await this.exportInterpreterState());
    }

    async clearAll(): Promise<void> {
        await this.open();
        await writeStoredData(cloneEmptyData());
    }

    async exportInterpreterState(): Promise<StoredInterpreterState | null> {
        await this.open();
        const current = await readStoredData();
        return current.interpreterState;
    }
}

export class TauriFileIO implements FileIO {
    async saveJson(defaultName: string, data: unknown): Promise<SaveResult> {
        const [{ save }, { writeTextFile }] = await Promise.all([
            dynamicImport('@tauri-apps/plugin-dialog'),
            dynamicImport('@tauri-apps/plugin-fs')
        ]);

        const path = await save({
            defaultPath: defaultName,
            filters: [{ name: 'JSON', extensions: ['json'] }]
        });

        if (!path) {
            throw new Error('Save cancelled');
        }

        await writeTextFile(path, formatJsonDocument(data));
        // The name the user chose in the dialog, not the one it was offered.
        return { filename: path.split(/[\\/]/).pop() ?? defaultName };
    }

    async openJsonFile(): Promise<OpenResult | null> {
        const [{ open }, { readTextFile }] = await Promise.all([
            dynamicImport('@tauri-apps/plugin-dialog'),
            dynamicImport('@tauri-apps/plugin-fs')
        ]);

        const selected = await open({
            multiple: false,
            filters: [{ name: 'JSON', extensions: ['json'] }]
        });

        if (!selected || Array.isArray(selected)) {
            return null;
        }

        const text = await readTextFile(selected);
        const filename = selected.split(/[\\/]/).pop() ?? 'import.json';
        return { filename, text };
    }
}
