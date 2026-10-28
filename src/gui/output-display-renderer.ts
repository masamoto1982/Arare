import katex from 'katex';
import 'katex/dist/katex.min.css';
import type { Value, ExecuteResult, ExactTerm, Fraction } from '../wasm-interpreter-types';
import { isFailure, readRecordParts, toError } from './interpreter-execution-utils';
import { createEmptyWordsElement } from './vocabulary-state-controller';

export interface DisplayElements {
    outputDisplay: HTMLElement;
    stackDisplay: HTMLElement;
}

export interface DisplayState {
    readonly mainOutput: string;
}

export interface Display {
    readonly init: () => void;
    readonly renderExecutionResult: (result: ExecuteResult) => void;
    readonly renderError: (
        error: Error | { message?: string } | string,
        precedingOutput?: string
    ) => void;
    readonly renderInfo: (text: string, append?: boolean) => void;
    readonly renderDocumentation: (text: string) => void;
    readonly renderStack: (stack: Value[]) => void;
    readonly extractState: () => DisplayState;
}

// A signed decimal string split into its sign and magnitude, so a `-` is
// written once, in front, wherever a number is typeset from its parts.
const splitSign = (numerator: string): { readonly negative: boolean; readonly magnitude: string } => {
    const negative = numerator.startsWith('-');
    return { negative, magnitude: negative ? numerator.slice(1) : numerator };
};

// ── Render budget ───────────────────────────────────────────────────────────
// How much of a stack value the Stack area draws.
//
// The interpreter's materialization ceiling bounds what a generative Word may
// build, not what the host can draw. `1 500000 RANGE` sits well inside that
// ceiling and is an ordinary, correct program — drawn one DOM node per
// element, it would lock the browser tab for tens of seconds with no way to
// abort, clear the editor, or read the result. A safety mechanism that stops
// the interpreter and then hands the host an unbounded drawing job has only
// moved where the program hangs.
//
// So the Stack area draws a bounded prefix of any collection and says, in
// place, how many elements it left out. This is presentation, in the same class
// as the execution step limit: a host control, not a language constraint. The
// value on the stack is whole and unmodified, every Word still sees all of it,
// and `LENGTH` remains the way to ask how long it really is.

/**
 * Elements drawn from any one collection before the rest is summarized. The
 * engine hands the Stack area no more than this of any Vector
 * (`rust/src/agent/stack_view.rs`, `VIEW_ELEMENTS_PER_COLLECTION`), so it
 * may not grow past 100 without that growing too.
 */
export const MAX_RENDERED_ELEMENTS_PER_COLLECTION = 100;

/**
 * Elements drawn across the whole Stack area in one render. A per-collection
 * cap alone still admits a stack of ten thousand short vectors, so the budget
 * is global to the render and every collection draws from it.
 */
export const MAX_RENDERED_ELEMENTS_PER_STACK = 2_000;

export interface RenderBudget {
    remaining: number;
}

export const createRenderBudget = (
    total: number = MAX_RENDERED_ELEMENTS_PER_STACK
): RenderBudget => ({ remaining: Math.max(0, total) });

export interface CollectionRenderPlan {
    /** Leading elements to draw. */
    readonly shown: number;
    /** Elements summarized instead of drawn; 0 means the collection is complete. */
    readonly elided: number;
}

/**
 * Decide how much of a `length`-element collection to draw, and charge the
 * drawn part to the shared budget. A nested collection costs one element in its
 * parent and then draws its own children from the same budget, so the total
 * node count of one render stays bounded whatever the nesting looks like.
 */
export const planCollectionRender = (length: number, budget: RenderBudget): CollectionRenderPlan => {
    const safeLength = Number.isFinite(length) && length > 0 ? Math.floor(length) : 0;
    const shown = Math.min(safeLength, MAX_RENDERED_ELEMENTS_PER_COLLECTION, budget.remaining);
    budget.remaining -= shown;
    return { shown, elided: safeLength - shown };
};

/**
 * The marker that stands in for the undrawn tail. It is deliberately not
 * Ajisai-shaped — a reader must never mistake it for part of the value — and it
 * states the exact count, so the display never implies the collection ends
 * where the drawing does.
 */
export const formatElision = (elided: number): string => `… ${elided} more`;

// ── Canonical text rendering ────────────────────────────────────────────────

// Coloured by nesting depth in CSS (`--bracket-depth-N`).
const createBracketSpan = (bracket: string, depth: number): HTMLSpanElement => {
    const span = document.createElement('span');
    span.className = 'stack-bracket';
    span.dataset.depth = String(depth);
    span.textContent = bracket;
    return span;
};

