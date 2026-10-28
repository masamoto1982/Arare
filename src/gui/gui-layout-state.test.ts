import {
    beforeEach,
    describe,
    expect,
    it,
    vi
} from 'vitest';
import {
    applyExecutionAreaState,
    type ApplyAreaStateDeps,
    type LayoutState,
    createLayoutState,
    updateDesktopModes,
    resolveNextViewMode,
    VIEW_ORDER,
    type ViewMode,
    checkIsStationary,
    createMultiTapRecognizer,
    detectSwipeDirection,
    measureDistance
} from './gui-layout-state';

const makeElement = () => ({ hidden: false }) as HTMLElement;

const makeDeps = (mobileMode: boolean, state: LayoutState): ApplyAreaStateDeps => ({
    elements: {
        inputArea: makeElement(),
        outputArea: makeElement(),
        stackArea: makeElement(),
        dictionaryArea: makeElement(),
        leftPanelSelect: { value: '' } as HTMLSelectElement,
        rightPanelSelect: { value: '' } as HTMLSelectElement,
        mobilePanelSelect: { value: '' } as HTMLSelectElement,
    } as unknown as ApplyAreaStateDeps['elements'],
    state,
    mobile: {
        isMobile: () => mobileMode,
        updateView: vi.fn((mode: ViewMode) => { state.currentMode = mode; }),
    },
    showDictionarySheet: vi.fn(),
});

describe('applyExecutionAreaState', () => {
    beforeEach(() => {
        vi.stubGlobal('document', { body: { dataset: {} } });
    });

    it('switches the desktop right pane to Stack for stack-only execution changes', () => {
        const state: LayoutState = {
            currentMode: 'dictionary',
            currentLeftMode: 'input',
            currentRightMode: 'dictionary',
        };
        const deps = makeDeps(false, state);

        applyExecutionAreaState(deps, {
            outputChanged: false,
            stackChanged: true,
            dictionaryChanged: false,
        });

        expect(state.currentLeftMode).toBe('input');
        expect(state.currentRightMode).toBe('stack');
        expect(deps.elements.stackArea.hidden).toBe(false);
        expect(deps.elements.dictionaryArea.hidden).toBe(true);
        expect(deps.elements.rightPanelSelect.value).toBe('stack');
    });

    it('keeps desktop panes unchanged when execution changes no observable surface', () => {
        const state: LayoutState = {
            currentMode: 'dictionary',
            currentLeftMode: 'input',
            currentRightMode: 'dictionary',
        };
        const deps = makeDeps(false, state);

        applyExecutionAreaState(deps, {
            outputChanged: false,
            stackChanged: false,
            dictionaryChanged: false,
        });

        expect(state.currentLeftMode).toBe('input');
        expect(state.currentRightMode).toBe('dictionary');
    });

    it('switches the desktop left pane to Output for output-only changes, keeping the right pane', () => {
        const state: LayoutState = {
            currentMode: 'stack',
            currentLeftMode: 'input',
            currentRightMode: 'stack',
        };
        const deps = makeDeps(false, state);

        applyExecutionAreaState(deps, {
            outputChanged: true,
            stackChanged: false,
            dictionaryChanged: false,
        });

        expect(state.currentLeftMode).toBe('output');
        expect(state.currentRightMode).toBe('stack');
        expect(deps.elements.outputArea.hidden).toBe(false);
        expect(deps.elements.inputArea.hidden).toBe(true);
    });

    it('shows Output on the left and Stack on the right when both surfaces change', () => {
        const state: LayoutState = {
            currentMode: 'dictionary',
            currentLeftMode: 'input',
            currentRightMode: 'dictionary',
        };
        const deps = makeDeps(false, state);

        applyExecutionAreaState(deps, {
            outputChanged: true,
            stackChanged: true,
            dictionaryChanged: false,
        });

        expect(state.currentLeftMode).toBe('output');
        expect(state.currentRightMode).toBe('stack');
    });

    it('reveals the changed Words sheet on the desktop right pane for dictionary changes', () => {
        const state: LayoutState = {
            currentMode: 'stack',
            currentLeftMode: 'input',
            currentRightMode: 'stack',
        };
        const deps = makeDeps(false, state);

        applyExecutionAreaState(deps, {
            outputChanged: false,
            stackChanged: false,
            dictionaryChanged: true,
        });

        expect(state.currentRightMode).toBe('dictionary');
        expect(deps.showDictionarySheet).toHaveBeenCalledWith('user');
    });

    it('lets Dictionary outrank Stack for the desktop right pane when both change', () => {
        const state: LayoutState = {
            currentMode: 'input',
            currentLeftMode: 'input',
            currentRightMode: 'stack',
        };
        const deps = makeDeps(false, state);

        applyExecutionAreaState(deps, {
            outputChanged: false,
            stackChanged: true,
            dictionaryChanged: true,
        });

        expect(state.currentRightMode).toBe('dictionary');
        expect(deps.showDictionarySheet).toHaveBeenCalledWith('user');
    });

    it('reveals the changed Words sheet on mobile when the dictionary changes', () => {
        const state: LayoutState = {
            currentMode: 'input',
            currentLeftMode: 'input',
            currentRightMode: 'stack',
        };
        const deps = makeDeps(true, state);

        applyExecutionAreaState(deps, {
            outputChanged: false,
            stackChanged: false,
            dictionaryChanged: true,
        });

        expect(state.currentMode).toBe('dictionary');
        expect(deps.showDictionarySheet).toHaveBeenCalledWith('user');
        expect(deps.elements.mobilePanelSelect.value).toBe('dictionary');
    });

    it('uses Dictionary before Output and Stack on mobile because only one surface is visible', () => {
        const state: LayoutState = {
            currentMode: 'input',
            currentLeftMode: 'input',
            currentRightMode: 'stack',
        };
        const deps = makeDeps(true, state);

        applyExecutionAreaState(deps, {
            outputChanged: true,
            stackChanged: true,
            dictionaryChanged: true,
        });

        expect(state.currentMode).toBe('dictionary');
        expect(deps.elements.mobilePanelSelect.value).toBe('dictionary');
    });
});

