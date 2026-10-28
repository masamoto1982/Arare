// The Stack panel and the engine are two implementations of one display.
//
// A Record crosses the protocol as two aligned arrays of nodes
// (LANG.OBSERVATION.PROTOCOL), so the panel re-renders it from those arrays
// rather than receiving the engine's string. Nothing but this test stops the
// two drifting, and a display is expected to be source: the empty Vector is
// `[ ]`, because `[]` is a source error — a bracket must stand alone
// (`spec/grammar.json`, `bracketMustStandAlone`).
//
// Every expectation below is a string captured from the engine by running the
// named program through `ajisai run`, not one written by hand to match the
// panel.

import { afterAll, beforeAll, describe, expect, test } from 'vitest';
import {
    describeNilNode,
    describePointNode,
    formatValue,
    MAX_RENDERED_ELEMENTS_PER_COLLECTION,
    MAX_RENDERED_ELEMENTS_PER_STACK,
    createDisplay,
    createRenderBudget,
    formatElision,
    planCollectionRender,
    fractionToLatex,
    valueToLatex
} from './output-display-renderer';
import type { Value } from '../wasm-interpreter-types';
import { frac, num, rec, str, vec } from '../test-support';

type Node = Parameters<typeof formatValue>[0];


const render = (node: Node): string => formatValue(node, 0);

describe('a Record renders as the phrase that builds it', () => {
    test("[ 'x' 'y' ] [ 1 2 ] RECORD", () => {
        expect(render(rec([str('x'), str('y')], [num(1), num(2)]))).toBe(
            "[ 'x' 'y' ] [ 1/1 2/1 ] RECORD"
        );
    });

    test('the empty Record is two empty Vectors and RECORD', () => {
        expect(render(rec([], []))).toBe('[ ] [ ] RECORD');
    });

    test('a Record whose value is a Vector — the shape GROUP answers', () => {
        expect(
            render(rec([str('a'), str('b')], [vec(num(1), num(3)), vec(num(2))]))
        ).toBe("[ 'a' 'b' ] [ [ 1/1 3/1 ] [ 2/1 ] ] RECORD");
    });
});

describe('a Vector holding a Record is a COLLECT phrase', () => {
    // A Record has no literal, so inside `[ ]` its phrase would be data; the
    // Vector is written as its elements followed by `n COLLECT`, which reads
    // back as itself.
    test('one Record', () => {
        expect(render(vec(rec([str('a')], [num(1)])))).toBe("[ 'a' ] [ 1/1 ] RECORD 1 COLLECT");
    });

    test('a Record beside an ordinary value', () => {
        expect(render(vec(num(1), rec([str('a')], [num(2)])))).toBe(
            "1/1 [ 'a' ] [ 2/1 ] RECORD 2 COLLECT"
        );
    });

    test('the phrases nest', () => {
        expect(render(vec(vec(rec([str('a')], [num(1)]))))).toBe(
            "[ 'a' ] [ 1/1 ] RECORD 1 COLLECT 1 COLLECT"
        );
    });
});

describe('a Symbol inside a COLLECT phrase is read out of a literal', () => {
    const sym = (name: string): Value => ({ type: 'symbol', value: name } as Value);

    test('beside a Record it is written [ NAME ] 0 GET, not called', () => {
        expect(render(vec(rec([str('k')], [num(1)]), sym('V')))).toBe(
            "[ 'k' ] [ 1/1 ] RECORD [ V ] 0 GET 2 COLLECT"
        );
    });

    test('inside a bracket literal it stays bare', () => {
        expect(render(vec(sym('V'), num(1)))).toBe('[ V 1/1 ]');
    });
});

describe('a Vector of ordinary values is a literal', () => {
    test('a flat Vector', () => {
        expect(render(vec(num(1), num(2), num(3)))).toBe('[ 1/1 2/1 3/1 ]');
    });

    test('a nested Vector', () => {
        expect(render(vec(vec(num(1)), vec(num(2))))).toBe('[ [ 1/1 ] [ 2/1 ] ]');
    });

    test('the empty Vector is spaced, because a bracket must stand alone', () => {
        expect(render(vec())).toBe('[ ]');
    });
});