const checkFractionObject = (value: unknown): Record<string, unknown> | null => {
    if (!value || typeof value !== 'object') return null;
    const candidate = value as Record<string, unknown>;
    if (!('numerator' in candidate) || !('denominator' in candidate)) return null;
    return candidate;
};

// Canonical numeric rendering: every number is a reduced
// numerator/denominator, integers included (`3` -> `3/1`).
const formatNumber = (value: unknown): string => {
    const fraction = checkFractionObject(value);
    if (!fraction) return '?';
    return `${fraction.numerator}/${fraction.denominator}`;
};

// One element of a collection's literal, one level deeper; an element the
// renderer cannot read is drawn as `?` rather than taking the literal down.
const formatElementAt = (item: Value, depth: number): string => {
    try { return formatValue(item, depth); } catch { return '?'; }
};

// Whether a value holds a Record anywhere inside it, so that no bracket
// literal denotes it and the Vector around it is drawn as the phrase that
// builds it.
// A vector cut to its leading elements says whether its left-out elements
// hold one (`truncated.holdsRecord`).
const holdsRecord = (item: Value): boolean =>
    item.type === 'record'
    || (item.type === 'vector' && (
        item.truncated?.holdsRecord === true
        || (Array.isArray(item.value) && (item.value as Value[]).some(holdsRecord))));

// One element of a `COLLECT` phrase: a Symbol is written `[ NAME ] 0 GET`,
// which reads the name out of a literal rather than calling it; everything
// else is drawn as it is.
const formatPhraseElementAt = (item: Value, depth: number): string =>
    item.type === 'symbol' ? `[ ${String(item.value)} ] 0 GET` : formatElementAt(item, depth);

// A Vector renders as source that rebuilds it, matching the engine's own
// renderer (`rust/src/types/display.rs`): a bracket literal when every
// element has a literal, and otherwise — when it holds a Record, which has
// no literal — its elements followed by `n COLLECT`, since inside `[ ]` the
// Record's own phrase would be read as data.
//
// The empty Vector is `[ ]` and not `[]`: a bracket must stand alone
// (`spec/grammar.json`, `bracketMustStandAlone`), so `[]` is a source error
// rather than an empty Vector.
const formatVector = (value: unknown, depth: number): string => {
    if (!Array.isArray(value) || value.length === 0) return '[ ]';
    const items = value as Value[];
    if (items.some(holdsRecord)) {
        return `${items.map((v) => formatPhraseElementAt(v, depth + 1)).join(' ')} ${items.length} COLLECT`;
    }
    return `[ ${items.map((v) => formatElementAt(v, depth + 1)).join(' ')} ]`;
};

// The undrawn tail of a collection, stated as a count rather than drawn. See
// the render budget above for why the Stack area is bounded at all.
const createElisionSpan = (elided: number): HTMLSpanElement => {
    const span = document.createElement('span');
    span.className = 'stack-elision';
    span.textContent = formatElision(elided);
    span.title = `${elided} more element(s) are on the stack but not drawn; LENGTH reports the full length.`;
    return span;
};

// An irrational's normal form Σ c·√r written exactly as the engine writes it
// (`rust/src/types/display.rs::render_algebraic_terms`): one spaceless token,
// terms in the normal form's order, `sqrt(r)` for a unit coefficient,
// `n/d*sqrt(r)` otherwise, and the rational term as `n/d` — the canonical
// display the Stack surface is required to show (spec/gui-semantics.md).
const formatNormalForm = (terms: ReadonlyArray<ExactTerm> | undefined): string | null => {
    if (!terms || terms.length === 0) return null;
    let out = '';
    terms.forEach((term, index) => {
        const { negative, magnitude } = splitSign(term.numerator);
        if (index === 0) {
            if (negative) out += '-';
        } else {
            out += negative ? '-' : '+';
        }
        if (term.radicand === '1') {
            out += `${magnitude}/${term.denominator}`;
        } else if (magnitude === '1' && term.denominator === '1') {
            out += `sqrt(${term.radicand})`;
        } else {
            out += `${magnitude}/${term.denominator}*sqrt(${term.radicand})`;
        }
    });
    return out;
};

const NIL: Value = { type: 'nil' } as Value;

