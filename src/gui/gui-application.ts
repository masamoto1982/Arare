import type { AjisaiInterpreter } from '../wasm-interpreter-types';
import { createDisplay, type Display } from './output-display-renderer';
import { createVocabularyManager, type VocabularyManager } from './vocabulary-state-controller';
import { createEditor, createEditorHistory, type Editor } from './code-input-editor';
import { createPersistence, type Persistence } from './interpreter-state-persistence';
import { createExecutionController, type ExecutionController } from './execution-controller';
import type { ExecutionStateView } from './interpreter-execution-utils';
import { WORKER_MANAGER } from '../workers/execution-worker-manager';
import {
    applyExecutionAreaState,
    checkIsStationary,
    createLayoutController,
    createLayoutState,
    createMobileHandler,
    createMultiTapRecognizer,
    isDictionarySheetId,
    updateEditorPlaceholder,
    type ApplyAreaStateDeps,
    type DictionarySheetId,
    type ExecutionSurfaceChanges,
    type GesturePoint,
    type GUIElements,
    type LayoutController,
    type LayoutState,
    type MobileHandler,
    type ViewMode
} from './gui-layout-state';
import { trimSource } from './source-text';
import { syncEditorHintScroll } from './editor-hint';

// ── The page's elements ─────────────────────────────────────────────────────

type ElementConstructor<T extends HTMLElement> = {
    new (...args: unknown[]): T;
    readonly name: string;
};

// Every element the GUI binds to is required at startup: a missing one is a
// broken page, reported once here rather than as a null somewhere later.
function requireElement<T extends HTMLElement>(selector: string, expectedConstructor: ElementConstructor<T>): T {
    const element = document.querySelector(selector);
    if (!element) {
        throw new Error(`Required GUI element ${selector} was not found.`);
    }
    if (!(element instanceof expectedConstructor)) {
        throw new Error(`Required GUI element ${selector} has unexpected type: ${element.constructor.name}.`);
    }
    return element;
}

const cacheElements = (): GUIElements => ({
    codeInput: requireElement('#code-input', HTMLTextAreaElement),
    editorHint: requireElement('#editor-hint', HTMLElement),
    editorClearBtn: requireElement('#editor-clear-btn', HTMLButtonElement),
    stackClearBtn: requireElement('#stack-clear-btn', HTMLButtonElement),
    editorFormatBtn: requireElement('#editor-format-btn', HTMLButtonElement),
    exportBtn: requireElement('#export-btn', HTMLButtonElement),
    importBtn: requireElement('#import-btn', HTMLButtonElement),
    outputDisplay: requireElement('#output-display', HTMLElement),
    stackDisplay: requireElement('#stack-display', HTMLElement),
    coreWordsDisplay: requireElement('#core-words-display', HTMLElement),
    userWordsDisplay: requireElement('#user-words-display', HTMLElement),
    dictionarySearch: requireElement('#dictionary-search', HTMLInputElement),
    dictionarySearchClearBtn: requireElement('#dictionary-search-clear-btn', HTMLButtonElement),
    dictionarySheetSelect: requireElement('#dictionary-sheet-select', HTMLSelectElement),
    dictionaryCoreSheet: requireElement('#dictionary-sheet-core', HTMLElement),
    dictionaryUserSheet: requireElement('#dictionary-sheet-user', HTMLElement),
    inputArea: requireElement('.input-area', HTMLElement),
    outputArea: requireElement('.output-area', HTMLElement),
    stackArea: requireElement('.stack-area', HTMLElement),
    dictionaryArea: requireElement('#dictionary-panel', HTMLElement),
    leftPanelSelect: requireElement('#left-panel-select', HTMLSelectElement),
    rightPanelSelect: requireElement('#right-panel-select', HTMLSelectElement),
    mobilePanelSelect: requireElement('#mobile-panel-select', HTMLSelectElement),
    copyOutputBtn: requireElement('#copy-output-btn', HTMLButtonElement),
    runStatus: requireElement('#run-status', HTMLElement)
});

// ── Event bindings ──────────────────────────────────────────────────────────

