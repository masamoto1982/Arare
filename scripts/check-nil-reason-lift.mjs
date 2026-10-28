#!/usr/bin/env node
// Reason-preservation gate for the element-wise lift (LANG.COLLECTIONS.LIFT:
// "each lane preserves the exactness, truth, NIL, and ERROR distinctions of
// the scalar law").
//
// The scalar law is passthrough with the reason intact — `-1 SQRT 1 ADD` is still
// `NIL(domainMiss)`. This checks the lifted law says the same thing, lane
// for lane: a program that puts a reasoned NIL into a collection and then runs
// element-wise Words over it must still report that reason, and must still
// report an absence at all.
//
// Both halves are needed, and neither sees the other's failure:
//
//   - The *reason* check catches a lane whose absence survived but whose
//     reason did not. A lane law that rebuilds a collection from numbers
//     only can drop the reason on the way: the lane comes back reasonless —
//     which reads as `nil:literal`, "a NIL the program wrote rather than
//     computed" (spec/outcomes.json), for a NIL the program computed and
//     never wrote.
//   - The *count* check catches a lane that stopped being an absence. A
//     dense tensor holds numbers only (LANG.VALUES.EXACT), so a lift that
//     read a NIL lane as some number — a pair over zero, say — would compute
//     with it and answer an observable `0/0` scalar. There is no reason to
//     compare there because there is no NIL left to carry one, so the reason
//     check passes such a lane in silence.
//
// Every program here is executed; nothing is string-matched. Some programs
// legitimately raise — a chain that pairs the producer's vector with one its
// shape cannot pair with is a `shapeMismatch` (LANG.COLLECTIONS.LIFT) — and
// which ones is worked out from the shapes below, not observed: a program
// that raises where its shapes pair is a failure, and so is one that answers
// where they do not. Skipping every raise instead let a lift that wrongly
// refused a pairing (a one-element Vector over irrational lanes) pass here.
// A program that does not answer at all is always a failure.
//
// Usage:
//   node scripts/check-nil-reason-lift.mjs
//   AJISAI_BIN=/path/to/ajisai ...   # override CLI binary

import { reporter, resolveAjisaiBin, runAgent } from './lib/common.mjs';

const report = reporter('nil-reason-lift');
const fail = report.fail;

// A program that leaves a collection with at least one reasoned NIL lane, the
// reason every one of those lanes carries, and the shape of that collection
// (`null` for a ragged one, which pairs with no vector). Chosen to cover each way a lane
// can become absent and each representation a lane can live in: a dense
// rational vector, a nested one, a ragged one, and a vector of irrational
// exact reals (which takes an entirely separate lift).
//
// The written NIL (`[ 1 NIL 3 ]`) is not a spare case. A vector holding one
// used to be barred from dense storage, because dense storage could record
// only *that* a lane was absent; now that it records why, that vector is
// stored densely like any other and takes the same path as a computed one.
const PRODUCERS = [
  ['[ 1 -2 ] SQRT', 'domainMiss', [2]],
  ['[ 4 9 -1 ] SQRT', 'domainMiss', [3]],
  ['[ 1 ] [ 4 9 -1 ] MUL SQRT', 'domainMiss', [3]],
  ['[ 4 -1 ] SQRT', 'domainMiss', [2]],
  ['[ -1 -4 ] SQRT', 'domainMiss', [2]],
  ["[ '1' 'a' ] [ NUM ] MAP", 'invalidEncoding', [2]],
  ['[ 1 2 3 ] [ -1 MUL SQRT ] MAP', 'domainMiss', [3]],
  ['[ 2 3 ] [ SQRT ] MAP [ 1 -1 ] SQRT MUL', 'domainMiss', [2]],
  ['[ [ 1 -2 ] [ 3 4 ] ] SQRT', 'domainMiss', [2, 2]],
  ['[ -1 [ -2 -3 ] ] SQRT', 'domainMiss', null],
  ['[ 1 NIL 3 ] [ 2 ] MUL', 'literal', [3]],
];

// Applied after a producer. Each is lane-preserving: it maps over the lanes
// without adding or removing an absence, so the result must carry exactly the
// producer's absences, with exactly the producer's reasons. A chain that
// pairs the result with a vector says so in PAIRS_WITH below.
const CHAINS = [
  '',
  '[ 1 1 ] ADD',
  '[ 1 1 ] SUB',
  '[ 1 1 ] MUL',
  '[ 1 1 ] DIV',
  'FLOOR',
  'ROUND',
  '[ 2 ] MUL',
  '[ 2 ] ADD',
  '2 MUL',
  '2 ADD',
  'REVERSE',
  '[ 1 1 ] ADD [ 1 1 ] MUL',
  '[ 1 ADD ] MAP',
  // Structural rebuilds. These add no absence and remove none, but they
  // reassemble the collection from its children — which is where dense
  // storage is chosen. A lane whose reason lives only outside the dense
  // columns is lost exactly here, and nowhere the arithmetic chains above
  // would show it.
  '[ 3 4 ] CONCAT',
  '[ 3 4 ] CONCAT REVERSE',
  '[ 1 1 ] ADD [ 3 4 ] CONCAT',
];