// A collection's literal as DOM: `open`, the drawn children each preceded by
// a space, the elision marker for the rest, a space, `close` — the same
// spacing as the canonical text (`[ 1 2 ]`), which is source that rebuilds
// the value. Every child is drawn under the one render budget. `length` is the
// collection's element count, which a vector cut to its leading elements
// states rather than holds (`truncated.length`).
const renderCollectionNode = (
    node: HTMLElement,
    open: string,
    close: string,
    children: readonly Value[],
    length: number,
    depth: number,
    budget: RenderBudget
): HTMLElement => {
    const { shown, elided } = planCollectionRender(length, budget);
    node.dataset.depth = String(depth);
    node.appendChild(createBracketSpan(open, depth));
    for (let index = 0; index < shown; index++) {
        node.append(' ');
        node.appendChild(renderStackValueNode(children[index]!, depth + 1, budget));
    }
    if (elided > 0) {
        node.append(' ');
        node.appendChild(createElisionSpan(elided));
    }
    node.append(' ');
    node.appendChild(createBracketSpan(close, depth));
    return node;
};

// A Vector that holds a Record, as DOM: its elements each followed by a
// space, the elision marker for the rest, then `n COLLECT` — the phrase
// `formatVector` writes for it. A Symbol element is drawn as `[ NAME ] 0 GET`.
const renderPhraseNode = (
    node: HTMLElement,
    children: readonly Value[],
    length: number,
    depth: number,
    budget: RenderBudget
): HTMLElement => {
    const { shown, elided } = planCollectionRender(length, budget);
    node.dataset.depth = String(depth);
    for (let index = 0; index < shown; index++) {
        if (index > 0) node.append(' ');
        const child = children[index]!;
        if (child.type === 'symbol') {
            const symbol = document.createElement('span');
            symbol.className = 'stack-node';
            symbol.textContent = formatPhraseElementAt(child, depth + 1);
            node.appendChild(symbol);
        } else {
            node.appendChild(renderStackValueNode(child, depth + 1, budget));
        }
    }
    if (elided > 0) {
        if (shown > 0) node.append(' ');
        node.appendChild(createElisionSpan(elided));
    }
    node.append(`${shown > 0 || elided > 0 ? ' ' : ''}${length} COLLECT`);
    return node;
};

const renderStackValueNode = (item: Value, depth: number, budget: RenderBudget): HTMLElement => {
    const node = document.createElement('span');
    node.className = 'stack-node';

    if (item.type === 'vector' && Array.isArray(item.value)) {
        node.classList.add('stack-node-vector');
        const children = item.value as Value[];
        const length = item.truncated?.length ?? children.length;
        if (holdsRecord(item)) return renderPhraseNode(node, children, length, depth, budget);
        return renderCollectionNode(node, '[', ']', children, length, depth, budget);
    }

    // A Record is drawn as the phrase that builds it: its keys and its
    // values, each a Vector drawn as any other, then `RECORD` (see
    // `formatRecord`).
    if (item.type === 'record') {
        const { keys, values } = recordParts(item.value);
        node.classList.add('stack-node-record');
        node.dataset.depth = String(depth);
        node.appendChild(renderStackValueNode({ type: 'vector', value: keys } as Value, depth + 1, budget));
        node.append(' ');
        node.appendChild(renderStackValueNode({ type: 'vector', value: values } as Value, depth + 1, budget));
        node.append(' RECORD');
        return node;
    }

    if (depth === 1) {
        node.dataset.depth = String(depth);
    }
    node.textContent = formatValue(item, depth);
    if (item.type === 'nil') annotateNilNode(node, item);
    if (item.type === 'number') annotatePointNode(node, item);
    return node;
};

// A NIL's reason is its observable content (LANG.VALUES.NIL): `-1 SQRT` is a
// NIL whose reason is `domainMiss`, and a Stack that shows only `NIL`
// leaves the reader to find out why in an Output area that may not be on
// screen. The canonical text stays `NIL` — the display the engine writes —
// and the reason is the tooltip the pointer finds on it. Drawn in the line
// as `NIL · domainMiss`, it read as two values side by side; the tooltip
// keeps the `NIL` so that it reads whole on its own. Exported for
// `output-display-renderer.test.ts`.
export const describeNilNode = (item: Value): string => {
    const reason = item.semantics?.absence?.reason;
    return reason ? `NIL · ${reason}` : 'NIL';
};

// The NIL is set in its own ink (`--color-nil`) so it stands apart from the
// values around it, and a reasoned one invites the pointer: the tooltip
// carries the reason, and `stack-node-nil-reasoned` draws the affordance.
const annotateNilNode = (node: HTMLElement, item: Value): void => {
    const label = describeNilNode(item);
    node.classList.add('stack-node-nil');
    if (label === 'NIL') return;
    node.classList.add('stack-node-nil-reasoned');
    node.title = label;
};

