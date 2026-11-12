#!/usr/bin/env node
// AR-VER-235: Arare's Arrow reader and writer agree with an independent Arrow
// implementation, the `apache-arrow` npm package (ADR-0014; AR-LLR-039 to
// AR-LLR-043), so the hand-written FlatBuffers code is not checked only
// against itself.
//
//   1. What `arare run --arrow-out` writes, as a file and as a stream,
//      apache-arrow reads: the types, decimal scales, values, nulls and
//      Arare's field metadata are those ADR-0014 states.
//   2. What apache-arrow writes, Arare reads: Int and Boolean columns with
//      nulls, a decimal quantized at --arrow-decimal, in both formats, and a
//      Float64 column refused.
//   3. Arare's file, read and written again by apache-arrow, reads back into
//      Arare as the same values.

import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  Bool, Decimal, Float64, Int32, Table, makeData, makeVector, tableFromIPC, tableToIPC, vectorFromArray,
} from 'apache-arrow';
import { reporter } from './lib/common.mjs';
import { arareBin } from './lib/oracle.mjs';

const report = reporter('arrow-interop');
const dir = mkdtempSync(join(tmpdir(), 'arare-arrow-'));

function arare(args, input = '') {
  const r = spawnSync(arareBin, ['run', ...args, '-'], { input, maxBuffer: 1 << 28 });
  return { status: r.status, stdout: r.stdout, stderr: r.stderr.toString() };
}

function check(cond, message) {
  if (!cond) report.fail(message);
}

/** A decimal lane's unscaled value from its little-endian 32-bit words. */
function unscaled(words, i, bitWidth) {
  const n = bitWidth / 32;
  let x = 0n;
  for (let j = n - 1; j >= 0; j--) x = (x << 32n) | BigInt(words[i * n + j]);
  return x >= 1n << BigInt(bitWidth - 1) ? x - (1n << BigInt(bitWidth)) : x;
}

function lanes(table, name) {
  const v = table.getChild(name);
  const data = v.data[0];
  const out = [];
  for (let i = 0; i < v.length; i++) {
    if (!v.isValid(i)) out.push(null);
    else if (data.type instanceof Decimal) out.push(unscaled(data.values, i + data.offset, data.type.bitWidth));
    else out.push(v.get(i));
  }
  return out;
}

const eq = (a, b) => JSON.stringify(a, (_, x) => (typeof x === 'bigint' ? `${x}n` : x)) === JSON.stringify(b, (_, x) => (typeof x === 'bigint' ? `${x}n` : x));

// 1. Arare writes, apache-arrow reads.
const program = '[ 1 2 3 ] [ TRUE NIL FALSE ] [ 1/0 8 0 QUANTIZE 0.5 8 -1 QUANTIZE NIL ] [ 0/0 8 0 QUANTIZE -3 9223372036854775806 ] [ 1.0 64 -30 QUANTIZE 2.0 64 -30 QUANTIZE 0/0 8 0 QUANTIZE ]';
const file = join(dir, 'out.arrow');
const w = arare(['--arrow-out', file], program);
check(w.status === 0, `arare --arrow-out failed: ${w.stderr}`);
const stream = arare(['--arrow-out', '-'], program);
check(stream.status === 0, `arare --arrow-out - failed: ${stream.stderr}`);
for (const [what, bytes] of [['file', readFileSync(file)], ['stream', stream.stdout]]) {
  const t = tableFromIPC(bytes);
  const f = (i) => t.schema.fields[i];
  check(t.numRows === 3, `${what}: ${t.numRows} rows, not 3`);
  check(f(0).type instanceof Decimal && f(0).type.scale === 0 && f(0).type.bitWidth === 128, `${what}: column 0 is ${f(0).type}`);
  check(eq(lanes(t, '0'), [1n, 2n, 3n]), `${what}: column 0 is ${lanes(t, '0')}`);
  check(f(1).type instanceof Bool, `${what}: column 1 is ${f(1).type}`);
  check(eq(lanes(t, '1'), [true, null, false]), `${what}: column 1 is ${lanes(t, '1')}`);
  // 1/2 at exponent −1 is the code 1, the decimal 5 at scale 1; 1/0 is null.
  check(f(2).type.scale === 1, `${what}: column 2 has scale ${f(2).type.scale}`);
  check(eq(lanes(t, '2'), [null, 5n, null]), `${what}: column 2 is ${lanes(t, '2')}`);
  check(f(2).metadata.get('arare:width') === '8' && f(2).metadata.get('arare:exponent') === '-1', `${what}: column 2's metadata`);
  check(f(2).metadata.get('arare:one-over-zero') === 'AQ==', `${what}: column 2's 1/0 lanes`);
  check(eq(lanes(t, '3'), [null, -3n, 9223372036854775806n]), `${what}: column 3 is ${lanes(t, '3')}`);
  check(f(3).metadata.get('arare:zero-over-zero') === 'AQ==', `${what}: column 3's 0/0 lanes`);
  // Width 64 at exponent −30 needs more than 38 digits: Decimal256. 1 is the
  // code 2^30, the unscaled 2^30 × 5^30 = 10^30 at scale 30.
  check(f(4).type.bitWidth === 256 && f(4).type.scale === 30, `${what}: column 4 is ${f(4).type}`);
  check(eq(lanes(t, '4'), [10n ** 30n, 2n * 10n ** 30n, null]), `${what}: column 4 is ${lanes(t, '4')}`);
}

