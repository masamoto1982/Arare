#!/usr/bin/env node
// What the meters charge per millisecond *under WebAssembly* — the engine the
// browser playground actually runs on.
//
// Run with:  node scripts/wasm-profile-calibration.mjs
//
// `rust/examples/work_meter_calibration.rs` and
// `rust/examples/collection_word_calibration.rs` measure the same meters on
// native. This exists because those two are not a proxy for this one, which is
// not obvious and was assumed once: measured back to back on one container,
// the native->WASM rate ratio is 2.1x for `numericWork`, 4.0x for
// `collectionWork` and 0.66x for `executionSteps`. A ratio that swings from
// "twice as fast" to "a third slower" depending on the meter means a single
// native calibration cannot make three ceilings bound equal time on WASM —
// which is the entire job of a derived host profile.
//
// Why the Node bundle rather than the browser one: `src/wasm/generated` is a
// `--target web` build that needs `fetch`, while `tools/mcp-server/wasm/
// generated` is `--target nodejs` and loads synchronously. Both come from the
// same Rust through the same wasm-pack profile in the same rebuild scripts, so
// the compiled module being measured is the module the playground ships. What
// differs is the embedding engine, and Node and Chrome are both V8 — so this
// measures a desktop-class V8, which is what the playground profile is
// derived for. A mobile browser is slower; see the profile's own doc comment.
//
// RESOLVED FINDING, 2026-08-14 — kept for whoever re-measures next.
// `UNIQUE 4k x 4096-digit` used to come back roughly 300-470x below every
// other collection path on this run (248 units/ms against 73,491 for
// `REVERSE 100k`; 780 against 45,025 on native release) — not a property of
// WASM, but `Fraction::hash` on the `Big` representation cloning through
// `to_bigint_pair()` and running a full, unbalanced `BigInt::gcd` once per
// element per scan (612 us at `gcd(4096-digit, 1)`, the common case for any
// wide integer-valued Fraction, against 1.2 us once balanced). Fixed by
// `balanced_bigint_gcd` (`rust/src/types/fraction_arithmetic.rs`): the same case now
// measures 47,162-73,457 units/ms across runs, in range with the other
// collection paths and still the floor by a normal margin, not a hole.
// De-quadraticization (same day) had already fixed this shape of defect for
// the `Small` representation; this closed it for `Big`. If a future
// collectionWork floor comes back looking like a hole again — one path
// wildly below the rest rather than merely lower — read this as the pattern
// to check for, not as evidence WASM itself is the cause.
//
// Method, matching the native harnesses after their 2026-08-14 fix:
//   - operand construction is executed first and subtracted, in both time and
//     charged units, so a rate is charged-work over the time that charged it;
//   - charged units are read back from the engine's own `resourceUsage`, never
//     derived from the pricing formula by hand;
//   - several repetitions, reporting the *median* — a minimum over a noisy
//     shared machine measures scheduling luck, not the path.

import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const bundleDir = resolve(here, '../tools/mcp-server/wasm/generated');
const require = createRequire(`${bundleDir}/`);
const wasm = require('./ajisai_core.js');

const STEP_LIMIT = 4_000_000_000;
const REPS = 3;

// `agent_compute` applies LOCAL_AGENT_RUNTIME_LIMITS (the MCP profile). Limits
// bound when work stops, not how fast it runs, so rates measured under them
// are the engine's rates — but every case below has to *fit*: 10,000,000
// numericWork, 20,000,000 collectionWork, 100,000 materializedElements.
async function once(source) {
    const started = process.hrtime.bigint();
    const json = await wasm.agent_compute(source, STEP_LIMIT);
    const millis = Number(process.hrtime.bigint() - started) / 1e6;
    const report = JSON.parse(json);
    if (report.status !== 'ok') {
        throw new Error(`case must complete to be measurable, got: ${report.message ?? report.status}`);
    }
    return { millis, usage: report.resourceUsage };
}

const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];

/**
 * Rate for `meter` over `ops`, with `setup`'s time and units subtracted.
 *
 * Setup and ops run in one `execute`, and the subtrahend is `setup` alone in
 * one `execute`, because that is the interval a ceiling actually governs: a
 * runaway computation is a loop *inside* one execute. Crossing an execute
 * boundary costs a large fixed amount on a big operand — ~380 ms to first
 * touch a 100,000-element vector carried over from a previous execute, against
 * ~11 ms for the same Word repeated inside one — since the epoch snapshot
 * shares the stack and the first mutation has to unshare it. That constant is
 * paid once per run, not once per iteration, so folding it into a per-unit
 * rate would misprice every budget derived from it.
 *
 * Returns `null` when the ops are too cheap to separate from setup jitter.
 * That is a real answer, not a failure: a path that cannot be timed against
 * its own setup is nowhere near the floor, which is the only thing this
 * harness is looking for.
 */
