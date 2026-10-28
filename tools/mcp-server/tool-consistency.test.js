#!/usr/bin/env node
// The three source tools answer one question each about the same program, and
// their answers have to fit together. This runs every source the golden and
// evaluation corpora hold, plus the edge cases that once split them, through
// `compute`, `check` and `infer_contracts`, and holds them to the relations
// they promise:
//
//   * an error's `outcome` and its `error.category` name one category;
//   * malformed source is malformed to all three;
//   * `check` ok means the run does not fail on the program's form or its names;
//   * inference succeeds exactly when the source reads;
//   * every `suggested` line, pasted back above the source, verifies.
//
// These split before: `[ 1 2` was `malformedSource` to two tools and an `ok`
// with no contracts to inference.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createBackend } from "./index.js";

const read = (path) => JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8"));

const EDGE_CASES = [
  "",
  "   ",
  "[ 1 2",
  "1 2 ]",
  "[ 1 [ 2 ]",
  "'unterminated",
  "FOO",
  "1 ADD",
  "1 0 DIV",
  "-1 SQRT",
  "0 0 DIV 1 LT",
  "NIL",
  "[ FOO ] 'W' DEF",
  "[ FOO ] 'W' DEF W",
  "[ 1 ADD ] 'W' DEF 2 W",
  "[ BAR ] 'W' DEF [ 1 ] 'BAR' DEF W",
  "[ 1 2 3 ] [ 2 MUL ] MAP",
  "[ [ 1 ] ADD ] 'INC' DEF [ INC ] 'TWICE' DEF 3 TWICE",
  "'W' DEL",
  "[ 1 ] 'ADD' DEF",
];

const sources = [
  ...new Set([
    ...read("./golden/cases.json").cases.map(({ source }) => source),
    ...read("./eval/cases.json")
      .cases.map(({ arguments: args }) => args?.source)
      .filter((source) => typeof source === "string"),
    ...EDGE_CASES,
  ]),
];

const backend = createBackend();
assert.ok(backend, "no execution backend available for the consistency test");

const failures = [];
const expect = (condition, source, message) => {
  if (!condition) failures.push(`${JSON.stringify(source)}: ${message}`);
};

let compared = 0;
for (const source of sources) {
  let run;
  try {
    run = await backend.compute(source);
  } catch (error) {
    // A host refusal (a ceiling the adapter holds) answers no language
    // question, so there is no outcome to compare.
    if (error?.code) continue;
    throw error;
  }
  const [checked, inferred] = await Promise.all([
    backend.check(source),
    backend.inferContracts(source),
  ]);
  compared += 1;
  const outcome = run.outcome;

  expect(!outcome?.startsWith("error:") || outcome === `error:${run.error?.category}`, source,
    `compute's outcome ${outcome} names another category than its error ${run.error?.category}`);

  const malformed = outcome === "error:malformedSource";
  expect((checked.error?.category === "malformedSource") === malformed, source,
    `check says ${checked.status}/${checked.error?.category}, compute says ${outcome}`);
  expect((inferred.error?.category === "malformedSource") === malformed, source,
    `infer_contracts says ${inferred.status}/${inferred.error?.category}, compute says ${outcome}`);
  expect((inferred.status === "ok") === !malformed, source,
    `infer_contracts is ${inferred.status} for a source compute reads as ${outcome}`);

  if (checked.status === "ok") {
    expect(!["error:malformedSource", "error:unknownWord"].includes(outcome), source,
      `check ok, but the run failed with ${outcome}`);
    for (const contract of inferred.contracts ?? []) {
      const declared = await backend.check(`${contract.suggested}\n${source}`);
      const verdict = declared.contractDecls?.declarations?.find(({ word }) => word === contract.name);
      expect(verdict && verdict.outcome !== "error", source,
        `the suggested \`${contract.suggested}\` does not verify: ${JSON.stringify(declared.contractDecls)}`);
    }
  }
}

assert.deepEqual(failures, [], `the three tools disagree:\n  ${failures.join("\n  ")}`);
assert.ok(compared >= 50, `expected to compare at least 50 sources, compared ${compared}`);
console.log(`tool consistency: ${compared} sources agree across compute, check and infer_contracts (${backend.kind}).`);
process.exit(0);
