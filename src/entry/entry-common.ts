// The playground's entry module: index.html imports this file, and it boots
// the application once the document is ready.

import { getPlatform } from '../platform';
import { createGUI, PLAYGROUND_CODE_HASH_MARKER } from '../gui/gui-application';
import { toError } from '../gui/interpreter-execution-utils';
import { EXECUTION_TIMEOUT_MS } from '../workers/execution-contract';
import { initWasm } from '../workers/execution-worker-manager';
import { parseHostProfile, type AjisaiInterpreter } from '../wasm-interpreter-types';

declare const __AJISAI_BUILD_TIMESTAMP__: string;
declare const __AJISAI_RELEASE_VERSION__: string;

// Injected by vite.config.ts (`define`).
const BUILD_TIMESTAMP = __AJISAI_BUILD_TIMESTAMP__;

const COMPARE_NOTE =
    'Compare against the repository when the Playground disagrees with the specification.';

/** The build half of the badge tooltip's detail. */
function buildDetailLines(): string[] {
    return [`Version: ${__AJISAI_RELEASE_VERSION__}`, `Build: ${BUILD_TIMESTAMP}`, COMPARE_NOTE];
}

/**
 * Apply a mutation to every element matching any of the given selectors.
 *
 * Every match, not just the first: the header and the splash screen (see
 * initSplashScreen()) each carry their own `.version` badge, so a
 * `querySelector` here would write to whichever comes first in the document
 * — the splash's copy, which is then removed on dismissal — and silently
 * leave the header's behind.
 */
function setLabelForAll(selectors: string[], mutate: (el: HTMLElement) => void): void {
    for (const selector of selectors) {
        document.querySelectorAll<HTMLElement>(selector).forEach(mutate);
    }
}

/**
 * State which build is deployed. (The `Playground` badge itself is written
 * in index.html, matching the Reference header's own `リファレンス` badge.)
 *
 * Which build matters because the Playground is a separately deployed
 * artifact and a deploy can be stranded (see the `workflow_dispatch` note in
 * `.github/workflows/build.yml`). When that happens the symptom is a site
 * whose behaviour disagrees with the specification, and with no version
 * anywhere there is nothing to compare against.
 *
 * The header says it on hover and the splash says it in plain text, which is
 * what makes the pair work: a tooltip does not exist on a touch device, so
 * the splash is how the version reaches one — and once it has, the header
 * does not need to keep spending its brand row on a stamp nobody reads twice.
 * The splash keeps it to one line so it still fits a phone screen at a glance.
 */
function setBuildVersionLabel(): void {
    // Until the interpreter is up this is all there is to tell; the host
    // profile rewrites this tooltip with its ceilings appended.
    setPlaygroundBadgeTooltip(buildDetailLines());

    // The splash states which build in a single line; the compare note is
    // for someone already chasing a discrepancy, so it stays in the tooltip.
    setLabelForAll(['#splash-version-line'], (el) => {
        el.hidden = false;
        el.textContent = `${__AJISAI_RELEASE_VERSION__} · build ${BUILD_TIMESTAMP}`;
    });
}

/**
 * Put the technical detail on the header's `Playground` badge, which is the
 * only place it remains once the splash is dismissed.
 *
 * Always the complete text, never an append: the two halves arrive at
 * different moments, so a caller that added only its own half would leave the
 * tooltip reading differently depending on which ran last.
 *
 * The splash's own badge is deliberately left alone — the splash is a
 * one-time summary, not the place to dig into the detail.
 */
function setPlaygroundBadgeTooltip(lines: string[]): void {
    setLabelForAll(['header .version'], (el) => {
        el.title = lines.join('\n');
    });
}

/**
 * Say which resource profile this host applies, and what its ceilings are.
 *
 * LANG.MACHINE.LIMITS makes limits a host safety control rather than value semantics, so
 * two conforming hosts legitimately enforce different ceilings — and they do:
 * `0 100001 RANGE` materializes here and is refused under the MCP agent
 * profile (`resourceLimitExceeded`, naming `materializedElements`). A ceiling
 * is never a value, so the difference is a named failure on one host and an
 * answer on the other, never two answers; it is only a trap when a host does
 * not disclose what it applies, so this one does.
 * See docs/dev/mcp-host-profiles.md for the comparison.
 */
function setHostProfileLabel(interpreter: AjisaiInterpreter): void {
    // Best effort throughout: this is a label, and nothing below it may throw
    // into initializeApplication() and take the GUI down with it. The call
    // itself sits inside the guard for the same reason as the parse.
    let reported: string;
    try {
        reported = interpreter.host_profile();
    } catch {
        return;
    }
    const profile = parseHostProfile(reported);
    if (!profile) return;
    // The MCP server's profile, beside this host's: the two differ by 10x to
    // 120x on purpose, and a program tried here and then handed to an agent
    // met that difference as a surprise, because only the Reference and the
    // MCP README said so. Best effort like everything else here — an engine
    // without the method simply shows this host's table alone.
    let agentProfile: ReturnType<typeof parseHostProfile> = null;
    try {
        agentProfile = parseHostProfile(interpreter.agent_host_profile());
    } catch {
        agentProfile = null;
    }
    const text = `resource limits: ${profile.profile}`;
    const limitLines = [
        ...Object.entries(profile.limits).map(([name, value]) => {
            const agentValue = agentProfile?.limits[name];
            const beside =
                typeof agentValue === 'number' && agentValue !== value
                    ? ` (${agentProfile?.profile}: ${agentValue.toLocaleString()})`
                    : '';
            return `${name}: ${value.toLocaleString()}${beside}`;
        }),
        // The wall-clock guard is this host's alone and is not one of the
        // interpreter's ceilings, so `host_profile()` cannot report it — and a
        // list that omits the guard most likely to stop a long run reads as a
        // complete list that is wrong.
        `executionTimeoutMs: ${EXECUTION_TIMEOUT_MS.toLocaleString()} (wall clock, this host only)`,
    ];
    if (agentProfile) {
        limitLines.push(
            `A program run through the MCP server (${agentProfile.profile}) meets the ceilings in parentheses instead.`
        );
    }

    // Now that the ceilings are known, the badge's tooltip can state the whole
    // thing: build detail, then the full ceiling table.
    setPlaygroundBadgeTooltip([...buildDetailLines(), '', text, ...limitLines]);

    // The splash names the profiles only. The full table (eleven rows) pushed
    // everything else off a phone screen; what a first visit needs is to know
    // that this host and the MCP server enforce different ceilings at all.
    setLabelForAll(['#splash-host-profile'], (el) => {
        el.hidden = false;
        el.textContent =
            agentProfile && agentProfile.profile !== profile.profile
                ? `Limits: ${profile.profile} · MCP: ${agentProfile.profile}`
                : `Limits: ${profile.profile}`;
    });
}

