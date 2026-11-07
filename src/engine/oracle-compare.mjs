// Comparing Arare's answer with Ajisai's (ADR-0007, AR-LLR-037): the one
// definition both the oracle harness (scripts/lib/oracle.mjs, AR-VER-209 and
// AR-VER-217) and the Playground's comparison panel
// (src/gui/oracle-panel.ts) use, so the Playground shows exactly the
// comparison CI enforces.
//
// Plain JavaScript, typed by oracle-compare.d.mts, because Node runs the
// harness without compiling TypeScript.

/**
 * What Ajisai runs ahead of a program: Arare's Words that Ajisai lacks, as
 * their exact counterparts. QUANTIZE, RESCALE and NARROW keep the exact
 * value, NEG is `-1 MUL`, SUM is a FOLD of ADD.
 */
export const PRELUDE = [
  "[ 'ARARE_E' BIND 'ARARE_W' BIND ] 'QUANTIZE' DEF",
  "[ 'ARARE_S' BIND ] 'RESCALE' DEF",
  "[ 'ARARE_W' BIND ] 'NARROW' DEF",
  "[ -1 MUL ] 'NEG' DEF",
  "[ 0 [ ADD ] FOLD ] 'SUM' DEF",
].join('\n');

/** A rational from a string `p/q` or `p`, as BigInts. */
export function rat(text) {
  const [p, q = '1'] = String(text).split('/');
  return { p: BigInt(p), q: BigInt(q) };
}

/** A protocol node in a shape both languages share; Arare's carries its budget. */
export function norm(node) {
  const budget = node.budget === undefined ? undefined : node.budget === null ? null : rat(node.budget);
  switch (node.type) {
    case 'number': return { kind: 'number', p: BigInt(node.value.numerator), q: BigInt(node.value.denominator), budget };
    case 'boolean': return { kind: 'boolean', value: node.value, budget };
    case 'nil': return { kind: 'nil', budget };
    case 'vector': return { kind: 'vector', items: node.value.map(norm), budget };
    default: return { kind: node.type, budget };
  }
}

export function show(v) {
  switch (v.kind) {
    case 'number': return `${v.p}/${v.q}`;
    case 'boolean': return v.value ? 'TRUE' : 'FALSE';
    case 'nil': return 'NIL';
    case 'vector': return `[ ${v.items.map(show).join(' ')} ]`;
    default: return v.kind;
  }
}

export const showRat = (r) => `${r.p}/${r.q}`;

const abs = (x) => (x < 0n ? -x : x);
const gcd = (a, b) => (b === 0n ? abs(a) : gcd(b, a % b));

/** Whether `a ≤ b` for two rationals with positive denominators. */
export const ratLe = (a, b) => a.p * b.q <= b.p * a.q;

/** Whether Arare's value `a` is within `tol` of Ajisai's `j`. */
export function within(a, j, tol) {
  if (a.kind !== j.kind) return false;
  switch (a.kind) {
    case 'number':
      if (a.q === 0n || j.q === 0n) return a.q === j.q && (a.p === 0n) === (j.p === 0n);
      // |a.p/a.q − j.p/j.q| ≤ tol.p/tol.q, all denominators positive.
      return abs(a.p * j.q - j.p * a.q) * tol.q <= tol.p * a.q * j.q;
    case 'boolean': return a.value === j.value;
    case 'nil': return true;
    case 'vector': return a.items.length === j.items.length && a.items.every((x, i) => within(x, j.items[i], tol));
    default: return false;
  }
}

/**
 * The largest distance between Arare's value `a` and Ajisai's `j`, lane by
 * lane, in lowest terms; `null` when the two are not numbers of the same
 * shape, or a wheel point stands in either (its distance is not a number).
 */
export function difference(a, j) {
  if (a.kind !== j.kind) return null;
  if (a.kind === 'number') {
    if (a.q === 0n || j.q === 0n) return null;
    const p = abs(a.p * j.q - j.p * a.q);
    const q = a.q * j.q;
    const g = gcd(p, q) || 1n;
    return { p: p / g, q: q / g };
  }
  if (a.kind === 'vector') {
    if (a.items.length !== j.items.length) return null;
    let most = { p: 0n, q: 1n };
    for (const [i, x] of a.items.entries()) {
      if (x.kind === 'nil' && j.items[i].kind === 'nil') continue;
      const d = difference(x, j.items[i]);
      if (d === null) return null;
      if (!ratLe(d, most)) most = d;
    }
    return most;
  }
  return null;
}

/**
 * One value of the stack compared: Arare's and Ajisai's as text, the budget,
 * the distance between them, and the verdict — `within` when Arare's value is
 * within its budget of Ajisai's, `outside` when it is not, `noBudget` when the
 * checker inferred none, `missing` when one stack has no value here.
 */
export function compareStacks(arare, ajisai) {
  const rows = [];
  for (let i = 0; i < Math.max(arare.length, ajisai.length); i++) {
    const a = arare[i] === undefined ? null : norm(arare[i]);
    const j = ajisai[i] === undefined ? null : norm(ajisai[i]);
    const row = {
      arare: a === null ? null : show(a),
      ajisai: j === null ? null : show(j),
      budget: a?.budget ? showRat(a.budget) : null,
      difference: null,
      verdict: 'missing',
    };
    if (a !== null && j !== null) {
      const d = difference(a, j);
      row.difference = d === null ? null : showRat(d);
      if (!a.budget) row.verdict = 'noBudget';
      else row.verdict = within(a, j, a.budget) ? 'within' : 'outside';
    }
    rows.push(row);
  }
  return rows;
}