// The two points over zero (LANG.VALUES.EXACT) are numbers, drawn in the
// number's own ink as the pairs they are: `1/0` and `0/0`. Nothing in
// the text says they are the points where the field's laws give way, so a
// reader who has not met them yet would take `1/0` for a malformed fraction.
// Like a reasoned NIL, the node invites the pointer and the tooltip says
// where the point stands; the canonical text stays what the engine writes.
// Exported for `output-display-renderer.test.ts`.
export const describePointNode = (item: Value): string | null => {
    if (item.type !== 'number' || item.semantics?.exactTerms) return null;
    const fraction = checkFractionObject(item.value);
    if (!fraction) return null;
    const numerator = String(fraction.numerator);
    const denominator = String(fraction.denominator);
    if (!/^-?0+$/.test(denominator)) return null;
    const text = `${numerator}/${denominator}`;
    if (/^-?0+$/.test(numerator)) {
        return `${text} · the point with no order: it absorbs every operation`;
    }
    return `${text} · the point at infinity: unsigned, with no order`;
};

const annotatePointNode = (node: HTMLElement, item: Value): void => {
    const label = describePointNode(item);
    if (label === null) return;
    node.classList.add('stack-node-point');
    node.title = label;
};

// A number as the engine displays it. An irrational's `n/d` is only an
// approximation, so its normal form (`semantics.exactTerms`) is written
// instead; a host that sends none gets the approximation marked `≈`, never a
// bare `n/d` that would read as exact.
const formatNumberNode = (item: Value): string => {
    const semantics = item?.semantics;
    const exact = formatNormalForm(semantics?.exactTerms);
    if (exact) return exact;
    const text = formatNumber(item.value);
    return semantics?.approximate === true ? `≈ ${text}` : text;
};

// Exported for `output-display-renderer.test.ts`, which pins these strings
// against the ones `rust/src/types/display.rs` produces. The two renderers
// are separate implementations of one display, and nothing but that test
// stops them drifting.
export const formatValue = (item: Value, depth: number): string => {
    if (!item || !item.type) return '?';

    switch (item.type) {
        case 'number':
            return formatNumberNode(item);
        case 'string':
            return `'${item.value}'`;
        case 'symbol':
            return String(item.value);
        case 'boolean':
            return item.value ? 'TRUE' : 'FALSE';
        case 'vector':
            return formatVector(item.value, depth);
        case 'record':
            return formatRecord(item.value, depth);
        case 'nil':
            return 'NIL';
        default:
            return JSON.stringify(item.value);
    }
};

// A Record (LANG.RECORDS.STRUCTURE) crosses the protocol as two aligned
// arrays of nodes, and renders as the phrase that builds it — its keys and
// its values, each as a Vector, then `RECORD` — which is the same display the
// engine's own stack rendering produces (`rust/src/types/display.rs`) and
// reads back as the same value. The empty Record is `[ ] [ ] RECORD`, which
// needs no case of its own.
//
// A short value array is padded with NIL rather than dropped, because the
// two arrays are aligned by position and a missing slot is the protocol
// having been malformed, not a Record with fewer values than keys — and
// `RECORD` does not admit a length mismatch.
const recordParts = (value: unknown): { keys: Value[]; values: Value[] } => {
    const { keys, values } = readRecordParts(value);
    return { keys, values: keys.map((_, index) => values[index] ?? NIL) };
};

const formatRecord = (value: unknown, depth: number): string => {
    const { keys, values } = recordParts(value);
    return `${formatVector(keys, depth + 1)} ${formatVector(values, depth + 1)} RECORD`;
};

// ── Math view (LaTeX) ───────────────────────────────────────────────────────
// Derives a LaTeX reading of a stack value from its structured protocol form
// (never by parsing display strings).
//
// Presentation only. The canonical display strings (`3/1`, `[ 1/1 2/1 ]`)
// remain the observable semantics the conformance suite checks; the LaTeX
// produced here is an alternate GUI rendering of the same structured `Value`.
// Values without a faithful math reading return `null`, and the caller falls
// back to the canonical text rendering.

// Beyond this many numeric lanes a matrix stops being readable and the
// bracket text form is the better surface.
const MAX_MATH_LANES = 64;

const INTEGER_PATTERN = /^-?\d+$/;

