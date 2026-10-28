#!/usr/bin/env node
// Gate: a Word's cost says when it grows with the size of an algebraic value.
//
// A cost class (`cost.<axis>.class` in spec/words.json) says how a Word's
// charge grows with its *input*, counted in elements. It does not see the
// size of one element, and for an irrational that size is the whole story:
// `MUL` is `const`, yet multiplying two 16-term values charges 262,144 units
// of numericWork where two rationals charge 1, and a product of fourteen
// `1 + √p` factors is refused at 512 terms after 2.1 million units. An agent
// budgeting a program from the registry alone had no way to see that.
//
// `cost.<axis>.valueDependent: "algebraicTerms"` marks an axis whose charge
// also grows with the term count of an algebraic operand. This gate decides
// the flag by measurement rather than by reading the code: for every Word
// that can be handed a number, it runs the Word on a rational, a 7-term and a
// 16-term algebraic operand, takes the charge the Word alone added on each
// axis, and calls the axis value-dependent exactly when that charge strictly
// grows across the three. The registry must say the same. A Word that cannot
// be handed a number is listed in EXEMPT with the reason, and may not carry
// the flag; a Word in neither list fails, so a new Word is classified when it
// is added.
//
// Usage:
//   node scripts/check-value-dependent-cost.mjs            # check
//   node scripts/check-value-dependent-cost.mjs --suggest  # print the measured flags
import { reporter, resolveAjisaiBin, runAgent, words } from './lib/common.mjs';

const report = reporter('value-dependent-cost', { countSummary: true });
const bin = resolveAjisaiBin('value-dependent-cost');
const FLAG = 'algebraicTerms';
const AXES = { numeric: 'numericWork', collection: 'collectionWork' };

// One rational and two algebraic values of growing term count. Each is bound
// to X before the Word's operands are set up, so the setup is charged in the
// baseline and only the Word itself is measured.
const OPERANDS = [
  '3/7',
  '2 SQRT 3 SQRT ADD 5 SQRT ADD 7 SQRT ADD 2 POW',
  '2 SQRT 3 SQRT ADD 5 SQRT ADD 7 SQRT ADD 11 SQRT ADD 13 SQRT ADD 2 POW',
];

// Word -> the operands it is handed, written over X.
const PROBES = {
  EQ: 'X X', LT: 'X 1', GT: 'X 1', MIN: 'X 1', MAX: 'X 1', SELECT: 'X 1 TRUE',
  RECONCILE: 'X X', ADD: 'X X', SUB: 'X 1', MUL: 'X X', DIV: 'X 3', FLOOR: 'X', ROUND: 'X',
  SQRT: 'X', POW: 'X 2', GCD: 'X 2', RATIO: 'X',
  GET: 'X 1 2 COLLECT 0', LENGTH: 'X 1 2 COLLECT', TAKE: 'X 1 2 COLLECT 1', DROP: 'X 1 2 COLLECT 1',
  CONCAT: 'X 1 COLLECT X 1 COLLECT', REVERSE: 'X 1 2 COLLECT', COLLECT: 'X X 2', RANGE: 'X 3',
  FILL: '[ 3 ] X', SHAPE: 'X 1 2 COLLECT', RESHAPE: 'X X 2 COLLECT [ 2 1 ]', FLATTEN: 'X 1 COLLECT 1 COLLECT',
  DEPTH: 'X 1 2 COLLECT', SORT: 'X 1 2 COLLECT', ORDER: 'X 1 2 COLLECT', UNIQUE: 'X X 2 COLLECT',
  ZIP: 'X 1 COLLECT X 1 COLLECT 2 COLLECT', PUT: 'X 1 2 COLLECT 0 X', BSEARCH: '1 X 2 COLLECT X',
  TALLY: 'X X 2 COLLECT', GROUP: 'X X 2 COLLECT 1 2 2 COLLECT', RECORD: "[ 'a' ] X 1 COLLECT",
  KEYS: "[ 'a' ] X 1 COLLECT RECORD", VALUES: "[ 'a' ] X 1 COLLECT RECORD",
  WITHOUT: "[ 'a' 'b' ] X X 2 COLLECT RECORD 'a'", 'HAS?': "[ 'a' ] X 1 COLLECT RECORD 'a'",
  MERGE: "[ 'a' ] X 1 COLLECT RECORD [ 'b' ] X 1 COLLECT RECORD",
  STR: 'X', FORMAT: 'X 3', 'JSON-ENCODE': 'X', DIGEST: 'X', BIND: "X 'Y'", 'NIL?': 'X', PRINT: 'X',
};

