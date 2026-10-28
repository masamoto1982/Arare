// Presentation layer for Ajisai's four observation surfaces (LANG.OBSERVATION.PROJECTIONS:
// Input/π_Input, Output/π_Output, Stack/π_Stack, Dictionary/π_Dict). The
// concrete way those surfaces are made visible on a device is a "Presentation
// Profile" (SPEC Portability Profiles): a labeled transition system over
// visibility configurations. This module holds the initial configuration c0
// (`createLayoutState`), the desktop transition core (`updateDesktopModes`),
// the single-surface core (`resolveNextViewMode`) with the touch gestures
// that drive it, and the execution-driven transition
// (`applyExecutionAreaState`); the spec invariants are checked in
// `gui-layout-state.test.ts`.

import { isMobileViewport } from '../platform/viewport';
import { applyEditorHint, DESKTOP_EDITOR_HINT, MOBILE_EDITOR_HINT } from './editor-hint';

export type ViewMode = 'input' | 'output' | 'stack' | 'dictionary';

// ── Touch gestures ──────────────────────────────────────────────────────────
// Pure recognizers for the touch gestures the mobile presentation uses.
//
// A tap is a touch that goes down and comes up in the same place, and a run of
// taps is a sequence of those close together in both time and space; a bare
// `touchend` is not a tap (the end of a drag-to-select inside the editor, the
// two ends of a pinch, the release of a swipe). This section states that once,
// for both the editor's triple-tap and the Stack/Output double-taps to share.
//
// Thresholds are device tuning, not semantics (Portability Profiles
// "Presentation Profile": gesture thresholds and tap counts have the same
// standing as the execution step limit), so they are parameters here and the
// shipped values live with the bindings that choose them.

export interface GesturePoint {
    readonly x: number;
    readonly y: number;
}

export interface MultiTapOptions {
    /** How long after a tap a further tap still continues the same run. */
    readonly intervalMs: number;
    /** How far a tap may land from the run's first tap and still continue it. */
    readonly movementTolerancePx: number;
}

export interface MultiTapRecognizer {
    /**
     * Register one completed tap and answer how many taps the current run now
     * holds. A tap too late, or too far from where the run started, begins a
     * new run and answers 1.
     */
    readonly registerTap: (point: GesturePoint, at: number) => number;
    /** Abandon the current run; the next tap starts a new one. */
    readonly reset: () => void;
}

export const measureDistance = (from: GesturePoint, to: GesturePoint): number =>
    Math.hypot(to.x - from.x, to.y - from.y);

/**
 * Whether a touch that went down at `from` and came up at `to` stayed put
 * enough to read as a tap rather than as a drag (a text selection, a swipe,
 * a scroll).
 */
export const checkIsStationary = (
    from: GesturePoint,
    to: GesturePoint,
    tolerancePx: number
): boolean => measureDistance(from, to) <= tolerancePx;

export const createMultiTapRecognizer = (options: MultiTapOptions): MultiTapRecognizer => {
    // The anchor is the run's *first* tap, not its previous one: measuring
    // against the previous tap lets a run drift across the screen a tolerance
    // at a time, which is a drag with pauses in it, not a multi-tap.
    let anchor: GesturePoint | null = null;
    let count = 0;
    let lastTapAt = 0;

    const reset = (): void => {
        anchor = null;
        count = 0;
        lastTapAt = 0;
    };

    const registerTap = (point: GesturePoint, at: number): number => {
        const continuesRun = anchor !== null
            && at - lastTapAt <= options.intervalMs
            && checkIsStationary(anchor, point, options.movementTolerancePx);

        if (continuesRun) {
            count += 1;
        } else {
            anchor = point;
            count = 1;
        }

        lastTapAt = at;
        return count;
    };

    return { registerTap, reset };
};

export interface SwipeOptions {
    /** Minimum horizontal travel before a drag reads as a swipe. */
    readonly thresholdPx: number;
}

/**
 * The horizontal direction of a drag, or `null` when it travelled mostly
 * vertically (a scroll) or not far enough to mean anything.
 */
export const detectSwipeDirection = (
    from: GesturePoint,
    to: GesturePoint,
    options: SwipeOptions
): 'left' | 'right' | null => {
    const deltaX = to.x - from.x;
    const deltaY = to.y - from.y;

    if (Math.abs(deltaX) <= Math.abs(deltaY)) return null;
    if (Math.abs(deltaX) <= options.thresholdPx) return null;

    return deltaX > 0 ? 'right' : 'left';
};