// Digit count at which a numerator or denominator stops being readable as
// a digit string and the math view switches to scientific notation.
const SCIENTIFIC_DIGIT_THRESHOLD = 10;
const MANTISSA_DIGITS = 6;

// Stricter than `checkFractionObject` above: the math view typesets only a
// rational whose parts are integers and whose denominator is not zero, and
// falls back to the canonical text for anything else.
const checkFractionShape = (value: unknown): Fraction | null => {
    if (!value || typeof value !== 'object') return null;
    const candidate = value as { numerator?: unknown; denominator?: unknown };
    const numerator = String(candidate.numerator ?? '');
    const denominator = String(candidate.denominator ?? '');
    if (!INTEGER_PATTERN.test(numerator) || !INTEGER_PATTERN.test(denominator)) return null;
    // A zero denominator is one of the two points over zero
    // (LANG.VALUES.EXACT), a number with no decimal or scientific form.
    // Reject it here so the math view falls back to the canonical text
    // rendering instead of dividing by zero inside `scientificLatex`.
    // Matches INTEGER_PATTERN-allowed forms like "0", "-0" and "0000000000".
    if (/^-?0+$/.test(denominator)) return null;
    return { numerator, denominator };
};

// Scientific reading of a huge ratio: mantissa times a power of ten,
// computed exactly with BigInt long division and prefixed with \approx
// whenever any precision is dropped — the math view never presents a
// truncated value as exact.
const scientificLatex = (numeratorStr: string, denominatorStr: string): string => {
    let numerator = BigInt(numeratorStr);
    let denominator = BigInt(denominatorStr);
    // Defensive: a zero denominator would divide by zero below. Internal callers
    // are pre-filtered by `checkFractionShape`, but `fractionToLatex` is exported
    // and may be called directly, so keep this primitive total.
    if (denominator === 0n) return '\\mathrm{NIL}';
    if (denominator < 0n) {
        denominator = -denominator;
        numerator = -numerator;
    }
    const negative = numerator < 0n;
    if (negative) numerator = -numerator;
    if (numerator === 0n) return '0';

    // Scale so the quotient carries one digit beyond the mantissa, then
    // read mantissa and exponent off the quotient's decimal digits.
    const digitGap = String(numerator).length - String(denominator).length;
    const scale = MANTISSA_DIGITS + 1 - digitGap;
    const scaled = scale >= 0
        ? (numerator * 10n ** BigInt(scale)) / denominator
        : numerator / (denominator * 10n ** BigInt(-scale));
    const dividesExactly = scale >= 0
        ? (numerator * 10n ** BigInt(scale)) % denominator === 0n
        : numerator % (denominator * 10n ** BigInt(-scale)) === 0n;

    const digits = String(scaled);
    const exponent = digits.length - 1 - scale;
    const kept = digits.slice(0, MANTISSA_DIGITS);
    const dropped = digits.slice(MANTISSA_DIGITS);
    const exact = dividesExactly && /^0*$/.test(dropped);

    let significand = kept;
    let exponentOut = exponent;
    if (!exact && dropped.length > 0 && dropped[0]! >= '5') {
        // Round half-up on the first dropped digit; a carry out of the top
        // digit (9.99999... -> 10) bumps the exponent instead.
        const rounded = String(BigInt(kept) + 1n);
        if (rounded.length > kept.length) {
            significand = '1';
            exponentOut = exponent + 1;
        } else {
            significand = rounded;
        }
    }
    significand = significand.replace(/0+$/, '') || '0';

    // Huge components do not imply a huge value (a best rational
    // approximation of sqrt(2) has ten-digit components and the value 1.41…),
    // so a human-scale exponent renders as a plain decimal and only a
    // genuinely large or tiny value gets the power of ten.
    const sign = negative ? '-' : '';
    let body: string;
    if (exponentOut >= 0 && exponentOut <= 5) {
        const integerLength = exponentOut + 1;
        const padded = significand.padEnd(integerLength, '0');
        const integerPart = padded.slice(0, integerLength);
        const fractionalPart = padded.slice(integerLength);
        body = `${sign}${integerPart}${fractionalPart ? `.${fractionalPart}` : ''}`;
    } else if (exponentOut < 0 && exponentOut >= -4) {
        body = `${sign}0.${'0'.repeat(-exponentOut - 1)}${significand}`;
    } else {
        const mantissa = significand.length > 1
            ? `${significand[0]}.${significand.slice(1)}`
            : significand;
        body = mantissa === '1'
            ? `${sign}10^{${exponentOut}}`
            : `${sign}${mantissa} \\times 10^{${exponentOut}}`;
    }
    return exact ? body : `\\approx ${body}`;
};

