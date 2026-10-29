#!/usr/bin/env node
// The error budget against the Ajisai oracle on generated programs
// (AR-VER-217, WORK_ORDER.md §7 Phase 3: at least 10,000 programs, no
// violation). Each program runs in Arare (`arare check --lines`) and in the
// pinned Ajisai build (scripts/lib/oracle.mjs); wherever the checker infers a
// budget, Arare's value must lie within it of Ajisai's exact one. This is the
// empirical half of the soundness the Lean theorem
// `Arare.Budget.budget_sound` proves (AR-HLR-027).
//
// The programs are random but fixed: a seeded generator (ORACLE_SEED,
// default 20261029) writes ORACLE_PROGRAMS of them (default 10,600, so that
// at least ORACLE_MIN_RAN, default 10,000, run in Arare: a program Arare
// refuses, such as one whose scale leaves ±31, is not compared). They
// keep out of the three divergences ADR-0007 records — no NIL is written, no
// SUM sees an empty vector, and every divisor is a nonzero literal, so the
// exact meaning never meets a wheel point and a SELECT never branches on
// UNKNOWN in Ajisai — so every difference is the checker's to bound. Inside
// brackets Ajisai reads its operands as data and runs only its own Words, so
// vectors hold integers and blocks use integers and Ajisai's Words; quantized
// values reach vectors through arithmetic at the top level.
//
// The gate fails on any violation, on a program Ajisai refuses that Arare
// ran, and when the checker bounds fewer than MIN_BOUNDED of the values: a
// checker that claimed nothing would otherwise pass.

import { reporter } from './lib/common.mjs';
import { runAjisai, runArare, show, showRat, within } from './lib/oracle.mjs';

const report = reporter('oracle-random');
const SEED = Number(process.env.ORACLE_SEED ?? 20261029);
const COUNT = Number(process.env.ORACLE_PROGRAMS ?? 10600);
const MIN_RAN = Number(process.env.ORACLE_MIN_RAN ?? (process.env.ORACLE_PROGRAMS ? 0 : 10000));
const MIN_BOUNDED = 0.5;

