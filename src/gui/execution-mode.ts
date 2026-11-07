// Switching between Run and step execution (AR-LLR-036, refining AR-HLR-034).
//
// Both have always had a key — Shift+Enter runs the editor, Ctrl+Enter takes
// one step — and keep it. The switch decides what the Execute button and the
// touch gesture (triple-tap on the editor) do, so a reader on a phone, who has
// no Ctrl+Enter, can step through a program too. The choice is remembered for
// the next visit.

export type ExecutionMode = 'run' | 'step';

export const EXECUTION_MODE_STORAGE_KEY = 'arare-playground-execution-mode';

/** A stored choice, or Run when there is none or it is not one. */
export const readExecutionMode = (stored: string | null): ExecutionMode =>
    stored === 'step' ? 'step' : 'run';

/** The Execute button's label and title in `mode`. */
export const describeExecutionMode = (mode: ExecutionMode): { label: string; title: string } =>
    mode === 'run'
        ? { label: 'Run', title: 'Run the editor (Shift+Enter)' }
        : { label: 'Step', title: 'Run the next step (Ctrl+Enter); the editor highlights it' };

export interface ExecutionModeElements {
    readonly runRadio: HTMLButtonElement;
    readonly stepRadio: HTMLButtonElement;
    readonly executeButton: HTMLButtonElement;
}

export interface ExecutionModeSwitch {
    readonly mode: () => ExecutionMode;
    /** Do what the current mode does: run the editor, or take one step. */
    readonly execute: () => void;
}

export const createExecutionModeSwitch = (
    elements: ExecutionModeElements,
    actions: { readonly run: () => void; readonly step: () => void },
    storage: Pick<Storage, 'getItem' | 'setItem'> | null
): ExecutionModeSwitch => {
    let stored: string | null = null;
    try {
        stored = storage?.getItem(EXECUTION_MODE_STORAGE_KEY) ?? null;
    } catch {
        stored = null;
    }
    let mode = readExecutionMode(stored);

    const show = (): void => {
        elements.runRadio.setAttribute('aria-checked', String(mode === 'run'));
        elements.stepRadio.setAttribute('aria-checked', String(mode === 'step'));
        const { label, title } = describeExecutionMode(mode);
        elements.executeButton.textContent = label;
        elements.executeButton.title = title;
    };
    const choose = (next: ExecutionMode): void => {
        mode = next;
        try {
            storage?.setItem(EXECUTION_MODE_STORAGE_KEY, next);
        } catch {
            // Not remembered; the switch still works for this visit.
        }
        show();
    };
    elements.runRadio.addEventListener('click', () => choose('run'));
    elements.stepRadio.addEventListener('click', () => choose('step'));
    const execute = (): void => (mode === 'run' ? actions.run() : actions.step());
    elements.executeButton.addEventListener('click', execute);
    show();
    return { mode: () => mode, execute };
};