// There is deliberately no swipe exemption list: a horizontal drag that starts
// on a textarea, an input or the suggestion panel is still the layout's swipe.
// On a touch screen, dragging sideways across an editor is not a text
// selection — that takes a long-press and then the selection handles, a
// separate gesture entirely — and the editor is most of the Input surface,
// the one the app opens on. The swipe belongs to the layout everywhere.

// ── The page's elements ─────────────────────────────────────────────────────

export interface GUIElements {
    readonly codeInput: HTMLTextAreaElement;
    readonly editorHint: HTMLElement;
    readonly editorClearBtn: HTMLButtonElement;
    readonly stackClearBtn: HTMLButtonElement;
    readonly editorFormatBtn: HTMLButtonElement;
    readonly exportBtn: HTMLButtonElement;
    readonly importBtn: HTMLButtonElement;
    readonly outputDisplay: HTMLElement;
    readonly stackDisplay: HTMLElement;
    readonly coreWordsDisplay: HTMLElement;
    readonly userWordsDisplay: HTMLElement;
    readonly dictionarySearch: HTMLInputElement;
    readonly dictionarySearchClearBtn: HTMLButtonElement;
    readonly dictionarySheetSelect: HTMLSelectElement;
    readonly dictionaryCoreSheet: HTMLElement;
    readonly dictionaryUserSheet: HTMLElement;
    readonly inputArea: HTMLElement;
    readonly outputArea: HTMLElement;
    readonly stackArea: HTMLElement;
    readonly dictionaryArea: HTMLElement;
    readonly leftPanelSelect: HTMLSelectElement;
    readonly rightPanelSelect: HTMLSelectElement;
    readonly mobilePanelSelect: HTMLSelectElement;
    readonly copyOutputBtn: HTMLButtonElement;
    readonly runStatus: HTMLElement;
}

/** The four surfaces the layout shows and hides. */
export type AreaElements = Pick<GUIElements, 'inputArea' | 'outputArea' | 'stackArea' | 'dictionaryArea'>;

// Show the left column's surface and the right column's surface, hiding the
// other two. The single-surface presentation passes the same mode for both
// columns, which leaves exactly that one surface visible.
const applyAreaVisibility = (elements: AreaElements, leftMode: ViewMode, rightMode: ViewMode): void => {
    elements.inputArea.hidden = leftMode !== 'input';
    elements.outputArea.hidden = leftMode !== 'output';
    elements.stackArea.hidden = rightMode !== 'stack';
    elements.dictionaryArea.hidden = rightMode !== 'dictionary';
};

// ── Single-surface (mobile) presentation ────────────────────────────────────

export interface MobileHandler {
    readonly isMobile: () => boolean;
    readonly updateView: (mode: ViewMode) => void;
}

export interface MobileHandlerOptions {
    /** The surface currently showing (LayoutState.currentMode); a swipe moves from it. */
    readonly currentMode: () => ViewMode;
    readonly onModeChange?: (mode: ViewMode) => void;
}

// Device tuning, not semantics (LANG.AUTHORITY.FREEDOM standing), kept out of
// the transition core below.
const SWIPE_THRESHOLD = 50;

// LANG.OBSERVATION.PROJECTIONS (Observation surfaces) / Portability Profiles "Presentation Profile".
// On a single-surface device the four observation surfaces are cycled in this
// fixed order; `VIEW_ORDER` and `resolveNextViewMode` are the pure transition
// core of the mobile presentation profile (a model of the Presentation Profile
// LTS). They are exported so the conformance suite can exercise the shipped
// logic directly.
export const VIEW_ORDER: ViewMode[] = ['input', 'output', 'stack', 'dictionary'];

export const resolveNextViewMode = (currentMode: ViewMode, direction: 'left' | 'right'): ViewMode => {
    const currentIndex = VIEW_ORDER.indexOf(currentMode);
    const nextIndex = direction === 'left'
        ? (currentIndex + 1) % VIEW_ORDER.length
        : (currentIndex - 1 + VIEW_ORDER.length) % VIEW_ORDER.length;
    return VIEW_ORDER[nextIndex]!;
};