/** mulberry32: a small, fixed pseudo-random sequence. */
function generator(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const random = generator(SEED);
const int = (lo, hi) => lo + Math.floor(random() * (hi - lo + 1));
const pick = (xs) => xs[Math.floor(random() * xs.length)];

/** An exponent that keeps a value of a few units inside the width. */
const exponent = (width) => int({ 8: -4, 16: -10, 32: -20 }[width], 0);

function quantized(nonzero = false) {
  const sign = random() < 0.3 ? '-' : '';
  let text;
  if (random() < 0.6) {
    const digits = int(1, 3);
    const frac = String(int(nonzero ? 1 : 0, 10 ** digits - 1)).padStart(digits, '0');
    text = `${sign}${int(0, 3)}.${frac}`;
  } else {
    text = `${sign}${int(1, 9)}/${int(2, 9)}`;
  }
  const width = pick([8, 16, 32]);
  return `${text} ${width} ${exponent(width)} QUANTIZE`;
}

function literal() {
  const k = random();
  if (k < 0.35) return String(int(-20, 20));
  if (k < 0.42) return String(int(-100000, 100000));
  // Large enough that a few products leave the widest width.
  if (k < 0.47) return String(int(-3e9, 3e9));
  return quantized();
}

/** A literal whose exact value is not zero. */
function divisor() {
  if (random() < 0.5) return String(pick([-1, 1]) * int(1, 20));
  return quantized(true);
}

function condition(depth) {
  const k = random();
  if (depth > 0 && k < 0.15) return `${condition(depth - 1)} ${condition(depth - 1)} AND`;
  if (depth > 0 && k < 0.25) return `${condition(depth - 1)} NOT`;
  return `${scalar(depth)} ${scalar(depth)} ${pick(['LT', 'LT', 'EQ'])}`;
}

function vector(n) {
  const items = Array.from({ length: n }, () => int(-20, 20)).join(' ');
  return `[ ${items} ]`;
}

function block() {
  return pick([`[ ${int(-9, 9)} MUL ]`, `[ ${int(-20, 20)} ADD ]`, `[ ${int(-20, 20)} SUB ]`]);
}

/** A vector of `n` lanes; vectors that meet lane by lane have the same length. */
function vectorValue(depth, n = int(1, 5)) {
  const k = random();
  if (depth === 0 || k < 0.3) return vector(n);
  if (k < 0.55) return `${vectorValue(depth - 1, n)} ${block()} MAP`;
  if (k < 0.8) return `${vectorValue(depth - 1, n)} ${scalar(depth - 1)} ${pick(['ADD', 'SUB', 'MUL'])}`;
  return `${vectorValue(depth - 1, n)} ${vectorValue(depth - 1, n)} ${pick(['ADD', 'MUL'])}`;
}

/** A vector value that is data at the top level, not a quote left unread. */
const vectorData = (depth) => `${vectorValue(depth)} [ 0 ADD ] MAP`;

function scalar(depth) {
  if (depth === 0) return literal();
  const k = random();
  const d = depth - 1;
  if (k < 0.12) return literal();
  if (k < 0.45) return `${scalar(d)} ${scalar(d)} ${pick(['ADD', 'SUB', 'MUL'])}`;
  if (k < 0.55) return `${scalar(d)} ${divisor()} DIV`;
  if (k < 0.6) return `${scalar(d)} NEG`;
  if (k < 0.67) return `${scalar(d)} ${int(1, 4)} RESCALE`;
  if (k < 0.71) return `${scalar(d)} ${pick([16, 32, 64])} NARROW`;
  if (k < 0.77) {
    const width = pick([16, 32]);
    return `${scalar(d)} ${width} ${exponent(width)} QUANTIZE`;
  }
  if (k < 0.87) return `${scalar(d)} ${scalar(d)} ${condition(d)} SELECT`;
  if (k < 0.94) return `${vectorValue(d)} SUM`;
  return `${vectorValue(d)} ${literal()} [ ${pick(['ADD', 'ADD', 'MUL'])} ] FOLD`;
}

function program() {
  const k = random();
  const depth = int(1, 4);
  if (k < 0.7) return scalar(depth);
  if (k < 0.8) return `${scalar(depth)} ${scalar(depth)}`;
  if (k < 0.9) return vectorData(depth);
  return `${vectorValue(depth)} [ ${int(-20, 20)} LT ] FILTER`;
}

const sources = Array.from({ length: COUNT }, program);
let arare;
try {
  arare = runArare(sources);
} catch (error) {
  report.fatal(error.message);
}

let refused = 0;
let values = 0;
let bounded = 0;
let closed = 0;
let violations = 0;
for (const [i, source] of sources.entries()) {
  const a = arare[i];
  if (a.error) { refused++; continue; }
  const j = await runAjisai(source);
  if (j.error) { report.fail(`Ajisai refused a program Arare ran: ${source}: ${j.error}`); continue; }
  if (a.field === 'closed') closed++;
  if (a.stack.length !== j.stack.length) { report.fail(`stacks differ: ${source}`); continue; }
  for (const [k, x] of a.stack.entries()) {
    values++;
    if (x.budget === null) continue;
    bounded++;
    if (!within(x, j.stack[k], x.budget)) {
      violations++;
      report.fail(`budget ${showRat(x.budget)} violated: ${source}\n  Arare ${show(x)}; Ajisai ${show(j.stack[k])}`);
    }
  }
}

const ran = COUNT - refused;
if (ran < COUNT * 0.9) report.fail(`Arare refused ${refused} of ${COUNT} programs; the generator is writing programs Arare cannot run.`);
if (ran < MIN_RAN) report.fail(`only ${ran} programs ran; Phase 3's exit criterion asks for at least ${MIN_RAN}.`);
if (bounded < values * MIN_BOUNDED) report.fail(`the checker bounded only ${bounded} of ${values} values (floor ${MIN_BOUNDED * 100}%).`);
report.done(
  `${ran} program(s) (seed ${SEED}, ${refused} refused by Arare), ${bounded} of ${values} value(s) bounded, ` +
    `${violations} violation(s); ${closed} program(s) field=closed.`,
);