const checkHugeDigits = (frac: Fraction): boolean => {
    const numeratorDigits = frac.numerator.replace('-', '').length;
    const denominatorDigits = frac.denominator.replace('-', '').length;
    return numeratorDigits >= SCIENTIFIC_DIGIT_THRESHOLD
        || denominatorDigits >= SCIENTIFIC_DIGIT_THRESHOLD;
};

// `3/1` reads as the integer 3; `-3/4` keeps its sign outside the bar.
// Huge components switch to scientific notation so the rendering stays
// inside the Stack area instead of running off its right edge.
export const fractionToLatex = (frac: Fraction): string => {
    if (checkHugeDigits(frac)) return scientificLatex(frac.numerator, frac.denominator);
    if (frac.denominator === '1') return frac.numerator;
    const { negative, magnitude } = splitSign(frac.numerator);
    const body = `\\frac{${magnitude}}{${frac.denominator}}`;
    return negative ? `-${body}` : body;
};

const rowsToMatrixLatex = (rows: string[][]): string => {
    const body = rows.map(row => row.join(' & ')).join(' \\\\ ');
    return `\\begin{bmatrix} ${body} \\end{bmatrix}`;
};

const numberElementToLatex = (item: Value): string | null => {
    if (item.type !== 'number') return null;
    const frac = checkFractionShape(item.value);
    return frac === null ? null : fractionToLatex(frac);
};

const vectorToLatex = (elements: Value[]): string | null => {
    if (elements.length === 0 || elements.length > MAX_MATH_LANES) return null;

    // Homogeneous numeric vector: a one-row matrix.
    const scalarRow = elements.map(numberElementToLatex);
    if (scalarRow.every((tex): tex is string => tex !== null)) {
        return rowsToMatrixLatex([scalarRow]);
    }

    // Rectangular vector-of-numeric-vectors: a rank-2 matrix.
    const rows: string[][] = [];
    let width: number | null = null;
    for (const element of elements) {
        if (element.type !== 'vector' || !Array.isArray(element.value) || element.truncated) return null;
        const row = (element.value as Value[]).map(numberElementToLatex);
        if (!row.every((tex): tex is string => tex !== null)) return null;
        if (width === null) width = row.length;
        if (row.length !== width || width === 0) return null;
        rows.push(row);
    }
    if (rows.reduce((total, row) => total + row.length, 0) > MAX_MATH_LANES) return null;
    return rowsToMatrixLatex(rows);
};

// Σ c·√r as typeset mathematics. A coefficient of one is left implicit, the
// rational term (radicand 1) is drawn as an ordinary fraction, and a negative
// term joins with a minus rather than `+ -`.
const normalFormToLatex = (
    terms: ReadonlyArray<ExactTerm> | undefined
): string | null => {
    if (!terms || terms.length === 0) return null;
    let out = '';
    for (const term of terms) {
        const { negative, magnitude } = splitSign(term.numerator);
        const root = term.radicand === '1' ? '' : `\\sqrt{${term.radicand}}`;
        const unit = magnitude === '1' && term.denominator === '1' && root !== '';
        const coefficient = unit
            ? ''
            : fractionToLatex({ numerator: magnitude, denominator: term.denominator });
        if (out === '') {
            out = `${negative ? '-' : ''}${coefficient}${root}`;
        } else {
            out += ` ${negative ? '-' : '+'} ${coefficient}${root}`;
        }
    }
    return out;
};

// The LaTeX reading of a stack value, or `null` when the canonical text
// rendering is the only faithful surface.
export const valueToLatex = (item: Value): string | null => {
    if (!item || !item.type) return null;

    switch (item.type) {
        case 'number': {
            const semantics = item.semantics;
            // An algebraic irrational carries its exact normal form, and that
            // is what mathematics notation is for: `\sqrt{3}` says the whole
            // value, where the approximation below can only gesture at it.
            const exact = normalFormToLatex(semantics?.exactTerms);
            if (exact !== null) return exact;
            const frac = checkFractionShape(item.value);
            if (frac === null) return null;
            const tex = fractionToLatex(frac);
            // Best rational approximation of an exact irrational under a
            // lossy role (LANG.OBSERVATION.FIREWALL): make the approximation visible. The
            // scientific form may already carry its own \approx.
            const approximate = semantics?.approximate === true;
            return approximate && !tex.startsWith('\\approx') ? `\\approx ${tex}` : tex;
        }
        case 'vector':
            // A vector cut to its leading elements is longer than any matrix
            // drawn here, and its matrix would be missing rows.
            return Array.isArray(item.value) && !item.truncated ? vectorToLatex(item.value as Value[]) : null;
        default:
            return null;
    }
};