// ── merged from presentation-profile.test.ts ──

// Presentation Profile conformance suite (LANG.OBSERVATION.PROJECTIONS "Observation surfaces" +
// Portability Profiles "Presentation Profile").
//
// SPEC formalizes the device-facing programming experience in two layers:
//   * LANG.OBSERVATION.PROJECTIONS — the four observation surfaces are total, pure projections of the
//     runtime state: π_Input, π_Stack, π_Output, π_Dict over the surface set A.
//   * Presentation Profile — how those surfaces are made visible on a device is
//     a labeled transition system M = (C, Σ, →, c0) over visibility
//     configurations c ⊆ A, constrained by six normative invariants.
//
// This suite checks that the *shipped* desktop and single-surface (mobile)
// layouts are two models of that one abstract LTS, by exercising the real
// transition cores (`updateDesktopModes`, `resolveNextViewMode`) rather than a
// re-encoding. Device tuning (breakpoints, swipe thresholds, tap counts, column
// geometry) is implementation freedom (LANG.AUTHORITY.FREEDOM standing) and is not asserted.


// Surface set A (LANG.OBSERVATION.PROJECTIONS). Order is irrelevant here; configurations are sets.
const SURFACES: readonly ViewMode[] = ['input', 'output', 'stack', 'dictionary'];

// A presentation profile as a labeled transition system M = (C, Σ, →, c0).
// `step` is the partial function →; `visible` reads the configuration c ⊆ A;
// `key` gives a configuration/state a canonical id for reachability dedup.
interface PresentationLTS<S> {
    readonly name: string;
    readonly initial: S;
    readonly key: (state: S) => string;
    readonly visible: (state: S) => ReadonlySet<ViewMode>;
    /** Alphabet Σ as concrete operation labels understood by `step`. */
    readonly operations: readonly string[];
    /** The `show(a)` operation label for surface `a` (used by Invariants 2, 5, 6). */
    readonly show: (surface: ViewMode) => string;
    /** The `run` operation label (used by Invariant 6.ii). */
    readonly run: string;
    readonly step: (state: S, op: string) => S;
}

