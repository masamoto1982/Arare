// The cheat sheet shown in the empty editor. Desktop lists keyboard shortcuts;
// mobile lists the equivalent touch gestures.
//
// A textarea placeholder is plain text and cannot draw a key, so the sheet is
// kept as data and rendered twice: as the `placeholder` attribute (what a
// screen reader announces, what drives the :placeholder-shown CSS that hides
// the inline clear/format buttons and the sheet itself once anything is
// typed, and what gives the empty textarea its scroll extent) and as an
// `aria-hidden` overlay where each key is a <kbd> keycap. The placeholder's
// own glyphs are drawn transparent, so the overlay is the one a sighted reader
// sees.
//
// Every operation here lives only as a shortcut (or, for the two clears, also
// a button) — none can be typed into the editor and run. That is deliberate:
// the vocabulary holds nothing that throws away the values a program was
// handed, the text it was typed as, or the dictionary it was defined in, and
// the Input surface holds nothing but the program being written.

/**
 * One line of the sheet: what it does, and how — `keys` is a chord written
 * `Ctrl+Enter`, with ` / ` between alternatives; `touch` is one touch control
 * (a gesture, or a button's glyph), drawn as a keycap like a key, with `where`
 * as plain prose after it; `gesture` is an outcome or an action written as
 * prose only. A line with none of them is a note.
 */
export interface HintEntry {
    readonly label: string;
    readonly keys?: string;
    readonly touch?: string;
    readonly where?: string;
    readonly gesture?: string;
}

export interface HintGroup {
    readonly heading?: string;
    readonly entries: readonly HintEntry[];
}

export interface HintSheet {
    readonly lead: string;
    readonly groups: readonly HintGroup[];
}

export const DESKTOP_EDITOR_HINT: HintSheet = {
    lead: 'Enter code here',
    groups: [
        {
            entries: [
                { label: 'run this code', keys: 'Shift+Enter' },
                { label: 'run one step at a time', keys: 'Ctrl+Enter' },
                { label: 'format this code', keys: 'Shift+Alt+F' },
                { label: 'look up the word at the cursor', keys: 'Ctrl+Alt+L' }
            ]
        },
        {
            entries: [
                { label: 'clear the stack, keep your words', keys: 'Ctrl+Alt+S' },
                { label: 'clear this editor, keep everything', keys: 'Ctrl+Alt+E' },
                // Reset is the one line here a reader acts on expecting an
                // empty dictionary: it clears the stack and the words you
                // defined, then seeds the Example Words back (`fullReset` →
                // `loadExampleWords`). The line says so, so the seeded words
                // do not read as leftovers of your own work.
                { label: 'reset: erase the stack and your words, keep the example words', keys: 'Ctrl+Alt+Enter' }
            ]
        },
        {
            entries: [
                { label: 'bring back your last program', keys: 'Ctrl+Up / Ctrl+Down' },
                { label: 'stop a run or a step', keys: 'Escape' }
            ]
        },
        {
            // The dictionary panel writes into this editor, and the space
            // *between* its buttons is a control of its own: a click there
            // types a space, a double-click takes the last word back.
            // Undocumented, it reads as a misfired click on a word button.
            entries: [
                { label: 'click a word in the dictionary', gesture: 'write it here' },
                { label: 'click the space around them', gesture: 'write a space' },
                { label: 'double-click that space', gesture: 'take back the last word' }
            ]
        }
    ]
};

// The mobile sheet carries the whole touch vocabulary, because on a phone this
// is the only place it is written down. A bar of labelled buttons would take
// its height out of the editor, and the editor is the thing the page is for.
// Prose in the sheet costs nothing — the editor is empty whenever it shows —
// so the sheet is where the teaching goes, and it is allowed to be long. It
// scrolls, and its first lines are the ones a first-time reader needs.
//
// Ordered by what a reader reaches for: run it, move between surfaces, fix the
// text, then what types for you, then the stack's own control. The last block
// is the honest one — five operations have a shortcut and no touch control,
// and saying so beats letting someone hunt for a button that is not there
// (spec/gui-semantics.md, rule 4: nothing a surface is reached by goes
// unsaid, and recall of a submitted program is reached only by Ctrl+Up).
//
// Every line is kept short on purpose, so that neither rendering wraps on a
// 320px phone: a wrapped line turns a tidy two-column sheet into ragged prose.
export const MOBILE_EDITOR_HINT: HintSheet = {
    lead: 'Enter code here',
    groups: [
        {
            entries: [
                { label: 'run it', touch: 'triple-tap', where: 'here' },
                { label: 'change surface', touch: 'swipe' }
            ]
        },
        {
            entries: [
                { label: 'format', touch: '≡', where: 'lower right' },
                { label: 'clear', touch: '×', where: 'upper right' }
            ]
        },
        {
            entries: [
                { label: 'write a word', touch: 'tap', where: 'in Dictionary' },
                { label: 'two letters', gesture: 'suggestions' }
            ]
        },
        {
            entries: [
                { label: 'the stack survives reload' },
                { label: 'empty the stack', touch: '×', where: 'on Stack' }
            ]
        },
        {
            heading: 'keyboard only, for now:',
            entries: [
                { label: 'step', keys: 'Ctrl+Enter' },
                { label: 'stop', keys: 'Escape' },
                { label: 'look up', keys: 'Ctrl+Alt+L' },
                { label: 'recall', keys: 'Ctrl+Up / Ctrl+Down' },
                { label: 'reset', keys: 'Ctrl+Alt+Enter' }
            ]
        }
    ]
};

