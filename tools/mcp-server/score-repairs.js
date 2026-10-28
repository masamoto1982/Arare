#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { atPointer, connectInMemory, readEval } from "./eval-common.js";
import {
  LANGUAGES,
  callsOf,
  indexTraces,
  mayAssertPerfect,
  traceKey,
  traceProvenance,
  validateCorpus,
} from "./evaluation-contract.js";

function matches(result, expected) {
  return result?.isError !== true && result?.structuredContent &&
    Object.entries(expected).every(([pointer, value]) =>
      JSON.stringify(atPointer(result.structuredContent, pointer)) === JSON.stringify(value));
}

/**
 * Replay every call the attempt made, and answer with the first result that
 * satisfies what the case expects.
 *
 * A turn is one attempt and may hold several calls — the reading
 * `score-traces.js` was corrected to in the first baseline round, arriving here
 * for the same reason and, this time, on a model that had got *better*. After
 * the entry-surface round it began statically checking before running; `check`
 * on `1 ADD` reports nothing, because a stack underflow is a runtime fact, and
 * grading only that call recorded the failing `compute` right behind it as
 * never having happened. Two cases went 1.0 -> 0.0 on a strategy change that
 * was an improvement.
 *
 * `some` rather than `first` is the same rule both ends: the attempt observed
 * the diagnosis if any of its calls did, and repaired if any of its calls
 * produced the expected result.
 *
 * The round before, this replayed only `compute` and scored every other tool as
 * nothing having happened — the same defect one level down, since `1 2 AD`
 * through `check` returns the identical `unknownWord` error naming
 * the identical Word. Both corrections point the same way: grade what the model
 * did, through the tool it chose, across the whole turn. That is also what lets
 * the two rates separate rather than being identical by construction — a repair
 * answered with a contract lookup still fails the second expectation, because
 * it never produced the value.
 */
async function callAttempt(client, attempt, expected) {
  const calls = callsOf(attempt);
  let fallback = null;
  for (const call of calls) {
    let outcome = null;
    try {
      outcome = await client.callTool({ name: call.name, arguments: call.arguments ?? {} });
    } catch {
      outcome = null;
    }
    if (matches(outcome, expected)) return outcome;
    fallback ??= outcome;
  }
  return fallback;
}

const tracePath = process.argv[2];
if (!tracePath) {
  console.error("usage: node score-repairs.js <repair-trace-file.json> [--require-perfect]");
  process.exit(2);
}
const corpus = readEval("./eval/repair-cases.json");
const traceDoc = JSON.parse(readFileSync(tracePath, "utf8"));
const traces = indexTraces(traceDoc, validateCorpus(corpus, { repair: true }), { repair: true });
const provenance = traceProvenance(traceDoc);
const { client, close } = await connectInMemory("ajisai-repair-eval");

const tallies = new Map(
  LANGUAGES.map((language) => [language, { asked: 0, missing: 0, observed: 0, repaired: 0 }]),
);
try {
  for (const testCase of corpus.cases) {
    for (const language of LANGUAGES) {
      const tally = tallies.get(language);
      tally.asked += 1;
      const label = `${testCase.id} [${language}]`;
      const trace = traces.get(traceKey(testCase.id, language));
      if (!trace) {
        tally.missing += 1;
        console.log(`MISS ${label}`);
        continue;
      }
      const first = await callAttempt(client, trace.firstAttempt, testCase.firstAttempt.expect);
      const diagnosisOk = matches(first, testCase.firstAttempt.expect);
      if (diagnosisOk) tally.observed += 1;
      // A lucky standalone answer is not diagnostic recovery.
      const repaired = diagnosisOk
        ? await callAttempt(client, trace.repairedAttempt, testCase.repairedAttempt.expect)
        : null;
      const repairOk = diagnosisOk && matches(repaired, testCase.repairedAttempt.expect);
      if (repairOk) tally.repaired += 1;
      console.log(`${repairOk ? "PASS" : "FAIL"} ${label} diagnosis=${diagnosisOk}`);
    }
  }
} finally {
  await close();
}

const combined = { asked: 0, missing: 0, observed: 0, repaired: 0 };
for (const tally of tallies.values()) {
  for (const key of Object.keys(combined)) combined[key] += tally[key];
}
const byLanguage = Object.fromEntries(
  [...tallies].map(([language, tally]) => [language, {
    asked: tally.asked,
    missingTraces: tally.missing,
    diagnosisObservedRate: tally.observed / tally.asked,
    diagnosisDrivenRepairRate: tally.repaired / tally.asked,
  }]),
);
const metrics = {
  schemaVersion: 1,
  // Travels with the numbers: a repair rate copied out of a log still says
  // whether it describes a model or the scorer that grades one.
  provenance,
  measures: provenance.source === "referenceFixture"
    ? "scorer conformance only — not model performance"
    : `model performance for ${provenance.modelId}`,
  cases: corpus.cases.length,
  languages: LANGUAGES,
  asked: combined.asked,
  missingTraces: combined.missing,
  diagnosisObservedRate: combined.observed / combined.asked,
  diagnosisDrivenRepairRate: combined.repaired / combined.asked,
  byLanguage,
  languageGap: {
    diagnosisDrivenRepairRate:
      byLanguage.en.diagnosisDrivenRepairRate - byLanguage.ja.diagnosisDrivenRepairRate,
  },
};
console.log(JSON.stringify(metrics, null, 2));
const total = combined.asked;
const missing = combined.missing;
const diagnosesObserved = combined.observed;
const repairsSucceeded = combined.repaired;
if (process.argv.includes("--require-perfect")) {
  // See score-traces.js: perfection is asserted of the harness, never of a model.
  if (!mayAssertPerfect(traceDoc)) {
    console.error(
      "--require-perfect asserts scorer conformance and is only valid for a referenceFixture trace; " +
        "a model trace is reported, never asserted.",
    );
    process.exit(2);
  }
  if (missing !== 0 || diagnosesObserved !== total || repairsSucceeded !== total) process.exit(1);
}