async function rate(meter, setup, ops) {
    const samples = [];
    let units = 0;
    for (let i = 0; i < REPS; i++) {
        const base = setup ? await once(setup) : { millis: 0, usage: { [meter]: 0 } };
        const full = await once(`${setup}${ops}`);
        units = full.usage[meter] - (base.usage[meter] ?? 0);
        const deltaMs = full.millis - base.millis;
        // Require the ops to be both positive and clear of setup jitter.
        if (deltaMs > 1 && deltaMs > base.millis * 0.02) samples.push(units / deltaMs);
    }
    if (!samples.length) return { units, rate: null };
    return { units, rate: median(samples) };
}

const wide = '9'.repeat(4096);
const PAIRS = [[2, 3], [5, 7], [11, 13], [17, 19], [23, 29], [31, 37], [41, 43], [47, 53]];
const cascade = (f) => PAIRS.slice(0, f).map(([l, r], i) => `${l} SQRT ${r} SQRT ADD${i > 0 ? ' MUL' : ''}`).join(' ');

const primes = (count) => {
    const found = [];
    for (let candidate = 2; found.length < count; candidate++) {
        if (found.every((p) => p * p > candidate || candidate % p !== 0)) found.push(candidate);
    }
    return found;
};
// `√(p₁p₂) + √(p₃p₄) + …`, then `√p₁`, `√p₃`, … added one at a time.
const refining = (pairs) => {
    const ps = primes(2 * pairs);
    let setup = '0';
    let ops = '';
    for (let i = 0; i < ps.length; i += 2) {
        setup += ` ${ps[i]} ${ps[i + 1]} MUL SQRT ADD`;
        ops += ` ${ps[i]} SQRT ADD`;
    }
    return [setup, ops];
};
// `√(C·q₁) + √(C·q₂) + …` with `C` the product of the first `splits` primes,
// then the primes of `C` added one at a time.
const sharedSplit = (splits, terms) => {
    const ps = primes(splits + terms);
    const product = `${ps.slice(0, splits).join(' ')}${' MUL'.repeat(splits - 1)}`;
    const setup = '0' + ps.slice(splits).map((q) => ` ${product} ${q} MUL SQRT ADD`).join('');
    const ops = ps.slice(0, splits).map((p) => ` ${p} SQRT ADD`).join('');
    return [setup, ops];
};

const NUMERIC = [
    // The floor candidate: one charged unit per lane, a machine-word add
    // inside a tensor kernel, with nothing else to amortize it.
    ['dense tensor lanes (100k) x80', '0 99999 RANGE', ' 1 ADD'.repeat(80)],
    // 19, not 20: the twentieth multiplication crosses the MCP profile's
    // `bigintBits` (272,133 against 262,144), which is the very boundary
    // `profile_liveness_tests` pins. Measuring under a real profile means
    // living inside every one of its ceilings, not just the one being timed.
    ['scalar wide MUL x19 (4096-digit)', `[ ${wide} MUL ] 'M' DEF 1`, ' M'.repeat(19)],
    ['scalar wide ADD x200 (4096-digit)', `[ ${wide} ADD ] 'W' DEF 1`, ' W'.repeat(200)],
    ['algebraic cascade(8) x18', `[ ${cascade(8)} ] 'C' DEF`, ' C'.repeat(18)],
    // Sums, priced by ALGEBRAIC_SUM_TERM_UNITS. The same three shapes
    // `work_meter_calibration.rs` measures natively, sized to fit the profile:
    // a fold of square roots, a sum whose every addend splits a composite
    // basis element, and one whose split element every monomial shares.
    ['algebraic sum (fold of SQRT 1..150)', '', '1 150 RANGE SQRT 0 [ ADD ] FOLD'],
    ['algebraic sum, refining (60 splits)', refining(60)[0], refining(60)[1]],
    ['algebraic sum, shared split (16 x 50)', sharedSplit(16, 50)[0], sharedSplit(16, 50)[1]],
];