describe('an irrational renders as the engine writes it', () => {
    // Captured from `ajisai agent compute` (`stackDisplay`), with the node's
    // `semantics.exactTerms` as the input.
    const irrational = (...terms: [string, string, string][]): Node =>
        ({
            type: 'number',
            value: { numerator: '0', denominator: '1' },
            semantics: {
                approximate: true,
                exactTerms: terms.map(([numerator, denominator, radicand]) => ({
                    numerator,
                    denominator,
                    radicand
                }))
            }
        }) as Node;

    test.each([
        ['2 SQRT', irrational(['1', '1', '2']), 'sqrt(2)'],
        ['1 2 SQRT ADD', irrational(['1', '1', '1'], ['1', '1', '2']), '1/1+sqrt(2)'],
        ['2 SQRT 3 SQRT SUB', irrational(['1', '1', '2'], ['-1', '1', '3']), 'sqrt(2)-sqrt(3)'],
        ['2 SQRT 2 DIV', irrational(['1', '2', '2']), '1/2*sqrt(2)'],
        ['0 2 SQRT SUB', irrational(['-1', '1', '2']), '-sqrt(2)']
    ])('%s', (_source, node, expected) => {
        expect(render(node)).toBe(expected);
    });

    test('inside a Vector it is still one element', () => {
        expect(render(vec(irrational(['1', '1', '2']), num(1)))).toBe('[ sqrt(2) 1/1 ]');
    });
});

// A NIL's reason is its observable content (LANG.VALUES.NIL). The canonical
// text stays the engine's `NIL`; the Stack keeps the reason in the tooltip
// the pointer finds on it, and the label it shows there is this one.
describe('a NIL in the Stack carries its reason', () => {
    const nil = (reason?: string): Node =>
        ({ type: 'nil', value: null, semantics: reason ? { absence: { reason } } : {} }) as Node;

    test('-1 SQRT', () => {
        expect(render(nil('domainMiss'))).toBe('NIL');
        expect(describeNilNode(nil('domainMiss'))).toBe('NIL · domainMiss');
    });

    test('a NIL the program wrote names that reason too', () => {
        expect(describeNilNode(nil('literal'))).toBe('NIL · literal');
    });

    test('a NIL the host sent no reason for is a bare NIL', () => {
        expect(describeNilNode(nil())).toBe('NIL');
    });
});

describe('a point over zero in the Stack says where it stands', () => {
    const point = (numerator: string, denominator = '0'): Node =>
        ({ type: 'number', value: { numerator, denominator }, semantics: {} }) as Node;

    test('100 0 DIV is 1/0, the unsigned point at infinity', () => {
        expect(render(point('1'))).toBe('1/0');
        expect(describePointNode(point('1'))).toBe(
            '1/0 · the point at infinity: unsigned, with no order'
        );
    });

    test('0 0 DIV is 0/0, the point with no order', () => {
        expect(describePointNode(point('0'))).toBe(
            '0/0 · the point with no order: it absorbs every operation'
        );
    });

    test('an ordinary fraction has nothing to add', () => {
        expect(describePointNode(point('1', '2'))).toBeNull();
        expect(describePointNode(num(3))).toBeNull();
    });

    test('an irrational is never a point, whatever its approximation carries', () => {
        const root = { type: 'number', value: { numerator: '1', denominator: '0' },
            semantics: { exactTerms: [{ numerator: '1', denominator: '1', radicand: '2' }] } } as Node;
        expect(describePointNode(root)).toBeNull();
    });

    test('a NIL is not a point', () => {
        expect(describePointNode({ type: 'nil', value: null, semantics: {} } as Node)).toBeNull();
    });
});

// ── merged from stack-render-budget.test.ts ──

// The Stack area's drawing bound. A legal program can put a half-million
// element Vector on the stack; drawing it unbounded would freeze the browser
// tab for tens of seconds, so the render is capped and the undrawn tail is
// counted.


