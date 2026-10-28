// The Dictionary panel: the Core and User word sheets, their buttons, the
// search filter, and deletion of a User word.

import type { AjisaiInterpreter, CoreWordInfo, ExecuteResult, UserWordInfo } from '../wasm-interpreter-types';
import { isFailure, toError } from './interpreter-execution-utils';

// ── Element builders ────────────────────────────────────────────────────────

const compareWordName = (a: string, b: string): number => {
    const aIsAlpha = /^[A-Za-z]/.test(a);
    const bIsAlpha = /^[A-Za-z]/.test(b);

    if (!aIsAlpha && bIsAlpha) return -1;
    if (aIsAlpha && !bIsAlpha) return 1;

    return a.localeCompare(b);
};

const checkWordMatchesFilter = (wordName: string, filter: string): boolean => {
    if (!filter) return true;
    return wordName.toLowerCase().includes(filter.toLowerCase());
};

const createNoResultsElement = (): HTMLElement => {
    const message = document.createElement('div');
    message.className = 'no-results-message';
    message.textContent = 'No matching words found';
    return message;
};

/// The muted line an empty surface shows in place of its content. The Stack
/// area draws its own empty state with this too.
export const createEmptyWordsElement = (text: string): HTMLElement => {
    const message = document.createElement('div');
    message.className = 'empty-words-message';
    message.textContent = text;
    return message;
};

const BACKGROUND_CLICK_HINT = 'Click the blank area to insert a space';

const registerBackgroundClickListeners = (
    container: HTMLElement,
    onBackgroundClick?: () => void,
    onBackgroundDoubleClick?: () => void
): void => {
    const shouldIgnoreBackgroundInteraction = (): boolean =>
        container.classList.contains('is-empty');

    const isBackgroundClick = (e: MouseEvent): boolean => {
        if (shouldIgnoreBackgroundInteraction()) return false;
        const target = e.target as HTMLElement;
        return !target.closest('.word-button');
    };

    let clickTimer: ReturnType<typeof setTimeout> | null = null;

    if (onBackgroundClick) {
        // The background is a control too, so the screen says so: the
        // browser's own tooltip carries the hint, the way a word's description
        // rides on its button. It is set as the pointer arrives rather than
        // once: an empty list ignores background clicks, so it must not
        // advertise one.
        container.addEventListener('mouseover', () => {
            container.title = shouldIgnoreBackgroundInteraction()
                ? ''
                : BACKGROUND_CLICK_HINT;
        });
        container.addEventListener('click', (e) => {
            if (!isBackgroundClick(e as MouseEvent)) return;
            if (clickTimer) clearTimeout(clickTimer);
            clickTimer = setTimeout(() => {
                onBackgroundClick();
                clickTimer = null;
            }, 200);
        });
    }

    if (onBackgroundDoubleClick) {
        container.addEventListener('dblclick', (e) => {
            if (!isBackgroundClick(e as MouseEvent)) return;
            if (clickTimer) {
                clearTimeout(clickTimer);
                clickTimer = null;
            }
            onBackgroundDoubleClick();
        });
    }
};

const createWordButtonElement = (
    text: string,
    className: string,
    onClick: () => void,
    /**
     * What the word does and an example of using it, shown as the browser's
     * own tooltip: the browser owns the timing, the placement and the
     * dismissal, and the surface reserves no row of its own for a hover
     * display.
     */
    title?: string,
    onContextMenu?: (event: MouseEvent) => void
): HTMLButtonElement => {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = text;
    button.className = className;
    // Always set, even when empty: an absent `title` would inherit the list
    // background's hint, which describes the gap between buttons, not a word.
    button.title = title ?? '';

    button.addEventListener('click', onClick);

    if (onContextMenu) {
        button.addEventListener('contextmenu', (e) => {
            e.preventDefault();
            onContextMenu(e);
        });
    }

    return button;
};

// ── The vocabulary manager ──────────────────────────────────────────────────

export interface VocabularyElements {
    readonly coreWordsDisplay: HTMLElement;
    readonly userWordsDisplay: HTMLElement;
}