// Math view (docs/dev/gui-current-design-memory.md): an alternate KaTeX
// rendering of stack values, derived from the structured protocol form.
// Presentation only — the canonical display strings stay untouched and
// remain the conformance observation; the toggle swaps the view, never
// the value. Output text is never scanned for delimiters.
const MATH_VIEW_STORAGE_KEY = 'ajisai-stack-math-view';

// The canonical protocol strings are the standard rendering; Math view is
// an opt-in alternate so Ajisai's observable surface never depends on
// KaTeX (portability: the GUI stays faithful without it).
const readMathViewPreference = (): boolean => {
    try {
        return globalThis.localStorage?.getItem(MATH_VIEW_STORAGE_KEY) === '1';
    } catch {
        return false;
    }
};

const writeMathViewPreference = (enabled: boolean): void => {
    try {
        globalThis.localStorage?.setItem(MATH_VIEW_STORAGE_KEY, enabled ? '1' : '0');
    } catch {
        // Preference is a convenience; rendering works without persistence.
    }
};

const renderMathValueNode = (item: Value): HTMLElement | null => {
    const tex = valueToLatex(item);
    if (tex === null) return null;
    const node = document.createElement('span');
    node.className = 'stack-node stack-node-math';
    // Tag the top-level node so the stack-top fill's selector
    // (`.stack-item:last-child .stack-node[data-depth="1"]`) matches in LaTeX
    // view exactly as it does for the canonical rendering.
    node.dataset.depth = '1';
    // Trusted markup: KaTeX output for TeX generated from the structured
    // value by valueToLatex, never from user-supplied text.
    node.innerHTML = katex.renderToString(tex, { throwOnError: false });
    return node;
};

// ── The Output and Stack areas ──────────────────────────────────────────────

const formatErrorMessage = (error: Error | { message?: string } | string): string =>
    `Error: ${typeof error === 'string' ? error : error.message || toError(error).message}`;

type OutputKind = 'debug' | 'program' | 'error' | 'info';

// The host writes each `PRINT` emission as one line, so the stream ends in the
// terminator of its last emission. Only that terminator is presentation; every
// other character is the observation itself (LANG.EFFECTS.OUTPUT), so nothing
// else is trimmed: `'' PRINT` is one empty emission and not a run that printed
// nothing, `'  x' PRINT` keeps its indentation, and `'' PRINT 'a' PRINT` keeps
// its first line. Trimming the whole string collapsed all three into their
// neighbours and made the Output projection disagree with the CLI's `output`.
const stripEmissionTerminator = (output: string): string =>
    output.endsWith('\n') ? output.slice(0, -1) : output;

const createSpanElement = (text: string, kind: OutputKind): HTMLSpanElement => {
    const span = document.createElement('span');
    span.className = `output-${kind}`;
    span.textContent = text;
    return span;
};