export const createMobileHandler = (
    elements: AreaElements,
    options: MobileHandlerOptions
): MobileHandler => {
    // `null` means the gesture in progress is not a candidate swipe: it grew a
    // second finger (a pinch ends two touches, each with a delta of its own).
    // Where it started is not a disqualifier — see the swipe note above.
    let swipeOrigin: GesturePoint | null = null;

    const resolveSwipeGesture = (origin: GesturePoint, end: GesturePoint): void => {
        const direction = detectSwipeDirection(origin, end, { thresholdPx: SWIPE_THRESHOLD });
        if (direction === null) return;
        options.onModeChange?.(resolveNextViewMode(options.currentMode(), direction));
    };

    document.body.addEventListener('touchstart', (e: TouchEvent) => {
        const touch = e.changedTouches[0];
        swipeOrigin = e.touches.length > 1 || !touch
            ? null
            : { x: touch.screenX, y: touch.screenY };
    }, { passive: true });

    document.body.addEventListener('touchend', (e: TouchEvent) => {
        const origin = swipeOrigin;
        swipeOrigin = null;
        if (!isMobileViewport()) return;
        // Fingers still down: this is one release out of a multi-touch
        // gesture, and its travel is that gesture's, not a swipe's.
        if (origin === null || e.touches.length > 0) return;
        const touch = e.changedTouches[0];
        if (touch) resolveSwipeGesture(origin, { x: touch.screenX, y: touch.screenY });
    }, { passive: true });

    return {
        isMobile: isMobileViewport,
        updateView: (mode) => applyAreaVisibility(elements, mode, mode)
    };
};

// ── Layout state and the desktop transition core ────────────────────────────

const LEFT_TAB_MODES: ViewMode[] = ['input', 'output'];
const RIGHT_TAB_MODES: ViewMode[] = ['stack', 'dictionary'];

/** The one record of which surface is showing; every other view of it is derived. */
export interface LayoutState {
    /** Last mode selected. Shared between desktop and mobile; used to re-apply layout on resize and to drive mobile-only behaviors. */
    currentMode: ViewMode;
    /** Desktop left column state. Always 'input' or 'output'. Mobile does not read this. */
    currentLeftMode: ViewMode;
    /** Desktop right column state. Always 'stack' or 'dictionary'. Mobile does not read this. */
    currentRightMode: ViewMode;
}

/** The initial configuration c0. */
export const createLayoutState = (): LayoutState => ({
    currentMode: 'input',
    currentLeftMode: 'input',
    currentRightMode: 'stack'
});

// LANG.OBSERVATION.PROJECTIONS (Observation surfaces) / Portability Profiles "Presentation Profile".
// Pure transition core of the desktop presentation profile: it maps a selection
// of one observation surface onto the (left, right) column configuration. The two
// coupling rules below are the spec's Semantic-coupling invariant (Invariant 6),
// not layout cosmetics — they keep the surfaces that conflict in intent (Output
// vs. Dictionary) out of the reachable configuration space, which is exactly what
// makes the reachable subspace closed under idempotent selection (Invariant 5).
// Exported so the conformance suite can verify the shipped logic is a model of
// the Presentation Profile LTS.
export const updateDesktopModes = (state: LayoutState, mode: ViewMode): void => {
    if (LEFT_TAB_MODES.includes(mode)) {
        state.currentLeftMode = mode;
        if (mode === 'output') {
            // Running code surfaces Output on the left, so pull the right column to Stack so execution results are immediately visible (Presentation Profile Invariant 6.ii: execution is observable).
            state.currentRightMode = 'stack';
        }
    }
    if (RIGHT_TAB_MODES.includes(mode)) {
        state.currentRightMode = mode;
        if (mode === 'dictionary') {
            // Opening the dictionary returns the left column to Input so that clicked words can be inserted (Presentation Profile Invariant 6.iii: selection feeds editing).
            state.currentLeftMode = 'input';
        }
    }
};

export interface ExecutionSurfaceChanges {
    readonly outputChanged: boolean;
    readonly stackChanged: boolean;
    readonly dictionaryChanged: boolean;
}

// The dictionary has two tiers (LANG.DICTIONARY.RESOLUTION), and the
// Dictionary area shows one at a time.
export type DictionarySheetId = 'core' | 'user';

export const isDictionarySheetId = (value: unknown): value is DictionarySheetId =>
    value === 'core' || value === 'user';