describe('planCollectionRender', () => {
    test('a small collection is drawn whole and elides nothing', () => {
        const budget = createRenderBudget();
        expect(planCollectionRender(3, budget)).toEqual({ shown: 3, elided: 0 });
        expect(budget.remaining).toBe(MAX_RENDERED_ELEMENTS_PER_STACK - 3);
    });

    test('an empty collection costs nothing', () => {
        const budget = createRenderBudget();
        expect(planCollectionRender(0, budget)).toEqual({ shown: 0, elided: 0 });
        expect(budget.remaining).toBe(MAX_RENDERED_ELEMENTS_PER_STACK);
    });

    test('a collection past the per-collection cap is cut there and the rest counted', () => {
        const budget = createRenderBudget();
        const plan = planCollectionRender(500_000, budget);
        expect(plan.shown).toBe(MAX_RENDERED_ELEMENTS_PER_COLLECTION);
        expect(plan.elided).toBe(500_000 - MAX_RENDERED_ELEMENTS_PER_COLLECTION);
    });

    test('many collections cannot together exceed the per-render budget', () => {
        const budget = createRenderBudget();
        let drawn = 0;
        for (let i = 0; i < 10_000; i++) {
            drawn += planCollectionRender(MAX_RENDERED_ELEMENTS_PER_COLLECTION, budget).shown;
        }
        expect(drawn).toBe(MAX_RENDERED_ELEMENTS_PER_STACK);
        expect(budget.remaining).toBe(0);
    });

    test('an exhausted budget draws nothing and elides everything', () => {
        const budget = createRenderBudget(0);
        expect(planCollectionRender(7, budget)).toEqual({ shown: 0, elided: 7 });
    });

    test('shown plus elided always accounts for the whole collection', () => {
        for (const length of [1, 99, 100, 101, 2_500, 1_000_000]) {
            const budget = createRenderBudget();
            const plan = planCollectionRender(length, budget);
            expect(plan.shown + plan.elided).toBe(length);
        }
    });

    test('a nonsense length is treated as empty rather than drawn', () => {
        const budget = createRenderBudget();
        expect(planCollectionRender(Number.NaN, budget)).toEqual({ shown: 0, elided: 0 });
        expect(planCollectionRender(-5, budget)).toEqual({ shown: 0, elided: 0 });
        expect(budget.remaining).toBe(MAX_RENDERED_ELEMENTS_PER_STACK);
    });
});

