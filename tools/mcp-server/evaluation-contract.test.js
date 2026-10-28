#!/usr/bin/env node
import assert from "node:assert/strict";
import { LANGUAGES, indexTraces, mayAssertPerfect, validateCorpus } from "./evaluation-contract.js";

const validCase = {
  id: "one",
  prompts: { en: "compute one", ja: "1 を計算して" },
  expectedTool: "compute",
  arguments: { source: "1" },
  expect: { "/status": "ok" },
};
const corpus = { schemaVersion: 1, cases: [validCase] };
// Every trace document now declares what produced it. `fixture` is the shape a
// hand-written conformance trace takes; `modelDoc` is a real capture.
const fixture = (traces) => ({
  schemaVersion: 1,
  provenance: { source: "referenceFixture" },
  traces,
});
const ids = validateCorpus(corpus);
assert.throws(
  () => validateCorpus({ schemaVersion: 1, cases: [validCase, validCase] }),
  /duplicate evaluation case id/,
);
assert.throws(
  () => validateCorpus({ schemaVersion: 1, cases: [{ ...validCase, expect: { status: "ok" } }] }),
  /invalid JSON pointer/,
);
// Every case is asked in both languages, and the two askings must differ. A
// pair whose Japanese side is the English side copied across would score twice
// and report a language comparison it never made.
assert.throws(
  () => validateCorpus({ schemaVersion: 1, cases: [{ ...validCase, prompts: { en: "only" } }] }),
  /must be asked in exactly/,
);
assert.throws(
  () => validateCorpus({
    schemaVersion: 1,
    cases: [{ ...validCase, prompts: { en: "same text", ja: "same text" } }],
  }),
  /repeats the same prompt text/,
);
assert.throws(
  () => indexTraces(fixture([{ caseId: "other", language: "en", selectedTool: null }]), ids),
  /unknown caseId/,
);
// A trace that does not say which asking produced it cannot be filed against
// the right half of a pair.
assert.throws(
  () => indexTraces(fixture([{ caseId: "one", selectedTool: null }]), ids),
  /must record which of/,
);
assert.throws(
  () => indexTraces(fixture([
    { caseId: "one", language: "en", selectedTool: "compute", arguments: {} },
    { caseId: "one", language: "en", selectedTool: "compute", arguments: {} },
  ]), ids),
  /duplicate trace for one:en/,
);
// The same case in the other language is a different trace, not a duplicate.
assert.equal(
  indexTraces(fixture(LANGUAGES.map((language) => (
    { caseId: "one", language, selectedTool: "compute", arguments: {} }
  ))), ids).size,
  LANGUAGES.length,
  "a case asked once per language files as one trace per language",
);
assert.throws(
  () => indexTraces(fixture([
    { caseId: "one", language: "en", selectedTool: "shell", arguments: {} },
  ]), ids),
  /known tool/,
);
// A trace document that does not say what produced it cannot be scored: the
// same numbers mean "the scorer works" or "the model performs this well"
// depending on an answer the file would not be carrying.
assert.throws(
  () => indexTraces({ schemaVersion: 1, traces: [] }, ids),
  /provenance block/,
);
assert.throws(
  () => indexTraces({ schemaVersion: 1, provenance: { source: "vibes" }, traces: [] }, ids),
  /provenance.source must be one of/,
);
// A model trace without the metadata that makes it re-runnable is not a
// measurement — a number nobody can reproduce or compare a later run against.
assert.throws(
  () => indexTraces({ schemaVersion: 1, provenance: { source: "model" }, traces: [] }, ids),
  /must record modelId, promptTemplateDigest, toolChoice, capturedAt/,
);
const modelDoc = {
  schemaVersion: 1,
  provenance: {
    source: "model",
    modelId: "claude-opus-5",
    promptTemplateDigest: "abc123",
    toolChoice: "auto",
    capturedAt: "2026-08-11T00:00:00Z",
    serverVersion: "0.3.0",
    engineVersion: "0.2.0-alpha.1",
    registryDigest: "a67241e0",
  },
  traces: [],
};
assert.deepEqual(indexTraces(modelDoc, ids).size, 0, "a complete model trace validates");
// The asymmetry that keeps a lucky run from becoming a committed claim.
assert.equal(mayAssertPerfect(fixture([])), true);
assert.equal(mayAssertPerfect(modelDoc), false);
console.log("evaluation contract rejection checks passed");