/**
 * First-visit walkthrough: shows the Ajisai logo, then auto-reveals a curated
 * two-line summary — which build, and which resource profiles apply (see
 * setBuildVersionLabel() / setHostProfileLabel() above, which fill it in).
 * The full detail stays on the header badge's hover title; the splash keeps
 * only what fits a phone screen at a glance. Dismissed by click, tap, or any key press — never a wait the user is
 * forced to sit through — and shown at most once per session so reloading
 * mid-session to retry a program never re-interrupts. Runs independently of
 * WASM/GUI startup: the splash is pure DOM and does not gate
 * `initializeApplication()`.
 *
 * It is for an arrival with nothing else in mind. Someone who followed a
 * Reference sample's 「Playgroundで開く」 link has already said what they came
 * to do, so that arrival skips it — which is also the one route where it would
 * otherwise repeat without end: the Reference opens in its own tab, and
 * `sessionStorage` is per-tab, so the dismissal never reaches it.
 */
function initSplashScreen(): void {
    const splash = document.querySelector<HTMLElement>('#splash-screen');
    if (!splash) return;

    const SESSION_KEY = 'ajisai-splash-seen';
    const markSeen = (): void => {
        try {
            sessionStorage.setItem(SESSION_KEY, '1');
        } catch {
            // Best effort only; worst case the splash reappears next reload.
        }
    };

    // Read before the GUI strips the hash (gui-application.ts init, which
    // only does so once the wasm is up — long after this runs at
    // DOMContentLoaded). Marked seen as well: the GUI replaces the URL
    // without the fragment, so a reload in this tab arrives with no hash, and
    // that reload is exactly the mid-session one the splash must not
    // interrupt.
    if (window.location.hash.startsWith(PLAYGROUND_CODE_HASH_MARKER)) {
        markSeen();
        splash.remove();
        return;
    }

    try {
        if (sessionStorage.getItem(SESSION_KEY) === '1') {
            splash.remove();
            return;
        }
    } catch {
        // sessionStorage unavailable (e.g. private browsing) - show the splash
        // every time; harmless, since it is never more than one extra tap.
    }

    splash.hidden = false;
    splash.focus();

    const DETAIL_REVEAL_DELAY_MS = 900;
    window.setTimeout(() => splash.classList.add('splash-detail-visible'), DETAIL_REVEAL_DELAY_MS);

    let dismissed = false;
    const dismiss = (): void => {
        if (dismissed) return;
        dismissed = true;
        markSeen();
        splash.classList.add('splash-dismissing');
        splash.addEventListener('transitionend', (event) => {
            // The detail rows' own reveal transitions bubble up here too, and
            // one still in flight ends before this fade does — taking the
            // splash off screen mid-fade if it is allowed to answer for it.
            if (event.target !== splash) return;
            splash.remove();
        });
        // Fallback in case the transition never fires (e.g. reduced-motion
        // environments where the opacity change is instant).
        window.setTimeout(() => splash.remove(), 400);
    };

    splash.addEventListener('click', dismiss);
    splash.addEventListener('keydown', dismiss);
}

async function initializeApplication(): Promise<void> {
    console.log('[Main] Starting Ajisai application...');

    try {
        console.log('[Main] Initializing WASM...');
        const wasm = await initWasm();
        if (!wasm) {
            throw new Error('WASM initialization failed. Application cannot start.');
        }

        console.log('[Main] Creating main thread interpreter...');
        const interpreter = new wasm.AjisaiInterpreter();
        setHostProfileLabel(interpreter);

        console.log('[Main] Initializing GUI...');
        await createGUI(interpreter).init();

        console.log('[Main] Application initialization completed successfully');
    } catch (error) {
        console.error('[Main] Application startup failed:', error);
        const outputDisplay = document.getElementById('output-display');
        if (outputDisplay) {
            const errorSpan = document.createElement('span');
            errorSpan.className = 'output-error';
            errorSpan.textContent = `Application startup failed: ${toError(error).message}`;
            outputDisplay.replaceChildren(errorSpan);
        }
    }
}

// One wait for the document, through the platform's runtime seam. The former
// entry-bootstrap module waited for DOMContentLoaded itself and then called
// this, which waited again (and found the document already interactive), so
// the second wait always resolved synchronously; dropping it changes nothing
// about when the three calls below run.
getPlatform().runtime.onReady(() => {
    initSplashScreen();
    setBuildVersionLabel();
    void initializeApplication();
});