/** Breadth-first enumeration of the reachable configurations C from c0. */
const reachableStates = <S>(lts: PresentationLTS<S>): S[] => {
    const seen = new Map<string, S>();
    const queue: S[] = [lts.initial];
    seen.set(lts.key(lts.initial), lts.initial);
    while (queue.length > 0) {
        const state = queue.shift() as S;
        for (const op of lts.operations) {
            const next = lts.step(state, op);
            const id = lts.key(next);
            if (!seen.has(id)) {
                seen.set(id, next);
                queue.push(next);
            }
        }
    }
    return [...seen.values()];
};

/** Is surface `a` exposable from `state` via some finite operation sequence? */
const canExpose = <S>(lts: PresentationLTS<S>, state: S, surface: ViewMode): boolean => {
    const seen = new Set<string>([lts.key(state)]);
    const queue: S[] = [state];
    while (queue.length > 0) {
        const current = queue.shift() as S;
        if (lts.visible(current).has(surface)) return true;
        for (const op of lts.operations) {
            const next = lts.step(current, op);
            const id = lts.key(next);
            if (!seen.has(id)) {
                seen.add(id);
                queue.push(next);
            }
        }
    }
    return false;
};

const sortedConfig = <S>(lts: PresentationLTS<S>, state: S): string =>
    [...lts.visible(state)].sort().join(',');

// --- Model 1: desktop presentation profile (two columns) --------------------
// State = the full LayoutState; configuration = { left, right }. Selecting a
// surface runs the shipped `updateDesktopModes` coupling core.
const desktopProfile: PresentationLTS<LayoutState> = {
    name: 'desktop',
    initial: createLayoutState(),
    key: (s) => `${s.currentLeftMode}|${s.currentRightMode}`,
    visible: (s) => new Set<ViewMode>([s.currentLeftMode, s.currentRightMode]),
    operations: ['show:input', 'show:output', 'show:stack', 'show:dictionary', 'run'],
    show: (surface) => `show:${surface}`,
    run: 'run',
    step: (s, op) => {
        const next: LayoutState = { ...s };
        // Running surfaces Output (which the coupling core pins next to Stack).
        const mode = op === 'run' ? 'output' : (op.slice('show:'.length) as ViewMode);
        updateDesktopModes(next, mode);
        return next;
    },
};

// --- Model 2: single-surface presentation profile (mobile) ------------------
// State = the one visible surface; configuration = { surface }. Swipes advance/
// retreat through VIEW_ORDER; running moves to Stack (triple-tap, per the mobile
// editor affordances); direct selection shows a surface.
type MobileState = ViewMode;
const mobileProfile: PresentationLTS<MobileState> = {
    name: 'mobile',
    initial: 'input',
    key: (s) => s,
    visible: (s) => new Set<ViewMode>([s]),
    operations: ['advance', 'retreat', 'show:input', 'show:output', 'show:stack', 'show:dictionary', 'run'],
    show: (surface) => `show:${surface}`,
    run: 'run',
    step: (s, op) => {
        if (op === 'advance') return resolveNextViewMode(s, 'left');
        if (op === 'retreat') return resolveNextViewMode(s, 'right');
        if (op === 'run') return 'stack';
        return op.slice('show:'.length) as ViewMode;
    },
};

const PROFILES: ReadonlyArray<PresentationLTS<unknown>> = [
    desktopProfile as PresentationLTS<unknown>,
    mobileProfile as PresentationLTS<unknown>,
];

describe('Presentation Profile LTS — observation surfaces are device-independent', () => {
    it('both profiles range over exactly the four LANG.OBSERVATION.PROJECTIONS surfaces', () => {
        expect([...SURFACES].sort()).toEqual(['dictionary', 'input', 'output', 'stack']);
        expect([...VIEW_ORDER].sort()).toEqual([...SURFACES].sort());
    });
});

