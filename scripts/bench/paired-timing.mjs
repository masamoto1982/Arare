// Timing for a WebAssembly bench case whose operand must be built first.
//
// `bench_execute` runs one whole program, so the body cannot be timed alone:
// a case is timed as `setup body` and as `setup`, and the body is the
// difference. Subtracting two independent minima — the best `setup body`
// run less the best `setup` run — has no error bar, and clamping the result
// at zero turned noise into rows of `0.00 ms` that moved between runs.
//
// Here the two programs are run back to back, in alternating order, `pairs`
// times, and each pair gives one difference. The reported time is the median
// of the differences with their interquartile range beside it, unclamped: a
// row whose range reaches zero is below the noise floor, and `noisy` says so
// rather than printing a number that looks like a measurement. A body too
// small to separate from its setup should be repeated in its case
// (`repeat`), not read off a clamped difference.

import { performance } from "node:perf_hooks";

async function timeOnce(run, source) {
  const started = performance.now();
  await run(source);
  return performance.now() - started;
}

const quantile = (sorted, q) => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];

/// `{ ms, iqr, noisy }` for `body` after `setup`, from `pairs` paired runs.
export async function pairedBodyMs(run, setup, body, pairs = 15) {
  const full = setup ? `${setup} ${body}` : body;
  await run(full); // warm-up, discarded
  const diffs = [];
  for (let i = 0; i < pairs; i++) {
    const swap = i % 2 === 1;
    const first = await timeOnce(run, swap ? full : setup);
    const second = await timeOnce(run, swap ? setup : full);
    diffs.push(swap ? first - second : second - first);
  }
  diffs.sort((a, b) => a - b);
  const ms = quantile(diffs, 0.5);
  const iqr = quantile(diffs, 0.75) - quantile(diffs, 0.25);
  return { ms, iqr, noisy: quantile(diffs, 0.25) <= 0 };
}

/// One table row: the median, its spread, and a mark when it is noise.
export function formatRow(name, { ms, iqr, noisy }, elements) {
  const ns = (ms * 1e6) / elements;
  const mark = noisy ? "  (below noise floor)" : "";
  return `${name.padEnd(24)} ${ms.toFixed(2).padStart(10)} ms ±${iqr.toFixed(2).padStart(6)} ${ns.toFixed(1).padStart(10)} ns/elem${mark}`;
}