describe('formatElision', () => {
    test('states the exact undrawn count', () => {
        expect(formatElision(499_900)).toBe('… 499900 more');
    });

    test('is not Ajisai-shaped, so it cannot be misread as part of the value', () => {
        expect(formatElision(3)).not.toMatch(/^[[\]A-Z0-9'/-]+$/);
    });
});

// ── merged from value-latex.test.ts ──

// Math-view LaTeX derivation: the alternate KaTeX stack rendering must be
// generated from the structured protocol form and refuse (return null) any
// value without a faithful flat math reading, so the canonical text
// rendering remains the fallback.



describe('fractionToLatex', () => {
    test('integer collapses the denominator', () => {
        expect(fractionToLatex(frac(3))).toBe('3');
    });

    test('proper fraction renders as \\frac', () => {
        expect(fractionToLatex(frac(3, 4))).toBe('\\frac{3}{4}');
    });

    test('negative sign stays outside the bar', () => {
        expect(fractionToLatex(frac(-3, 4))).toBe('-\\frac{3}{4}');
    });

    test('nine-digit components stay exact', () => {
        expect(fractionToLatex(frac('999999999', '7'))).toBe('\\frac{999999999}{7}');
    });
});

describe('fractionToLatex: huge components switch to scientific notation', () => {
    test('huge integer rounds to a six-digit mantissa with \\approx', () => {
        expect(fractionToLatex(frac('12345678901'))).toBe('\\approx 1.23457 \\times 10^{10}');
    });

    test('exact power of ten needs no mantissa and no \\approx', () => {
        expect(fractionToLatex(frac('1' + '0'.repeat(40)))).toBe('10^{40}');
    });

    test('exact short mantissa keeps no \\approx', () => {
        expect(fractionToLatex(frac('5' + '0'.repeat(12)))).toBe('5 \\times 10^{12}');
    });

    test('huge ratio collapses to one scientific number', () => {
        const digits = '9'.repeat(40);
        expect(fractionToLatex(frac(digits, '7'))).toBe('\\approx 1.42857 \\times 10^{39}');
    });

    test('rounding can carry into the exponent', () => {
        expect(fractionToLatex(frac('999999999999'))).toBe('\\approx 10^{12}');
    });

    test('negative huge value keeps its sign', () => {
        expect(fractionToLatex(frac('-12345678901'))).toBe('\\approx -1.23457 \\times 10^{10}');
    });

    test('tiny ratio gets a negative exponent', () => {
        expect(fractionToLatex(frac('1', '1' + '0'.repeat(12)))).toBe('10^{-12}');
    });

    test('human-scale value with huge components renders as a decimal', () => {
        expect(fractionToLatex(frac('1414213562', '1000000000'))).toBe('\\approx 1.41421');
    });

    test('mid-scale value places the decimal point, not a power of ten', () => {
        expect(fractionToLatex(frac('31415926535', '100000000'))).toBe('\\approx 314.159');
    });

    test('near-zero value uses leading zeros down to 10^-4', () => {
        expect(fractionToLatex(frac('1234567891', '10000000000000'))).toBe('\\approx 0.000123457');
    });

    test('exact human-scale value carries no \\approx', () => {
        expect(fractionToLatex(frac('1500000000', '1000000000'))).toBe('1.5');
    });
});

describe('valueToLatex: scalars', () => {
    test('number renders as fraction', () => {
        expect(valueToLatex(num(1, 2))).toBe('\\frac{1}{2}');
    });

    test('approximate sqrt(2) renders as a decimal with a single \\approx', () => {
        const item = { ...num(1414213562, 1000000000), semantics: { approximate: true } } as Value;
        expect(valueToLatex(item)).toBe('\\approx 1.41421');
    });

    test('malformed numerator is refused (no TeX injection)', () => {
        const item: Value = { type: 'number', value: { numerator: '\\dangerous', denominator: '1' } };
        expect(valueToLatex(item)).toBeNull();
    });

    test('non-math types are refused', () => {
        expect(valueToLatex({ type: 'string', value: 'hello' })).toBeNull();
        expect(valueToLatex({ type: 'nil', value: null })).toBeNull();
        expect(valueToLatex({ type: 'boolean', value: true })).toBeNull();
    });
});

describe('valueToLatex: vectors', () => {
    test('numeric vector renders as a one-row matrix', () => {
        expect(valueToLatex(vec(num(1), num(2), num(3)))).toBe(
            '\\begin{bmatrix} 1 & 2 & 3 \\end{bmatrix}'
        );
    });

    test('rectangular nested vector renders as a rank-2 matrix', () => {
        expect(valueToLatex(vec(vec(num(1), num(2)), vec(num(3), num(4))))).toBe(
            '\\begin{bmatrix} 1 & 2 \\\\ 3 & 4 \\end{bmatrix}'
        );
    });

    test('ragged nested vector is refused', () => {
        expect(valueToLatex(vec(vec(num(1), num(2)), vec(num(3))))).toBeNull();
    });

    test('mixed-type vector is refused', () => {
        expect(valueToLatex(vec(num(1), { type: 'string', value: 'x' }))).toBeNull();
    });

    test('empty vector is refused (bracket text is the surface)', () => {
        expect(valueToLatex(vec())).toBeNull();
    });

    test('oversized vector falls back to text', () => {
        const elements = Array.from({ length: 65 }, (_, i) => num(i));
        expect(valueToLatex(vec(...elements))).toBeNull();
    });
});

// Adversarial robustness: the math view must never throw. A number value whose
// denominator is zero is malformed / NIL occupancy (it never arises from a
// canonical number, but can reach the renderer via restored or injected
// state), and `scientificLatex` must not divide by zero on a >=10-digit zero
// denominator, which would throw a RangeError out of the live Stack render.
describe('valueToLatex zero-denominator robustness', () => {
    for (const denom of ['0', '-0', '00', '0000000000', '-0000000000']) {
        for (const numer of ['1', '1234567890', '12345678901', '99999999999999999999']) {
            test(`returns null (text fallback) for ${numer}/${denom}`, () => {
                expect(valueToLatex(num(numer, denom))).toBeNull();
            });
        }
    }

    test('fractionToLatex does not throw on a huge zero denominator', () => {
        expect(() => fractionToLatex(frac('12345678901', '0000000000'))).not.toThrow();
        expect(() => fractionToLatex(frac('12345678901', '0'))).not.toThrow();
    });
});

// An algebraic irrational carries the multiquadratic normal form it is stored
// in. That form *is* the value, so the math view draws it rather than the best
// rational approximation the same node also carries: `\sqrt{3}` says the whole
// number where `\approx \frac{708158977}{408855776}` only gestures at it.
describe('valueToLatex: a cut Vector', () => {
    test('a Vector the engine cut to its leading elements has no matrix', () => {
        const cut = { ...vec(num(1), num(2)), truncated: { length: 1000, holdsNil: false, holdsRecord: false, digest: '0' } };
        expect(valueToLatex(cut)).toBeNull();
        expect(valueToLatex(vec(cut))).toBeNull();
    });
});

describe('valueToLatex exact normal form', () => {
    function irrational(
        approximation: Value,
        ...terms: Array<[string, string, string]>
    ): Value {
        return {
            ...approximation,
            semantics: {
                approximate: true,
                exactTerms: terms.map(([numerator, denominator, radicand]) => ({
                    numerator,
                    denominator,
                    radicand,
                })),
            },
        } as Value;
    }

    test('a bare square root drops the unit coefficient', () => {
        expect(valueToLatex(irrational(num(708158977, 408855776), ['1', '1', '3'])))
            .toBe('\\sqrt{3}');
    });

    test('an integer coefficient is written in front of the root', () => {
        expect(valueToLatex(irrational(num(2828427124, 1000000000), ['2', '1', '2'])))
            .toBe('2\\sqrt{2}');
    });

    test('a rational term and a scaled root sum', () => {
        expect(
            valueToLatex(
                irrational(num(1245355339, 1000000000), ['1', '2', '1'], ['1', '3', '5'])
            )
        ).toBe('\\frac{1}{2} + \\frac{1}{3}\\sqrt{5}');
    });

    test('a negative term joins with a minus, not a plus', () => {
        expect(
            valueToLatex(irrational(num(-732050807, 1000000000), ['1', '1', '1'], ['-1', '1', '3']))
        ).toBe('1 - \\sqrt{3}');
    });

    test('without a normal form the approximation is still marked', () => {
        const approximated = { ...num(1414213562, 1000000000), semantics: { approximate: true } } as Value;
        expect(valueToLatex(approximated)).toBe('\\approx 1.41421');
    });
});

// The budget used to be charged only by the elements inside a collection, so a
// deep stack — `1 100000 RANGE EXEC`, an ordinary program — drew every value
// one node each and locked the tab the budget is there to protect. Drawn
// against a counting stand-in for `document`.
describe('renderStack', () => {
    class FakeElement {
        className = '';
        textContent = '';
        title = '';
        dataset: Record<string, string> = {};
        children: Array<FakeElement | string> = [];
        parentElement: FakeElement | null = null;
        classes: string[] = [];
        classList = {
            add: (name: string) => { this.classes.push(name); },
            remove() { /* not observed */ },
            toggle() { /* not observed */ }
        };
        appendChild(child: FakeElement) { this.children.push(child); return child; }
        append(...children: Array<FakeElement | string>) { this.children.push(...children); }
        replaceChildren() { this.children = []; }
        querySelector() { return null; }
    }
    let created = 0;
    const previousDocument = (globalThis as any).document;
    beforeAll(() => {
        (globalThis as any).document = { createElement: () => { created += 1; return new FakeElement(); } };
    });
    afterAll(() => { (globalThis as any).document = previousDocument; });

    const draw = (stack: Value[]) => {
        const stackDisplay = new FakeElement();
        stackDisplay.parentElement = new FakeElement();
        const display = createDisplay({ outputDisplay: new FakeElement(), stackDisplay } as any);
        created = 0;
        display.renderStack(stack);
        return (stackDisplay.children[0] as FakeElement).children as FakeElement[];
    };
    const textOf = (element: FakeElement | string): string =>
        typeof element === 'string' ? element : element.textContent + element.children.map(textOf).join('');

    test('a deep stack of scalars draws its top within the budget and counts the rest below', () => {
        const items = draw(Array.from({ length: 100_000 }, (_, i) => num(i + 1)));
        expect(created).toBeLessThan(3 * MAX_RENDERED_ELEMENTS_PER_STACK);
        expect(textOf(items[0]!)).toBe(`${formatElision(100_000 - MAX_RENDERED_ELEMENTS_PER_STACK)} below`);
        expect(textOf(items.at(-1)!)).toBe('100000/1');
    });

    test('a deep stack of short vectors stays bounded too', () => {
        draw(Array.from({ length: 20_000 }, () => vec(num(1), num(2))));
        expect(created).toBeLessThan(5 * MAX_RENDERED_ELEMENTS_PER_STACK);
    });

    test('a stack within the budget is drawn whole, with no marker', () => {
        const items = draw([num(1), num(2)]);
        expect(items.map(textOf)).toEqual(['1/1', '2/1']);
    });

    // The engine hands over a long Vector cut to the elements drawn, with its
    // real length in `truncated` (rust/src/agent/stack_view.rs).
    const truncated = (length: number, overrides: Partial<NonNullable<Value['truncated']>> = {}): Value => ({
        ...vec(...Array.from({ length: MAX_RENDERED_ELEMENTS_PER_COLLECTION }, (_, i) => num(i))),
        truncated: { length, holdsNil: false, holdsRecord: false, digest: '0000000000000000', ...overrides }
    });

    test('a cut Vector counts the elements left out from its real length', () => {
        const [item] = draw([truncated(1_000_000)]);
        const text = textOf(item!);
        expect(text.startsWith('[ 0/1 1/1 ')).toBe(true);
        expect(text.endsWith(`99/1 ${formatElision(1_000_000 - MAX_RENDERED_ELEMENTS_PER_COLLECTION)} ]`)).toBe(true);
    });

    test('a cut Vector whose left-out part holds a Record is drawn as a COLLECT phrase', () => {
        const [item] = draw([truncated(500, { holdsRecord: true })]);
        expect(textOf(item!).endsWith(' 500 COLLECT')).toBe(true);
    });

    // `100 0 DIV` leaves `1/0`, a number drawn as the pair it is; the node
    // carries the affordance and the tooltip that say where the point stands,
    // at the top level and inside a Vector alike. An ordinary number carries
    // neither.
    test('a point over zero is drawn as a number with a tooltip, wherever it sits', () => {
        const items = draw([num(1, 0), vec(num(2), num(0, 0)), num(3)]);
        expect(items.map(textOf)).toEqual(['1/0', '[ 2/1 0/0 ]', '3/1']);
        // Each stack item wraps its value node.
        const nodes = items.map(item => item.children[0] as FakeElement);
        expect(nodes[0]!.classes).toContain('stack-node-point');
        expect(nodes[0]!.classes).not.toContain('stack-node-nil');
        expect(nodes[0]!.title).toBe('1/0 · the point at infinity: unsigned, with no order');
        const lanes = nodes[1]!.children.filter((c): c is FakeElement => typeof c !== 'string');
        const nullity = lanes.find(lane => lane.textContent === '0/0')!;
        expect(nullity.classes).toContain('stack-node-point');
        expect(nullity.title).toBe('0/0 · the point with no order: it absorbs every operation');
        expect(nodes[2]!.classes).not.toContain('stack-node-point');
        expect(nodes[2]!.title).toBe('');
    });
});