describe.each(PROFILES)('Presentation Profile invariants — $name model', (lts) => {
    const states = reachableStates(lts);

    it('has a non-empty reachable configuration space C', () => {
        expect(states.length).toBeGreaterThan(0);
    });

    // Invariant 1 — Partition: each reachable c partitions A into visible/hidden.
    it('Invariant 1 (Partition): visible ⊆ A and visible ⊎ hidden = A', () => {
        for (const state of states) {
            const visible = lts.visible(state);
            for (const surface of visible) expect(SURFACES).toContain(surface);
            const hidden = SURFACES.filter((s) => !visible.has(s));
            expect(visible.size + hidden.length).toBe(SURFACES.length);
            for (const surface of hidden) expect(visible.has(surface)).toBe(false);
        }
    });

    // Invariant 2 — Reachability: every surface is exposable from everywhere.
    it('Invariant 2 (Reachability): every surface is exposable from every c', () => {
        for (const state of states) {
            for (const surface of SURFACES) {
                expect(canExpose(lts, state, surface)).toBe(true);
            }
        }
    });

    // Invariant 3 — Non-emptiness: the user is never shown nothing.
    it('Invariant 3 (Non-emptiness): every reachable c shows ≥ 1 surface', () => {
        for (const state of states) {
            expect(lts.visible(state).size).toBeGreaterThanOrEqual(1);
        }
    });

    // Invariant 4 — Determinism: → is a (partial) function.
    it('Invariant 4 (Determinism): step is a function of (c, σ)', () => {
        for (const state of states) {
            for (const op of lts.operations) {
                expect(lts.key(lts.step(state, op))).toBe(lts.key(lts.step(state, op)));
            }
        }
    });

    // Invariant 5 — Idempotent selection: selecting a visible surface is a no-op.
    // Holds only over the *reachable* space: for desktop, the coupling rules
    // (Invariant 6) keep the conflicting { output, dictionary } config out of C,
    // which is precisely what makes show(visible surface) a no-op everywhere in C.
    it('Invariant 5 (Idempotent selection): show(a) with a ∈ c fixes c', () => {
        for (const state of states) {
            for (const surface of lts.visible(state)) {
                const after = lts.step(state, lts.show(surface));
                expect(sortedConfig(lts, after)).toBe(sortedConfig(lts, state));
            }
        }
    });

    // Invariant 6 — Semantic coupling: visibility tracks intent, not geometry.
    it('Invariant 6.i (Editing is observable): the edit entry shows Input', () => {
        // Selecting Input must make the edit buffer observable.
        const afterEdit = lts.step(lts.initial, lts.show('input'));
        expect(lts.visible(afterEdit).has('input')).toBe(true);
    });

    it('Invariant 6.ii (Execution is observable): run surfaces Stack from every c', () => {
        for (const state of states) {
            const afterRun = lts.step(state, lts.run);
            expect(lts.visible(afterRun).has('stack')).toBe(true);
        }
    });

    it('Invariant 6.iii (Selection feeds editing): π_Input stays reachable from every c', () => {
        for (const state of states) {
            expect(canExpose(lts, state, 'input')).toBe(true);
        }
    });
});

describe('Desktop coupling carves out the conflicting configuration', () => {
    it('{ output, dictionary } is unreachable (Output and Dictionary never coexist)', () => {
        const configs = reachableStates(desktopProfile).map((s) => sortedConfig(desktopProfile, s));
        expect(configs).not.toContain('dictionary,output');
        // The three reachable desktop configurations, for the record.
        expect([...new Set(configs)].sort()).toEqual([
            'dictionary,input',
            'input,stack',
            'output,stack',
        ]);
    });
});

// ── merged from touch-gestures.test.ts ──

const TAP = { intervalMs: 500, movementTolerancePx: 24 };
const SWIPE = { thresholdPx: 50 };

describe('checkIsStationary', () => {
    it('accepts a touch that came up where it went down', () => {
        expect(checkIsStationary({ x: 100, y: 100 }, { x: 100, y: 100 }, 24)).toBe(true);
    });

    it('accepts the wobble of a thumb inside the tolerance', () => {
        expect(checkIsStationary({ x: 100, y: 100 }, { x: 110, y: 108 }, 24)).toBe(true);
    });

    it('rejects a drag past the tolerance — a text selection, not a tap', () => {
        expect(checkIsStationary({ x: 100, y: 100 }, { x: 180, y: 100 }, 24)).toBe(false);
    });

    it('measures diagonally, not per axis', () => {
        // 20px on each axis is inside the tolerance on either axis alone and
        // outside it as a distance (~28px).
        expect(measureDistance({ x: 0, y: 0 }, { x: 20, y: 20 })).toBeCloseTo(28.28, 1);
        expect(checkIsStationary({ x: 0, y: 0 }, { x: 20, y: 20 }, 24)).toBe(false);
    });
});