// Gesture tuning, in the same class as the mobile breakpoint: a run of taps is
// one gesture while the taps stay inside this much time and this much of the
// screen. The tolerance is generous enough for a thumb that does not land
// twice on the same pixel and tight enough that a drag-to-select is not three
// taps in a row.
const MULTI_TAP_INTERVAL_MS = 500;
const TAP_MOVEMENT_TOLERANCE_PX = 24;
const TAP_OPTIONS = { intervalMs: MULTI_TAP_INTERVAL_MS, movementTolerancePx: TAP_MOVEMENT_TOLERANCE_PX };

// `action` runs on the `count`th click of a run, while `enabled` holds.
const bindClickCount = (
    target: HTMLElement,
    count: number,
    enabled: (e: MouseEvent) => boolean,
    action: () => void
): void => {
    const recognizer = createMultiTapRecognizer(TAP_OPTIONS);
    target.addEventListener('click', (e: MouseEvent) => {
        if (!enabled(e)) return;
        if (recognizer.registerTap({ x: e.clientX, y: e.clientY }, Date.now()) >= count) {
            recognizer.reset();
            action();
        }
    });
};

// Reset is the one operation that throws away the stack *and* the dictionary,
// so it asks first.
const RESET_CONFIRM_MESSAGE = 'Are you sure you want to reset the system?';

type GuiEventBindingContext = {
    readonly elements: GUIElements;
    readonly mobile: MobileHandler;
    readonly layoutState: LayoutState;
    readonly layoutController: LayoutController;
    readonly vocabulary: VocabularyManager;
    readonly display: Display;
    readonly editor: Editor;
    readonly executionController: ExecutionController;
    readonly persistence: Persistence;
    // Discard every value on the stack, leaving the dictionary alone.
    readonly clearStack: () => void;
};

const debounce = <T extends (...args: unknown[]) => void>(
    fn: T,
    delay: number
): ((...args: Parameters<T>) => void) => {
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    return (...args: Parameters<T>) => {
        if (timeoutId) clearTimeout(timeoutId);
        timeoutId = setTimeout(() => fn(...args), delay);
    };
};

function bindLayoutEvents(context: GuiEventBindingContext): void {
    const { elements, mobile, layoutState, layoutController } = context;
    const switchArea = layoutController.setArea;

    for (const select of [elements.leftPanelSelect, elements.rightPanelSelect, elements.mobilePanelSelect]) {
        select.addEventListener('change', () => switchArea(select.value as ViewMode));
    }

    // On mobile a double-tap on Stack shows Output, and on Output shows Input.
    const bindDoubleTapTransition = (target: HTMLElement, activeMode: ViewMode, nextMode: ViewMode): void =>
        bindClickCount(
            target,
            2,
            (e) => mobile.isMobile()
                && layoutState.currentMode === activeMode
                && !(e.target as HTMLElement).closest('button, a'),
            () => switchArea(nextMode)
        );
    bindDoubleTapTransition(elements.stackDisplay, 'stack', 'output');
    bindDoubleTapTransition(elements.outputDisplay, 'output', 'input');

    window.addEventListener('resize', () => {
        layoutController.handleResize();
    });
}