export interface VocabularyCallbacks {
    readonly onWordClick: (word: string) => void;
    readonly onBackgroundClick?: () => void;
    readonly onBackgroundDoubleClick?: () => void;
    readonly onUpdateDisplays?: () => void;
    readonly onSaveState?: () => Promise<void>;
    readonly showInfo?: (text: string, append: boolean) => void;
    readonly showError?: (error: Error) => void;
    // Takes the delete in turn with runs, so a run in flight cannot undo it.
    readonly runExclusive?: <T>(task: () => Promise<T>) => Promise<T>;
}

export interface VocabularyManager {
    readonly renderCoreWords: () => void;
    /** Every Core word's canonical name, read from WASM once per page load. */
    readonly collectCoreWordNames: () => string[];
    readonly updateUserWords: (userWordsInfo: UserWordInfo[]) => void;
    readonly updateSearchFilter: (filter: string) => void;
}

// The tooltip text for a User Word: what its author wrote for a reader
// (`#:contract`), or its source when nothing was written. Empty when the
// interpreter has neither.
const lookupUserWordTooltip = (interpreter: AjisaiInterpreter, name: string): string =>
    interpreter.lookup_word_description(name)
    ?? interpreter.lookup_word_definition(name)
    ?? '';

// DEL's refusal of a Word other Words still reference (spec/outcomes.json).
// Matched by category, never by the message, which is display text.
const DEPENDENCY_DELETE_CATEGORY = 'definitionConflict';

// Delete a User word with `DEL`, run on the session interpreter itself. A run
// that fails is not rolled back by the interpreter, so a refused `DEL` left
// the name it was handed on the stack, where the next Run picked it up and
// saved it. A refusal is not a stack edit: the stack is put back as it was.
export const deleteUserWord = async (
    interpreter: AjisaiInterpreter,
    wordName: string
): Promise<ExecuteResult> => {
    const stackBefore = interpreter.snapshot_stack();
    const result = await interpreter.execute(`'${wordName}' DEL`);
    if (isFailure(result)) interpreter.restore_stack_snapshot(stackBefore);
    return result;
};

// A native popover (top-layer placement, light-dismiss on outside click or
// Escape), positioned at the cursor by `renderDeleteContextMenu`.
const createDeleteContextMenuElement = (onDelete: () => void): HTMLDivElement => {
    const menu = document.createElement('div');
    menu.className = 'context-menu';
    menu.popover = 'auto';

    const deleteButton = document.createElement('button');
    deleteButton.type = 'button';
    deleteButton.textContent = 'Delete';
    deleteButton.addEventListener('click', (event) => {
        event.stopPropagation();
        onDelete();
    });

    menu.appendChild(deleteButton);
    document.body.appendChild(menu);
    return menu;
};