describe('createMultiTapRecognizer', () => {
    it('counts a run of taps in the same place', () => {
        const recognizer = createMultiTapRecognizer(TAP);
        const here = { x: 50, y: 50 };

        expect(recognizer.registerTap(here, 1000)).toBe(1);
        expect(recognizer.registerTap(here, 1200)).toBe(2);
        expect(recognizer.registerTap(here, 1400)).toBe(3);
    });

    it('starts a new run once the interval lapses', () => {
        const recognizer = createMultiTapRecognizer(TAP);
        const here = { x: 50, y: 50 };

        expect(recognizer.registerTap(here, 1000)).toBe(1);
        expect(recognizer.registerTap(here, 1501)).toBe(1);
    });

    it('treats the interval boundary as still the same run', () => {
        const recognizer = createMultiTapRecognizer(TAP);
        const here = { x: 50, y: 50 };

        recognizer.registerTap(here, 1000);
        expect(recognizer.registerTap(here, 1500)).toBe(2);
    });

    it('starts a new run when a tap lands away from the run', () => {
        const recognizer = createMultiTapRecognizer(TAP);

        expect(recognizer.registerTap({ x: 50, y: 50 }, 1000)).toBe(1);
        expect(recognizer.registerTap({ x: 50, y: 50 }, 1100)).toBe(2);
        expect(recognizer.registerTap({ x: 300, y: 50 }, 1200)).toBe(1);
    });

    it('measures against the first tap, so a run cannot drift across the screen', () => {
        const recognizer = createMultiTapRecognizer(TAP);

        // Each tap is within tolerance of the one before it, and the third is
        // 40px from where the run started: a slow drag with pauses, not a run
        // of taps on one spot.
        expect(recognizer.registerTap({ x: 0, y: 0 }, 1000)).toBe(1);
        expect(recognizer.registerTap({ x: 20, y: 0 }, 1100)).toBe(2);
        expect(recognizer.registerTap({ x: 40, y: 0 }, 1200)).toBe(1);
    });

    it('abandons the run on reset', () => {
        const recognizer = createMultiTapRecognizer(TAP);
        const here = { x: 50, y: 50 };

        recognizer.registerTap(here, 1000);
        recognizer.registerTap(here, 1100);
        recognizer.reset();
        expect(recognizer.registerTap(here, 1200)).toBe(1);
    });

    it('keeps counting past the trigger until the caller resets', () => {
        // The bindings reset on the tap that fires, so this only pins that the
        // recognizer itself does not silently roll over.
        const recognizer = createMultiTapRecognizer(TAP);
        const here = { x: 50, y: 50 };

        recognizer.registerTap(here, 1000);
        recognizer.registerTap(here, 1100);
        recognizer.registerTap(here, 1200);
        expect(recognizer.registerTap(here, 1300)).toBe(4);
    });
});

describe('detectSwipeDirection', () => {
    it('reads a long rightward drag as a right swipe', () => {
        expect(detectSwipeDirection({ x: 10, y: 10 }, { x: 120, y: 20 }, SWIPE)).toBe('right');
    });

    it('reads a long leftward drag as a left swipe', () => {
        expect(detectSwipeDirection({ x: 200, y: 10 }, { x: 40, y: 20 }, SWIPE)).toBe('left');
    });

    it('ignores a drag that did not travel far enough', () => {
        expect(detectSwipeDirection({ x: 10, y: 10 }, { x: 55, y: 10 }, SWIPE)).toBeNull();
    });

    it('ignores the threshold exactly, so the two readings never overlap', () => {
        expect(detectSwipeDirection({ x: 0, y: 0 }, { x: 50, y: 0 }, SWIPE)).toBeNull();
        expect(detectSwipeDirection({ x: 0, y: 0 }, { x: 51, y: 0 }, SWIPE)).toBe('right');
    });

    it('ignores a mostly vertical drag — that is a scroll', () => {
        expect(detectSwipeDirection({ x: 10, y: 10 }, { x: 120, y: 200 }, SWIPE)).toBeNull();
    });

    it('ignores a tap that never moved', () => {
        expect(detectSwipeDirection({ x: 10, y: 10 }, { x: 10, y: 10 }, SWIPE)).toBeNull();
    });
});