// The one-axis vector each pairing chain combines the producer's result with
// (its first pairing; a chain whose first pairing succeeds pairs the same
// shape again).
const PAIRS_WITH = {
  '[ 1 1 ] ADD': 2,
  '[ 1 1 ] SUB': 2,
  '[ 1 1 ] MUL': 2,
  '[ 1 1 ] DIV': 2,
  '[ 2 ] MUL': 1,
  '[ 2 ] ADD': 1,
  '[ 1 1 ] ADD [ 1 1 ] MUL': 2,
  '[ 1 1 ] ADD [ 3 4 ] CONCAT': 2,
};

// LANG.COLLECTIONS.LIFT: shapes align at the innermost axis, and a one-axis
// vector of length n pairs when n is 1 or the innermost length; a ragged
// vector pairs with no vector at all.
function pairs(shape, length) {
  if (length === undefined) return true;
  if (shape === null) return false;
  return length === 1 || length === shape[shape.length - 1];
}

function collectAbsenceReasons(node, out) {
  if (node === null || typeof node !== 'object') return;
  if (Array.isArray(node)) {
    for (const item of node) collectAbsenceReasons(item, out);
    return;
  }
  if (node.type === 'nil') out.push(node.semantics?.absence?.reason ?? '(no reason)');
  if (node.value !== undefined) collectAbsenceReasons(node.value, out);
}

const ajisaiBin = resolveAjisaiBin('nil-reason-lift');

// Run `source` and return its absence reasons, or a string describing why it
// has none to compare. A raise is one such reason; failing to answer is not.
function absenceReasons(source) {
  const json = runAgent(ajisaiBin, source, {
    exitMessage: (result) =>
      `the engine did not answer (exit ${result.status}) — no value, no NIL and no ERROR is ` +
      `no outcome under LANG.FAILURE.TRICHOTOMY: ${result.stderr.split('\n').slice(0, 3).join(' ')}`,
  });
  if (json.status === 'error') return { raised: true, category: json.error?.category ?? null, reasons: [] };
  const reasons = [];
  collectAbsenceReasons(json.stack ?? [], reasons);
  return { raised: false, reasons };
}

let checked = 0;
let refused = 0;
for (const [producer, expected, shape] of PRODUCERS) {
  let base;
  try {
    base = absenceReasons(producer);
  } catch (e) {
    fail(`producer ${JSON.stringify(producer)}: ${e.message}`);
    continue;
  }
  if (base.raised || base.reasons.length === 0) {
    fail(
      `producer ${JSON.stringify(producer)} is supposed to leave at least one reasoned NIL lane ` +
        `and left none — the case no longer tests what it was written for`,
    );
    continue;
  }

  for (const chain of CHAINS) {
    const source = chain ? `${producer} ${chain}` : producer;
    let observed;
    try {
      observed = absenceReasons(source);
    } catch (e) {
      fail(`${JSON.stringify(source)}: ${e.message}`);
      continue;
    }
    const shouldPair = pairs(shape, PAIRS_WITH[chain]);
    if (observed.raised) {
      if (shouldPair || observed.category !== 'shapeMismatch') {
        fail(
          `${JSON.stringify(source)}: raised ${observed.category ?? '(no category)'}, but ` +
            (shouldPair
              ? `its shapes pair under LANG.COLLECTIONS.LIFT, so the lift must answer`
              : `a pairing LANG.COLLECTIONS.LIFT refuses is a shapeMismatch`),
        );
        continue;
      }
      refused += 1;
      continue;
    }
    if (!shouldPair) {
      fail(
        `${JSON.stringify(source)}: answered, but ${shape === null ? 'a ragged vector' : `shape [${shape}]`} ` +
          `does not pair with a vector of length ${PAIRS_WITH[chain]} (LANG.COLLECTIONS.LIFT) — it must raise shapeMismatch`,
      );
      continue;
    }

    if (observed.reasons.length !== base.reasons.length) {
      fail(
        `${JSON.stringify(source)}: ${JSON.stringify(producer)} leaves ` +
          `${base.reasons.length} absent lane(s) and this leaves ${observed.reasons.length} — ` +
          `an element-wise Word neither creates nor fills a lane, so an absence stopped being one`,
      );
      continue;
    }
    const wrong = observed.reasons.filter((reason) => reason !== expected);
    if (wrong.length > 0) {
      fail(
        `${JSON.stringify(source)}: every absent lane was created with reason ` +
          `${JSON.stringify(expected)}, but the result reports ${JSON.stringify(observed.reasons)} — ` +
          `the lift dropped the reason the scalar law preserves`,
      );
      continue;
    }
    checked += 1;
  }
}

// Every combination is either checked or refused as predicted; a shortfall
// means some were silently passed over.
const combinations = PRODUCERS.length * CHAINS.length;
if (checked + refused !== combinations) {
  fail(`accounted for ${checked + refused} of ${combinations} programs`);
}

report.done(
  `every absent lane kept its reason and stayed absent across ${checked} executed ` +
    `programs, and ${refused} more raised the shapeMismatch their shapes predict ` +
    `(${PRODUCERS.length} producers x ${CHAINS.length} element-wise chains).`,
);