export const createVocabularyManager = (
    interpreter: AjisaiInterpreter,
    elements: VocabularyElements,
    callbacks: VocabularyCallbacks
): VocabularyManager => {
    const { onWordClick, onBackgroundClick, onBackgroundDoubleClick, onUpdateDisplays, onSaveState, showInfo, showError } = callbacks;
    const runExclusive = callbacks.runExclusive ?? (<T>(task: () => Promise<T>) => task());
    let activeContextWordName: string | null = null;

    const deleteContextMenu = createDeleteContextMenuElement(() => {
        if (!activeContextWordName) return;
        const selectedWordName = activeContextWordName;
        hideDeleteContextMenu();
        void runExclusive(() => deleteWord(selectedWordName));
    });

    const hideDeleteContextMenu = (): void => {
        if (deleteContextMenu.matches(':popover-open')) deleteContextMenu.hidePopover();
        activeContextWordName = null;
    };

    const renderDeleteContextMenu = (event: MouseEvent, wordName: string): void => {
        activeContextWordName = wordName;
        deleteContextMenu.style.left = `${event.clientX}px`;
        deleteContextMenu.style.top = `${event.clientY}px`;
        if (deleteContextMenu.matches(':popover-open')) deleteContextMenu.hidePopover();
        deleteContextMenu.showPopover();
    };

    // Reset the tracked word when the popover is light-dismissed (outside click /
    // Escape) so a later Delete can't act on a stale selection.
    deleteContextMenu.addEventListener('toggle', (event) => {
        if ((event as ToggleEvent).newState === 'closed') {
            activeContextWordName = null;
        }
    });

    for (const container of [elements.coreWordsDisplay, elements.userWordsDisplay]) {
        registerBackgroundClickListeners(container, onBackgroundClick, onBackgroundDoubleClick);
    }

    let searchFilter = '';
    let cachedUserWords: UserWordInfo[] = [];
    // Core words are fixed once WASM is loaded, so they are sorted once.
    let sortedCoreWordsCache: CoreWordInfo[] | null = null;

    const getSortedCoreWords = (): CoreWordInfo[] => {
        sortedCoreWordsCache ??= [...interpreter.collect_core_words_info()]
            .sort((a, b) => compareWordName(a[0], b[0]));
        return sortedCoreWordsCache;
    };

    // The Core half of the autocomplete list. Core words never change, so
    // the one fetch behind `getSortedCoreWords` serves this too.
    const collectCoreWordNames = (): string[] => getSortedCoreWords().map(([name]) => name);

    // A referenced word is not deletable, and there is no way to override that:
    // the only route is to delete the dependents first. The interpreter names
    // the referencing words in its message, so it is surfaced as-is.
    const deleteWord = async (wordName: string): Promise<void> => {
        try {
            const result = await deleteUserWord(interpreter, wordName);
            if (isFailure(result)) {
                const message = result.message || 'Unknown error';
                if (result.errorInfo?.category === DEPENDENCY_DELETE_CATEGORY) {
                    showInfo?.(message, true);
                } else {
                    showError?.(new Error(`Failed to delete word: ${message}`));
                }
                return;
            }

            onUpdateDisplays?.();
            await onSaveState?.();
            showInfo?.(`Word '${wordName}' deleted`, true);
        } catch (error) {
            showError?.(toError(error));
        }
    };

    const renderCoreWords = (): void => {
        try {
            const container = elements.coreWordsDisplay;
            container.replaceChildren();
            container.classList.remove('is-empty');

            const matched = getSortedCoreWords().filter(([name]) =>
                checkWordMatchesFilter(name, searchFilter)
            );

            const fragment = document.createDocumentFragment();
            for (const [name, summary, syntaxExample] of matched) {
                // One authored line on what the Word is, then how it is called.
                const hoverText = [summary, syntaxExample].filter(Boolean).join('\n');
                fragment.appendChild(createWordButtonElement(
                    name,
                    'word-button core',
                    () => onWordClick(name),
                    hoverText
                ));
            }
            container.appendChild(fragment);

            if (searchFilter && matched.length === 0) {
                container.classList.add('is-empty');
                container.appendChild(createNoResultsElement());
            }
        } catch (error) {
            console.error('Failed to render core words:', error);
        }
    };

    const renderUserWords = (): void => {
        const container = elements.userWordsDisplay;
        const words = cachedUserWords;
        container.replaceChildren();

        const filteredWords = words.filter(([name]) =>
            checkWordMatchesFilter(name, searchFilter)
        );
        const sortedFiltered = [...filteredWords].sort(([a], [b]) =>
            compareWordName(a, b)
        );

        const fragment = document.createDocumentFragment();
        for (const [name, hasDependents] of sortedFiltered) {
            // Another User Word calls it, so DEL refuses it until that caller
            // is gone; it is coloured apart.
            const className = hasDependents
                ? 'word-button dependency'
                : 'word-button non-dependency';
            fragment.appendChild(createWordButtonElement(
                name,
                className,
                () => onWordClick(name),
                // Read at render rather than on hover: a tooltip has to carry
                // its text before the pointer arrives.
                lookupUserWordTooltip(interpreter, name),
                (event) => renderDeleteContextMenu(event, name)
            ));
        }
        container.appendChild(fragment);

        // A filter that matches nothing says so, whether or not there are
        // words to match: an empty User sheet under a filter used to show
        // neither message.
        if (searchFilter && filteredWords.length === 0) {
            container.classList.add('is-empty');
            container.appendChild(createNoResultsElement());
            return;
        }

        if (!searchFilter && words.length === 0) {
            container.classList.add('is-empty');
            container.appendChild(createEmptyWordsElement('No user words defined yet.'));
            return;
        }

        container.classList.remove('is-empty');
    };

    const updateUserWords = (userWordsInfo: UserWordInfo[]): void => {
        cachedUserWords = userWordsInfo;
        renderUserWords();
    };

    const updateSearchFilter = (filter: string): void => {
        searchFilter = filter.trim();
        renderCoreWords();
        renderUserWords();
    };

    return {
        renderCoreWords,
        collectCoreWordNames,
        updateUserWords,
        updateSearchFilter
    };
};