function bindInteractionEvents(context: GuiEventBindingContext): void {
    const { elements, vocabulary, editor, mobile, layoutState, layoutController, display, persistence, executionController, clearStack } = context;
    const switchArea = layoutController.setArea;
    // Session-lived recall of submitted programs, so a run (which clears the
    // editor) and a Reset are both recoverable. See createEditorHistory.
    const history = createEditorHistory();
    // One word search, in the Dictionary area itself, for both presentations.
    const applySearchFilter = (filter: string): void => {
        elements.dictionarySearch.value = filter;
        vocabulary.updateSearchFilter(filter);
    };

    const applySearchInput = debounce(() => {
        applySearchFilter(elements.dictionarySearch.value);
    }, 150);

    elements.dictionarySearch.addEventListener('input', applySearchInput);
    elements.dictionarySearchClearBtn.addEventListener('click', () => applySearchFilter(''));

    elements.editorClearBtn.addEventListener('click', () => editor.clear());
    // Same control, same corner, same gesture as clearing the editor — the
    // Stack area's `×` throws away the values and keeps the dictionary, which
    // is what separates it from Reset.
    elements.stackClearBtn.addEventListener('click', () => clearStack());
    elements.editorFormatBtn.addEventListener('click', () => editor.format());

    // Reformat the editor before running it, so the source that defines words
    // (and therefore the stored definition) is tidied at execution time.
    //
    // The submitted source is recorded before execution, not after: a run that
    // succeeds clears the editor and a Reset clears it too, so recording later
    // would be recording exactly the cases the user can no longer recover.
    const runEditorCode = (): void => {
        editor.format();
        const source = editor.extractValue();
        history.record(source);
        executionController.executeCode(source);
    };

    // Ctrl+Up / Ctrl+Down walk the session's submitted programs back into the
    // editor. The plain arrows are left alone — they are how you move the caret
    // through a multi-line program, and the suggestion panel already uses them
    // to move through its list.
    const recallHistory = (direction: 'older' | 'newer'): void => {
        const recalled = direction === 'older'
            ? history.recallOlder(elements.codeInput.value)
            : history.recallNewer();
        if (recalled === null) return;
        editor.updateValue(recalled);
    };

    elements.outputArea.addEventListener('dblclick', (e: MouseEvent) => {
        if ((e.target as HTMLElement).closest('button, a')) return;
        if (!mobile.isMobile() && layoutState.currentLeftMode === 'output') {
            switchArea('input');
            editor.focus();
        }
    });

    elements.copyOutputBtn.addEventListener('click', (e: MouseEvent) => {
        e.stopPropagation();
        const text = display.extractState().mainOutput;
        navigator.clipboard.writeText(text).then(() => {
            const btn = elements.copyOutputBtn;
            const original = btn.textContent;
            btn.textContent = 'Copied!';
            setTimeout(() => { btn.textContent = original; }, 1500);
        });
    });

    elements.exportBtn.addEventListener('click', () => persistence.exportUserWords());
    elements.importBtn.addEventListener('click', () => persistence.importUserWords());

    elements.codeInput.addEventListener('keydown', (e: KeyboardEvent) => {
        if (e.key === 'Enter' && e.shiftKey) {
            e.preventDefault();
            runEditorCode();
        }
        if (e.key === 'Enter' && e.ctrlKey && !e.altKey && !e.shiftKey) {
            e.preventDefault();
            executionController.executeStep();
        }
        if ((e.key === 'ArrowUp' || e.key === 'ArrowDown') && e.ctrlKey && !e.altKey && !e.shiftKey) {
            e.preventDefault();
            recallHistory(e.key === 'ArrowUp' ? 'older' : 'newer');
        }
        // Shift+Alt+F reformats the editor contents (matches the format button).
        if (e.code === 'KeyF' && e.altKey && e.shiftKey && !e.ctrlKey && !e.metaKey) {
            e.preventDefault();
            editor.format();
        }
    });

    // Triple-tap the editor to Run — on mobile the one route, which the Input
    // surface's own text names (spec/gui-semantics.md, rule 4). A gesture that
    // shares its shape with the OS's own paragraph-select must therefore be
    // deliberate, so a tap here is a touch that went down and came up in the
    // same place, on its own: the end of a drag-to-select and one release out
    // of a pinch are not taps.
    {
        const recognizer = createMultiTapRecognizer(TAP_OPTIONS);
        let touchOrigin: GesturePoint | null = null;

        elements.codeInput.addEventListener('touchstart', (e: TouchEvent) => {
            const touch = e.changedTouches[0];
            if (e.touches.length > 1 || !touch) {
                recognizer.reset();
                touchOrigin = null;
                return;
            }
            touchOrigin = { x: touch.clientX, y: touch.clientY };
        }, { passive: true });

        elements.codeInput.addEventListener('touchend', (e: TouchEvent) => {
            const origin = touchOrigin;
            touchOrigin = null;
            if (!mobile.isMobile()) return;

            const touch = e.changedTouches[0];
            if (origin === null || !touch || e.touches.length > 0) {
                recognizer.reset();
                return;
            }

            const end: GesturePoint = { x: touch.clientX, y: touch.clientY };
            if (!checkIsStationary(origin, end, TAP_MOVEMENT_TOLERANCE_PX)) {
                recognizer.reset();
                return;
            }

            if (recognizer.registerTap(end, Date.now()) >= 3) {
                recognizer.reset();
                // Hand the text back from the OS before running it. The last
                // Word typed may still be the keyboard's unconfirmed
                // composition, and the first two taps of the run are the OS's
                // own double-tap, which selects the word under the finger;
                // either one stays painted over that Word after the Run.
                // Collapsing the selection alone does not end a composition —
                // only leaving the field does — and the Run moves on from the
                // editor anyway, so it gives up focus, keyboard and all.
                const { selectionEnd } = elements.codeInput;
                elements.codeInput.setSelectionRange(selectionEnd, selectionEnd);
                elements.codeInput.blur();
                // Run; the post-execution auto-navigation (applyExecutionAreaState)
                // chooses the destination surface from what actually changed, so
                // we deliberately do not force a switch to Stack here.
                runEditorCode();
            }
        }, { passive: true });
    }

    // There is no desktop triple-click Run. A triple-click selects a line in
    // every text field, and a Run cannot be taken back — it changes the stack
    // and the dictionary — so the gesture that means "select" must not mean
    // "execute". Shift+Enter is the one desktop Run; triple-tap stays on touch,
    // where no line-select gesture competes with it.

    window.addEventListener('keydown', (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
            // This listener captures and stops propagation, so the editor's own
            // Escape branch never sees the key. Dismissing an open suggestion
            // panel takes priority; Abort still gets Escape whenever there is
            // no panel to close.
            if (editor.dismissSuggestions()) {
                e.preventDefault();
                e.stopImmediatePropagation();
                return;
            }
            executionController.abortExecution();
            e.preventDefault();
            e.stopImmediatePropagation();
        }
        if (e.key === 'Enter' && e.ctrlKey && e.altKey) {
            if (confirm(RESET_CONFIRM_MESSAGE)) {
                executionController.executeReset();
            }
            e.preventDefault();
            e.stopImmediatePropagation();
        }
        // Ctrl+Alt+S clears the stack and Ctrl+Alt+E clears the editor. Both are
        // bound on the window rather than their buttons because the Stack area
        // can hold focus, and `e.code` so the binding does not move with the
        // layout. Neither confirms: unlike Reset, Stack clear loses only values
        // (one re-run away) and Editor clear is an ordinary edit that Ctrl+Z
        // takes back. (Recall brings back submitted programs, not unsaved
        // typing, so it never recovered an Editor clear.)
        if (e.code === 'KeyS' && e.ctrlKey && e.altKey && !e.shiftKey && !e.metaKey) {
            clearStack();
            e.preventDefault();
            e.stopImmediatePropagation();
        }
        if (e.code === 'KeyE' && e.ctrlKey && e.altKey && !e.shiftKey && !e.metaKey) {
            editor.clear();
            e.preventDefault();
            e.stopImmediatePropagation();
        }
        // Ctrl+Alt+L looks up the word at the cursor — no typed spelling, no
        // button, since the target comes from the cursor rather than an
        // argument someone could type or click. Silently does nothing when
        // the cursor is not on a word.
        if (e.code === 'KeyL' && e.ctrlKey && e.altKey && !e.shiftKey && !e.metaKey) {
            executionController.lookupWord(editor.getWordAtCursor());
            e.preventDefault();
            e.stopImmediatePropagation();
        }
    }, true);
}

