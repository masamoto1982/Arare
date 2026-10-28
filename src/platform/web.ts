// The web host: IndexedDB persistence and the browser's own file dialogs.

import type {
    FileIO,
    InterpreterStateSnapshot,
    OpenResult,
    Persistence,
    SaveResult,
    StoredInterpreterState
} from './index';

/** The one serialization every JSON document this host writes uses. */
export const formatJsonDocument = (data: unknown): string => JSON.stringify(data, null, 2);

/**
 * The stored record read back as the snapshot the GUI restores from, or null
 * when nothing is stored. Both hosts store the same record and answer the
 * same snapshot, so the reading is written once.
 */
export const readInterpreterStateSnapshot = (
    result: StoredInterpreterState | null
): InterpreterStateSnapshot | null => {
    if (!result) {
        return null;
    }
    return {
        stateVersion: Number(result.stateVersion),
        stackSnapshot: result.stackSnapshot as InterpreterStateSnapshot['stackSnapshot'],
        userWords: result.userWords as InterpreterStateSnapshot['userWords'],
        activeDictionarySheet: result.activeDictionarySheet
    };
};

const promisifyRequest = <T>(request: IDBRequest<T>): Promise<T> =>
    new Promise((resolve, reject) => {
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
    });

const withObjectStore = <T>(
    db: IDBDatabase,
    storeName: string,
    mode: IDBTransactionMode,
    action: (store: IDBObjectStore) => Promise<T>
): Promise<T> => {
    const store = db.transaction([storeName], mode).objectStore(storeName);
    return action(store);
};

// One record, under one key, in one object store. A second store, `tables`,
// was created, cleared, exported and migrated for years without anything
// ever writing a row to it; version 5 drops it.
const STATE_STORE = 'interpreter_state';
const STATE_KEY = 'interpreter_state';
const RETIRED_TABLES_STORE = 'tables';

class WebPersistence implements Persistence {
    private dbName = 'AjisaiDB';
    private version = 5;
    private db: IDBDatabase | null = null;
    private openPromise: Promise<IDBDatabase> | null = null;

    async open(): Promise<void> {
        if (this.db) {
            return;
        }

        if (this.openPromise) {
            await this.openPromise;
            return;
        }

        if (!window.indexedDB) {
            throw new Error('IndexedDB is not supported in this browser');
        }

        this.openPromise = new Promise<IDBDatabase>((resolve, reject) => {
            const request = indexedDB.open(this.dbName, this.version);

            request.onerror = () => {
                this.openPromise = null;
                reject(request.error);
            };

            request.onsuccess = () => {
                this.db = request.result;
                this.openPromise = null;
                resolve(this.db);
            };

            request.onupgradeneeded = (event) => {
                const db = (event.target as IDBOpenDBRequest).result;

                if (db.objectStoreNames.contains(RETIRED_TABLES_STORE)) {
                    db.deleteObjectStore(RETIRED_TABLES_STORE);
                }

                if (!db.objectStoreNames.contains(STATE_STORE)) {
                    db.createObjectStore(STATE_STORE, { keyPath: 'key' });
                }
            };
        });

        await this.openPromise;
    }

    async saveInterpreterState(state: InterpreterStateSnapshot): Promise<void> {
        if (!this.db) await this.open();

        return withObjectStore(this.db!, STATE_STORE, 'readwrite', async store => {
            const stateData: StoredInterpreterState = {
                key: STATE_KEY,
                ...state,
                updatedAt: new Date().toISOString()
            };
            await promisifyRequest(store.put(stateData));
        });
    }

    async loadInterpreterState(): Promise<InterpreterStateSnapshot | null> {
        return readInterpreterStateSnapshot(await this.exportInterpreterState());
    }

    async clearAll(): Promise<void> {
        if (!this.db) await this.open();

        return withObjectStore(this.db!, STATE_STORE, 'readwrite', async store => {
            await promisifyRequest(store.clear());
        });
    }

    async exportInterpreterState(): Promise<StoredInterpreterState | null> {
        if (!this.db) await this.open();

        return withObjectStore(this.db!, STATE_STORE, 'readonly', async store =>
            ((await promisifyRequest(store.get(STATE_KEY))) as StoredInterpreterState | undefined) ?? null
        );
    }
}

// One IndexedDB store per page: the web adapter's persistence, and the source
// the Tauri adapter migrates from on its first launch.
export const WEB_PERSISTENCE: Persistence = new WebPersistence();

const readFileAsText = (file: File): Promise<string> =>
    new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = (event) => {
            const result = event.target?.result;
            if (typeof result === 'string') {
                resolve(result);
            } else {
                reject(new Error('Failed to read file'));
            }
        };
        reader.onerror = () => reject(new Error('Failed to read file'));
        reader.readAsText(file);
    });

export class WebFileIO implements FileIO {
    async saveJson(defaultName: string, data: unknown): Promise<SaveResult> {
        const jsonString = formatJsonDocument(data);
        const blob = new Blob([jsonString], { type: 'application/json' });
        const url = URL.createObjectURL(blob);

        const a = document.createElement('a');
        a.href = url;
        a.download = defaultName;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);

        return { filename: defaultName };
    }

    async openJsonFile(): Promise<OpenResult | null> {
        return new Promise((resolve, reject) => {
            const input = document.createElement('input');
            input.type = 'file';
            input.accept = '.json';

            input.onchange = () => {
                const file = input.files?.[0];
                if (!file) {
                    resolve(null);
                    return;
                }
                readFileAsText(file)
                    .then((text) => resolve({ filename: file.name, text }))
                    .catch(reject);
            };

            input.click();
        });
    }
}