/** `'Ctrl+Up / Ctrl+Down'` → `[['Ctrl', 'Up'], ['Ctrl', 'Down']]`. */
export const parseKeyChords = (keys: string): string[][] =>
    keys.split(' / ').map((chord) => chord.split('+'));

/** The sheet as the plain text of a `placeholder` attribute. */
export const formatHintSheetText = (sheet: HintSheet): string => {
    const lines: string[] = [sheet.lead];
    for (const group of sheet.groups) {
        lines.push('');
        if (group.heading !== undefined) lines.push(group.heading);
        const width = Math.max(...group.entries.map((entry) => entry.label.length));
        for (const entry of group.entries) {
            const how = entry.keys
                ?? (entry.touch === undefined ? undefined : [entry.touch, entry.where].filter(Boolean).join(' '))
                ?? entry.gesture;
            lines.push(how === undefined ? entry.label : `${entry.label.padEnd(width)} → ${how}`);
        }
    }
    return lines.join('\n');
};

const renderChord = (doc: Document, chord: readonly string[]): HTMLElement => {
    const span = doc.createElement('span');
    span.className = 'editor-hint-chord';
    chord.forEach((key, index) => {
        if (index > 0) span.append('+');
        const kbd = doc.createElement('kbd');
        kbd.textContent = key;
        span.append(kbd);
    });
    return span;
};

const renderHow = (doc: Document, entry: HintEntry): HTMLElement => {
    const how = doc.createElement('dd');
    if (entry.keys !== undefined) {
        parseKeyChords(entry.keys).forEach((chord, index) => {
            if (index > 0) how.append(' / ');
            how.append(renderChord(doc, chord));
        });
    } else if (entry.touch !== undefined) {
        const kbd = doc.createElement('kbd');
        kbd.textContent = entry.touch;
        how.append(kbd);
        if (entry.where !== undefined) how.append(` ${entry.where}`);
    } else if (entry.gesture !== undefined) {
        how.textContent = `→ ${entry.gesture}`;
    }
    return how;
};

/** Replace `container`'s contents with the sheet, keys drawn as keycaps. */
const renderHintSheet = (container: HTMLElement, sheet: HintSheet): void => {
    const doc = container.ownerDocument;
    const lead = doc.createElement('p');
    lead.className = 'editor-hint-lead';
    lead.textContent = sheet.lead;
    const parts: HTMLElement[] = [lead];

    for (const group of sheet.groups) {
        if (group.heading !== undefined) {
            const heading = doc.createElement('p');
            heading.className = 'editor-hint-heading';
            heading.textContent = group.heading;
            parts.push(heading);
        }
        const list = doc.createElement('dl');
        list.className = 'editor-hint-group';
        for (const entry of group.entries) {
            const label = doc.createElement('dt');
            label.textContent = entry.label;
            // A note spans both columns; the empty <dd> keeps the list valid.
            if (entry.keys === undefined && entry.touch === undefined && entry.gesture === undefined) label.className = 'editor-hint-note';
            list.append(label, renderHow(doc, entry));
        }
        parts.push(list);
    }
    container.replaceChildren(...parts);
};

/** Show `sheet` in an empty `textarea`, as its placeholder and as `overlay`. */
export const applyEditorHint = (textarea: HTMLTextAreaElement, overlay: HTMLElement, sheet: HintSheet): void => {
    textarea.placeholder = formatHintSheetText(sheet);
    renderHintSheet(overlay, sheet);
    overlay.scrollTop = 0;
};

/**
 * Keep the overlay scrolled with the textarea. The overlay ignores the pointer
 * — taps and clicks go to the editor beneath it — so it cannot scroll itself;
 * the textarea scrolls over its (transparent) placeholder instead, and the
 * overlay follows in proportion, since the two renderings are not the same
 * height.
 */
export const syncEditorHintScroll = (textarea: HTMLTextAreaElement, overlay: HTMLElement): void => {
    const sync = (): void => {
        if (textarea.value !== '') return;
        const range = textarea.scrollHeight - textarea.clientHeight;
        const overlayRange = overlay.scrollHeight - overlay.clientHeight;
        overlay.scrollTop = range > 0 ? (textarea.scrollTop / range) * overlayRange : 0;
    };
    textarea.addEventListener('scroll', sync, { passive: true });
    textarea.addEventListener('input', () => { overlay.scrollTop = 0; });
};