// ── The application ─────────────────────────────────────────────────────────

/**
 * How the Reference's 「Playgroundで開く」 links hand a sample over:
 * `<playground-url>#code=<encodeURIComponent-encoded source>`.
 *
 * Exported because the splash screen also keys off it — arriving this way says
 * the visitor already knows what they came to do (entry-common.ts
 * initSplashScreen()) — and a second copy of the literal would drift.
 */
export const PLAYGROUND_CODE_HASH_MARKER = '#code=';

const RUN_STATUS_DELAY_MS = 300;

export interface GUI {
    readonly init: () => Promise<void>;
}

export const createGUI = (interpreter: AjisaiInterpreter): GUI => {
    const init = async (): Promise<void> => {
        console.log('[GUI] Initializing GUI...');

        const elements = cacheElements();
        const layoutState = createLayoutState();
        const display = createDisplay(elements);
        display.init();

        // The Output area is where persistence, the vocabulary manager and
        // the execution controller all report; each is handed the same two
        // functions rather than its own copy of them.
        const showInfo = (text: string, append: boolean): void => display.renderInfo(text, append);
        const showError = (error: Error): void => display.renderError(error);

        // The dictionary has two tiers (LANG.DICTIONARY.RESOLUTION), so the
        // sheet list is fixed — Core and User — and is a plain <select>, like
        // the two area selectors beside it.
        const sheetSelect = elements.dictionarySheetSelect;
        const showDictionarySheet = (sheetId: DictionarySheetId): void => {
            sheetSelect.value = sheetId;
            elements.dictionaryCoreSheet.hidden = sheetId !== 'core';
            elements.dictionaryUserSheet.hidden = sheetId !== 'user';
        };
        sheetSelect.addEventListener('change', () => {
            if (isDictionarySheetId(sheetSelect.value)) showDictionarySheet(sheetSelect.value);
            void persistence.saveCurrentState();
        });

        const mobile = createMobileHandler(elements, {
            currentMode: () => layoutState.currentMode,
            onModeChange: (mode) => layoutController.setArea(mode)
        });
        updateEditorPlaceholder(elements, mobile);
        syncEditorHintScroll(elements.codeInput, elements.editorHint);

        const layoutDeps: ApplyAreaStateDeps = { elements, state: layoutState, mobile, showDictionarySheet };
        const layoutController = createLayoutController(layoutDeps);

        // The full word list only changes when the vocabulary changes (after an
        // execution). Without this cache the whole set — including a WASM
        // round-trip per query — would be rebuilt on every keystroke. The Core
        // half is the vocabulary manager's one permanent fetch; only the User
        // half is read again when the dictionary is redrawn.
        let autocompleteWordsCache: string[] | null = null;

        const collectAutocompleteWords = (): string[] => {
            if (autocompleteWordsCache) return autocompleteWordsCache;
            // Canonical names only: the Core list carries no alias, and a User
            // Word is addressed by its bare name.
            const coreWords = vocabulary.collectCoreWordNames();
            const userWords = interpreter.collect_user_words_info().map(([name]) => name);
            autocompleteWordsCache = [...new Set([...coreWords, ...userWords])].sort((a, b) => a.localeCompare(b));
            return autocompleteWordsCache;
        };

        // Redraw the Stack and Dictionary panels. With no arguments both are
        // drawn from the interpreter; a run passes the view it has already
        // read back and what it changed, so only the changed panel is rebuilt
        // and the stack is not collected a second time for the drawing.
        const updateAllDisplays = (after?: ExecutionStateView, changes?: ExecutionSurfaceChanges): void => {
            const redrawStack = changes?.stackChanged ?? true;
            const redrawDictionary = changes?.dictionaryChanged ?? true;
            if (redrawDictionary) autocompleteWordsCache = null;
            try {
                if (redrawStack) display.renderStack(after?.stack ?? interpreter.collect_stack());
                if (redrawDictionary) vocabulary.updateUserWords(interpreter.collect_user_words_info());
            } catch (error) {
                console.error('Failed to update display:', error);
                display.renderError(new Error('Failed to update display.'));
            }
        };

        const persistence = createPersistence(interpreter, {
            showError,
            updateDisplays: updateAllDisplays,
            showInfo,
            readActiveDictionarySheet: () => sheetSelect.value,
            runExclusive: (task) => executionController.runExclusive(task)
        });
        await persistence.init();

        const editor = createEditor(elements.codeInput, {
            onSwitchToInputMode: () => layoutController.setArea('input'),
            onRequestSuggestions: () => collectAutocompleteWords()
        });

        const vocabulary = createVocabularyManager(interpreter, elements, {
            // One behaviour in both presentations, as the mobile placeholder
            // advertises (`tap a Dictionary word too`).
            onWordClick: (word) => editor.insertWord(word),
            onBackgroundClick: () => editor.insertWord(' '),
            onBackgroundDoubleClick: () => editor.removeLastWord(),
            onUpdateDisplays: updateAllDisplays,
            onSaveState: () => persistence.saveCurrentState(),
            showInfo,
            showError,
            runExclusive: (task) => executionController.runExclusive(task)
        });

        // Shown only once a run has taken long enough to be noticed, so the
        // everyday run that answers at once does not flash a line over the
        // editor.
        let runStatusTimer: ReturnType<typeof setTimeout> | null = null;
        const showRunStatus = (text: string | null): void => {
            if (runStatusTimer !== null) clearTimeout(runStatusTimer);
            runStatusTimer = null;
            if (text === null) {
                elements.runStatus.hidden = true;
                return;
            }
            runStatusTimer = setTimeout(() => {
                elements.runStatus.textContent = text;
                elements.runStatus.hidden = false;
            }, RUN_STATUS_DELAY_MS);
        };

        const executionController = createExecutionController(interpreter, {
            // Step mode (the sole consumer of this callback) splits the
            // extracted source on whitespace and feeds each piece to the
            // interpreter on its own — the same whitespace-only split the
            // tokenizer itself requires around `[` and `]` (SPEC
            // LANG.SOURCE.TEXT). Formatting first, exactly like `runEditorCode`
            // does for a normal run, guarantees every piece is one the
            // tokenizer accepts even when the author wrote brackets glued to
            // other text.
            extractEditorValue: () => { editor.format(); return editor.extractValue(); },
            readEditorValue: () => editor.extractValue(),
            clearEditor: (switchView) => { editor.clear(switchView); },
            showInfo,
            highlightSourceRange: (start, end) => editor.revealRange(start, end),
            showDocumentation: (text) => display.renderDocumentation(text),
            showError: (error, precedingOutput) => display.renderError(error, precedingOutput),
            showExecutionResult: (result) => display.renderExecutionResult(result),
            updateDisplays: (after, changes) => updateAllDisplays(after, changes),
            saveState: () => persistence.saveCurrentState(),
            fullReset: () => persistence.fullReset(),
            updateView: (mode) => layoutController.setArea(mode),
            updateAfterExecution: (changes) => applyExecutionAreaState(layoutDeps, changes),
            showRunStatus
        });

        // Clearing the stack keeps the dictionary — that is what separates it
        // from Reset — so it is the interpreter's `clear_stack` and nothing
        // else, followed by a redraw and a save. One definition for both
        // routes to it: the Stack area's `×` and `Ctrl+Alt+S`. It has no typed
        // spelling (spec/gui-semantics.md, "Operations without a typed spelling").
        // Pressed during a run, it clears the stack that run leaves.
        const clearStack = (): void => {
            void executionController.runExclusive(() => {
                interpreter.clear_stack();
                updateAllDisplays();
                display.renderInfo('Stack cleared', false);
                void persistence.saveCurrentState();
            });
        };

        const bindingContext: GuiEventBindingContext = {
            elements,
            mobile,
            layoutState,
            layoutController,
            vocabulary,
            display,
            editor,
            executionController,
            persistence,
            clearStack
        };
        bindLayoutEvents(bindingContext);
        bindInteractionEvents(bindingContext);
        vocabulary.renderCoreWords();
        updateAllDisplays();

        // Taken in turn with every run: the keys are already bound, and a run
        // issued while the saved session loads must start from it rather
        // than answer over it.
        const restored = await executionController.runExclusive(() => persistence.loadDatabaseData());
        updateAllDisplays();
        // A saved id that names no sheet leaves the current one showing.
        if (isDictionarySheetId(restored.activeDictionarySheet)) {
            showDictionarySheet(restored.activeDictionarySheet);
        }

        // Appended, not written over: restoring the session above may already
        // have said something the reader needs — the Example Words a first
        // visit loads, or saved Words that could not be restored — and a status
        // line that cleared Output erased it before it could be read.
        try {
            display.renderInfo('Initializing...', true);
            await WORKER_MANAGER.init();
            display.renderInfo('Ready', true);
        } catch (error) {
            console.error('[GUI] Failed to initialize workers:', error);
            display.renderError(new Error(`Failed to initialize parallel execution: ${error}`));
        }

        // A sample handed over by a Reference 「Playgroundで開く」 link, loaded
        // into the editor once; the fragment is stripped so a reload does not
        // load it again.
        const hash = window.location.hash;
        if (hash.startsWith(PLAYGROUND_CODE_HASH_MARKER)) {
            try {
                const code = decodeURIComponent(hash.slice(PLAYGROUND_CODE_HASH_MARKER.length));
                if (trimSource(code) !== '') {
                    editor.updateValue(code);
                    window.history.replaceState(null, '', window.location.pathname + window.location.search);
                }
            } catch (error) {
                console.warn('[GUI] Failed to apply playground code from URL:', error);
            }
        }

        console.log('[GUI] GUI initialization completed');
    };

    return { init };
};