export interface ApplyAreaStateDeps {
    readonly elements: GUIElements;
    readonly state: LayoutState;
    readonly mobile: MobileHandler;
    /** Select a dictionary sheet in the selector and show it. */
    readonly showDictionarySheet: (sheetId: DictionarySheetId) => void;
}

const applyMobileAreaState = (deps: ApplyAreaStateDeps, mode: ViewMode): void => {
    deps.mobile.updateView(mode);
    document.body.dataset.activeArea = mode;
    deps.elements.mobilePanelSelect.value = mode;
};

// Draw the desktop columns from the state: the two visible surfaces, the
// body's active-area flag, and the two column selectors.
const syncDesktopView = (deps: ApplyAreaStateDeps): void => {
    const { elements, state } = deps;
    applyAreaVisibility(elements, state.currentLeftMode, state.currentRightMode);
    document.body.dataset.activeArea = state.currentRightMode;
    elements.leftPanelSelect.value = state.currentLeftMode;
    elements.rightPanelSelect.value = state.currentRightMode;
};

const applyAreaState = (deps: ApplyAreaStateDeps, mode: ViewMode): void => {
    if (deps.mobile.isMobile()) {
        applyMobileAreaState(deps, mode);
    } else {
        updateDesktopModes(deps.state, mode);
        syncDesktopView(deps);
    }
};

// Execution-driven transition (distinct from the manual-selection core in
// `updateDesktopModes`): the surfaces an execution touched decide where the
// layout moves, per the desktop intent —
//   * Stack changed       → right column shows Stack.
//   * Output changed       → left column shows Output.
//   * both changed         → left=Output, right=Stack.
//   * neither changed      → both columns stay as they were.
//   * Dictionary changed   → right column shows the changed Words sheet
//                            (Dictionary outranks Stack for the right column,
//                            since defining/importing a word is the more
//                            notable structural change).
// The single-surface (mobile) profile cannot show two surfaces at once, so it
// surfaces the single most notable change in the same priority order
// (Dictionary > Output > Stack); when nothing changed it stays put, mirroring
// the desktop "keep both" rule.
export const applyExecutionAreaState = (
    deps: ApplyAreaStateDeps,
    changes: ExecutionSurfaceChanges
): void => {
    if (deps.mobile.isMobile()) {
        let nextMode: ViewMode | null = null;
        if (changes.dictionaryChanged) {
            nextMode = 'dictionary';
        } else if (changes.outputChanged) {
            nextMode = 'output';
        } else if (changes.stackChanged) {
            nextMode = 'stack';
        }
        if (nextMode) {
            // A run that changed the dictionary defined or deleted a User Word,
            // so the User sheet is the one to show.
            if (nextMode === 'dictionary') deps.showDictionarySheet('user');
            deps.state.currentMode = nextMode;
            applyMobileAreaState(deps, nextMode);
        }
        return;
    }

    if (changes.outputChanged) {
        deps.state.currentLeftMode = 'output';
    }
    if (changes.stackChanged) {
        deps.state.currentRightMode = 'stack';
    }
    if (changes.dictionaryChanged) {
        deps.state.currentRightMode = 'dictionary';
        deps.showDictionarySheet('user');
    }

    deps.state.currentMode = deps.state.currentRightMode;
    syncDesktopView(deps);
};

export const updateEditorPlaceholder = (elements: GUIElements, mobile: MobileHandler): void => {
    applyEditorHint(
        elements.codeInput,
        elements.editorHint,
        mobile.isMobile() ? MOBILE_EDITOR_HINT : DESKTOP_EDITOR_HINT
    );
};

export type LayoutController = {
    readonly setArea: (mode: ViewMode) => void;
    readonly handleResize: () => void;
};

// `setArea` realizes a Presentation Profile transition (SPEC Portability
// Profiles): selecting one observation surface (LANG.OBSERVATION.PROJECTIONS)
// drives the device-appropriate transition core via `applyAreaState`.
export const createLayoutController = (deps: ApplyAreaStateDeps): LayoutController => ({
    setArea: (mode) => {
        deps.state.currentMode = mode;
        applyAreaState(deps, mode);
    },
    handleResize: () => {
        applyAreaState(deps, deps.state.currentMode);
        updateEditorPlaceholder(deps.elements, deps.mobile);
    }
});