// 2. apache-arrow writes, Arare reads.
const dec = new Decimal(2, 9, 128);
const decimals = makeData({
  type: dec, length: 3, nullCount: 1,
  nullBitmap: Uint8Array.of(0b101),
  // 1.25, (null), -0.03 as unscaled 125 and -3, four 32-bit words each.
  data: Uint32Array.of(125, 0, 0, 0, 0, 0, 0, 0, 0xfffffffd, 0xffffffff, 0xffffffff, 0xffffffff),
});
const foreign = new Table({
  i: vectorFromArray([7, null, -2147483648], new Int32()),
  b: vectorFromArray([false, true, null], new Bool()),
  d: makeVector(decimals),
});
const expected = [
  '[ 7/1 NIL(imported) 1/0 ]',
  '[ FALSE TRUE NIL(imported) ]',
  '[ 5/4 NIL(imported) -1/32 ]',
];
for (const format of ['file', 'stream']) {
  const path = join(dir, `foreign.${format}`);
  writeFileSync(path, tableToIPC(foreign, format));
  const r = arare(['--arrow-in', path, '--arrow-decimal', '32:-5']);
  check(r.status === 0, `${format}: arare refused apache-arrow's table: ${r.stderr}`);
  check(eq(r.stdout.toString().trimEnd().split('\n'), expected), `${format}: arare read ${r.stdout}`);
  const without = arare(['--arrow-in', path]);
  check(without.status === 1 && without.stderr.includes('field d'), `${format}: a decimal without --arrow-decimal: ${without.stderr}`);
}
const floats = join(dir, 'float.arrow');
writeFileSync(floats, tableToIPC(new Table({ x: vectorFromArray([1.5], new Float64()) }), 'file'));
const f = arare(['--arrow-in', floats]);
check(f.status === 1 && f.stderr.includes('field x'), `a Float64 column: ${f.status} ${f.stderr}`);

// 3. Arare's file through apache-arrow and back.
const again = join(dir, 'again.arrow');
writeFileSync(again, tableToIPC(tableFromIPC(readFileSync(file)), 'file'));
const direct = arare(['--arrow-in', file, '--json']);
const through = arare(['--arrow-in', again, '--json']);
check(direct.status === 0 && through.status === 0, `reading back: ${direct.stderr}${through.stderr}`);
check(direct.stdout.toString() === through.stdout.toString(), `Arare's file rewritten by apache-arrow reads differently:\n${direct.stdout}\n${through.stdout}`);

report.done('Arare and apache-arrow read what each other writes.');