export const createDisplay = (elements: DisplayElements): Display => {
    let mainOutput = '';
    let mathViewEnabled = readMathViewPreference();
    let lastStack: Value[] = [];

    // The LaTeX toggle: a labeled checkbox at the bottom-right of the Stack
    // area. Checked means the LaTeX (KaTeX) rendering, unchecked the canonical
    // protocol strings. The unchecked mode is deliberately unnamed — a
    // checkbox states only what checking it adds.
    const init = (): void => {
        const panel = elements.stackDisplay.parentElement;
        if (!panel || panel.querySelector('.stack-latex-toggle')) return;

        const wrapper = document.createElement('label');
        wrapper.className = 'stack-latex-toggle';

        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.checked = mathViewEnabled;
        checkbox.addEventListener('change', () => {
            mathViewEnabled = checkbox.checked;
            writeMathViewPreference(mathViewEnabled);
            renderStack(lastStack);
        });

        const caption = document.createElement('span');
        caption.textContent = 'LaTeX';

        wrapper.append(checkbox, caption);
        panel.appendChild(wrapper);
    };

    const appendSpan = (text: string, kind: OutputKind): HTMLSpanElement => {
        const span = createSpanElement(text, kind);
        elements.outputDisplay.appendChild(span);
        return span;
    };

    const renderExecutionResult = (result: ExecuteResult): void => {
        const output = result.output ?? '';
        const program = stripEmissionTerminator(output);

        mainOutput = program;
        elements.outputDisplay.replaceChildren();

        if (output) {
            appendSpan(program, 'program');
        }

        if (!output && !isFailure(result)) {
            appendSpan('OK', 'debug');
        }
    };

    // An error is written *below* whatever the run already printed, never in
    // place of it. `PRINT` is the language's trace tool, and the run that ends
    // in an error is the run whose trace is wanted, so clearing the area first
    // would blank `PRINT` at the one moment it matters most. `precedingOutput`
    // is what the failing run printed before it stopped, which the host
    // reports on the error path too.
    const renderError = (
        error: Error | { message?: string } | string,
        precedingOutput = ''
    ): void => {
        const errorMessage = formatErrorMessage(error);
        const printed = stripEmissionTerminator(precedingOutput);

        elements.outputDisplay.replaceChildren();
        if (precedingOutput) {
            appendSpan(printed, 'program');
            elements.outputDisplay.appendChild(document.createElement('br'));
        }
        mainOutput = precedingOutput ? `${printed}\n${errorMessage}` : errorMessage;

        appendSpan(errorMessage, 'error');
    };

    const renderInfo = (text: string, append = false): void => {
        if (append && elements.outputDisplay.innerHTML.trim() !== '') {
            mainOutput = `${mainOutput}\n${text}`;
            appendSpan('\n' + text, 'info');
        } else {
            mainOutput = text;
            elements.outputDisplay.replaceChildren();
            appendSpan(text, 'info');
        }
    };

    // A Core Word's reference entry, as the host's lookup answered it.
    // Reference text is read rather than run, so
    // it is shown here instead of being written into the editor over whatever
    // the user was writing. `pre-wrap` is already set on the area, so the
    // entry's own line structure survives verbatim.
    const renderDocumentation = (text: string): void => {
        mainOutput = text;
        elements.outputDisplay.replaceChildren();
        appendSpan(text, 'debug');
    };

    const renderStack = (stack: Value[]): void => {
        lastStack = Array.isArray(stack) ? stack : [];
        const display = elements.stackDisplay;
        display.replaceChildren();

        // The clear control follows the same rule the editor's does: it is not
        // drawn when there is nothing to clear. The flag goes on the panel
        // because the button is a sibling of the display, not a child.
        display.parentElement?.classList.toggle('is-empty-stack', lastStack.length === 0);

        if (lastStack.length === 0) {
            display.classList.add('is-empty');
            display.appendChild(createEmptyWordsElement('No values on the stack yet.'));
            return;
        }

        display.classList.remove('is-empty');

        const container = document.createElement('div');
        container.className = 'area-content-flow stack-content-flow';

        // One budget for the whole render: the Stack area draws a bounded
        // amount however the values on it are shaped.
        const budget = createRenderBudget();

        // Spent from the top down, one element per value as a collection's
        // element costs one in its parent: the top is what the next Word
        // takes, so it is always drawn, and the values below the budget are
        // stated as a count under them rather than drawn one node each.
        const drawn: HTMLElement[] = [];
        let index = lastStack.length - 1;
        for (; index >= 0 && budget.remaining > 0; index--) {
            budget.remaining -= 1;
            const item = lastStack[index]!;
            const elem = document.createElement('span');
            elem.className = 'stack-item';
            try {
                const mathNode = mathViewEnabled ? renderMathValueNode(item) : null;
                elem.appendChild(mathNode ?? renderStackValueNode(item, 1, budget));
            } catch {
                console.error(`Error formatting item ${index}`);
                elem.textContent = 'ERROR';
            }
            drawn.push(elem);
        }

        const elidedBelow = index + 1;
        if (elidedBelow > 0) {
            const elem = document.createElement('span');
            elem.className = 'stack-item';
            const marker = document.createElement('span');
            marker.className = 'stack-elision';
            marker.textContent = `${formatElision(elidedBelow)} below`;
            marker.title = `${elidedBelow} more value(s) are on the stack below these but not drawn.`;
            elem.appendChild(marker);
            container.appendChild(elem);
        }
        for (let drawnIndex = drawn.length - 1; drawnIndex >= 0; drawnIndex--) {
            container.appendChild(drawn[drawnIndex]!);
        }

        display.appendChild(container);
    };

    const extractState = (): DisplayState => ({ mainOutput });

    return {
        init,
        renderExecutionResult,
        renderError,
        renderInfo,
        renderDocumentation,
        renderStack,
        extractState
    };
};