const EXEMPT = {
  'takes no operand': ['TRUE', 'FALSE', 'NIL'],
  'reads truth values, never a number': ['AND', 'NOT'],
  'reads text, never a number': ['CHARS', 'JOIN', 'TRIM', 'UPPER', 'LOWER', 'TOKENIZE', 'SEARCH', 'REPLACE', 'NUM', 'JSON-DECODE', 'FAIL', 'ABSENT'],
  'reads a name or a block; what the block does is charged to the Words it runs': ['CONTRACT', 'DEF', 'DEL', 'MAP', 'FILTER', 'FOLD', 'SCAN', 'EXEC'],
  'reads only the reason of a NIL': ['NIL-REASON'],
};
const exempt = new Map(Object.entries(EXEMPT).flatMap(([reason, names]) => names.map((name) => [name, reason])));

function usage(source) {
  return runAgent(bin, source).resourceUsage;
}

function measure(word, setup) {
  return OPERANDS.map((operand) => {
    const base = `${operand} 'X' BIND ${setup}`;
    const before = usage(base);
    const after = usage(`${base} ${word}`);
    return Object.fromEntries(Object.entries(AXES).map(([axis, key]) => [axis, after[key] - before[key]]));
  });
}

const strictlyGrows = (series) => series[0] < series[1] && series[1] < series[2];
const suggest = process.argv.includes('--suggest');

for (const entry of words().entries) {
  const { name, cost } = entry;
  const flagged = Object.keys(AXES).filter((axis) => cost[axis]?.valueDependent === FLAG);
  for (const axis of Object.keys(cost)) {
    const value = cost[axis].valueDependent;
    if (value !== undefined && value !== FLAG) report.fail(`${name}: cost.${axis}.valueDependent is "${value}", and only "${FLAG}" is defined`);
  }
  if (exempt.has(name)) {
    if (flagged.length > 0) report.fail(`${name} is exempt (${exempt.get(name)}) yet marks ${flagged.join(', ')} value-dependent`);
    continue;
  }
  if (!(name in PROBES)) {
    report.fail(`${name} has no probe and no exemption: add it to PROBES or EXEMPT in scripts/check-value-dependent-cost.mjs`);
    continue;
  }
  const runs = measure(name, PROBES[name]);
  const measured = Object.keys(AXES).filter((axis) => strictlyGrows(runs.map((run) => run[axis])));
  if (suggest) {
    console.log(`${name.padEnd(12)} ${measured.join(',') || '-'}  ${JSON.stringify(runs)}`);
    continue;
  }
  for (const axis of Object.keys(AXES)) {
    const says = flagged.includes(axis);
    const does = measured.includes(axis);
    if (says !== does) {
      const series = runs.map((run) => run[axis]).join(' / ');
      report.fail(
        `${name}: cost.${axis} ${says ? 'says' : 'does not say'} valueDependent "${FLAG}", but on a rational, ` +
          `a 7-term and a 16-term operand it charges ${series} ${AXES[axis]}`,
      );
    }
  }
}

for (const name of [...Object.keys(PROBES), ...exempt.keys()]) {
  if (!words().entries.some((entry) => entry.name === name)) report.fail(`${name} is probed or exempted but is not a Word`);
}

if (!suggest) report.done(`every Word's valueDependent flag matches what it charges on growing algebraic operands.`);