const COLLECTION = [
    // Repetition counts are capped by the MCP profile's 20,000,000
    // `collectionWork`, which the *setup* also spends: `0 99999 RANGE`
    // charges 1,700,000 before a single measured Word runs.
    ['REVERSE 100k x10', '0 99999 RANGE', ' REVERSE'.repeat(10)],
    ['UNIQUE 100k all-distinct x5', '0 99999 RANGE', ' UNIQUE'.repeat(5)],
    // Built with FILL, not `RANGE { 1 MOD } MAP`: the MAP costs 100,000
    // execution steps, and a setup that large and that variable swamps the
    // few milliseconds nine hash-hitting UNIQUE passes actually take.
    ['UNIQUE 100k d=1 x9', '[ 100000 ] 7 FILL', ' UNIQUE'.repeat(9)],
    ['SORT 100k x5', '0 99999 RANGE', ' SORT'.repeat(5)],
    ['TALLY 100k all-distinct x5', '0 99999 RANGE', ' TALLY'.repeat(5)],
    // Wide elements: an equality probe walks limbs, and this was by far the
    // dearest per-element shape on native. If any collection path is the WASM
    // floor, it is a candidate.
    ['UNIQUE 4k x 4096-digit x2', `1 4000 RANGE [ ${wide} MUL ] MAP`, ' UNIQUE'.repeat(2)],
];

const STEPS = [
    // 10,000 reps, not 200,000: each `" 7 ADD"` is 6 source bytes, and this
    // case's source is written out flat (unlike the trampoline below, which
    // stays short at any rep count because the loop body lives once in a
    // `DEF`) — 200,000 reps is 1,200,001 bytes, over the MCP profile's
    // 65,536-byte `sourceBytes`. 10,000 reps is 60,001 bytes, comfortably
    // under, and still gives `rate()` a wall-clock interval well clear of
    // jitter (rates are ms-normalized, so step *count* only needs to be
    // enough to time cleanly, not any particular round number).
    ['add loop x10k', '', `1${' 7 ADD'.repeat(10_000)}`],
    // Every iteration pays a dictionary lookup and a frame push, not just an
    // arithmetic op, which is what makes it the dearer of the two shapes. It
    // used to be written as a self-calling trampoline; LANG.DICTIONARY.ACYCLIC
    // refuses a self-call, so the same per-iteration cost is paid by a MAP over
    // a materialized Vector instead.
    ['user-word call x200k', "[ [ 7 ] ADD ] 'STEP' DEF", `1 200000 RANGE [ STEP ] MAP`],
];

async function section(title, meter, cases) {
    console.log(`\n── ${title} ${'─'.repeat(Math.max(0, 52 - title.length))}\n`);
    console.log(`${'path'.padEnd(36)}${'units'.padStart(14)}${'units/ms'.padStart(12)}`);
    let floor = Infinity;
    let floorName = '';
    for (const [name, setup, ops] of cases) {
        const r = await rate(meter, setup, ops);
        if (r.rate === null) {
            console.log(`${name.padEnd(36)}${String(r.units).padStart(14)}${'too fast'.padStart(12)}`);
            continue;
        }
        if (r.rate < floor) { floor = r.rate; floorName = name; }
        console.log(`${name.padEnd(36)}${String(r.units).padStart(14)}${r.rate.toFixed(0).padStart(12)}`);
    }
    console.log(`\nfloor: ${floor.toFixed(0)} ${meter === 'executionSteps' ? 'steps' : 'units'}/ms  (${floorName})`);
    return floor;
}

const numeric = await section('numericWork, under WASM', 'numericWork', NUMERIC);
const collection = await section('collectionWork, under WASM', 'collectionWork', COLLECTION);
const steps = await section('executionSteps, under WASM', 'executionSteps', STEPS);

console.log('\n── derived budgets at a 30s host time budget ────────────\n');
for (const [name, floor] of [['numericWork', numeric], ['collectionWork', collection], ['executionSteps', steps]]) {
    console.log(`${name.padEnd(16)} 30000 ms x ${floor.toFixed(0).padStart(8)} = ${Math.round(30_000 * floor).toLocaleString()}`);
}
console.log(
    '\nA budget here is only as good as the floor above it. Before deriving\n' +
    'anything from a floor, check that the winning path is priced correctly —\n' +
    'a floor that is one specific shape wildly below every other path is a\n' +
    'pricing hole, not a bound. See the RESOLVED FINDING at the top of this\n' +
    'file for what that looked like the one time it happened here.'
);
