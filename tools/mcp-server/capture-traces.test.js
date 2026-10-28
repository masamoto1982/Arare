#!/usr/bin/env node
// The capture harness, driven by a scripted client.
//
// This tests the harness — prompt assembly, tool-call extraction, the shape of
// what gets written — and **not** a model: the client here returns canned
// responses. That distinction is the whole subject of this round of work, so
// the file states it rather than leaving a reader to infer it from the name.

import assert from "node:assert/strict";
import { captureAll, captureCase, DEFAULT_MODEL, SYSTEM_PROMPT, toolDefinitions } from "./capture-traces.js";
import { LANGUAGES, callsOf, indexTraces, mayAssertPerfect, validateCorpus } from "./evaluation-contract.js";

const tools = await toolDefinitions();
assert.deepEqual(
  tools.map(({ name }) => name).sort(),
  ["check", "compute", "infer_contracts", "word_contract"],
  "the model is offered exactly the tools the server publishes",
);
assert.ok(
  tools.every(({ description, input_schema }) => description?.length > 0 && input_schema?.type === "object"),
  "each definition carries the server's own description and input schema",
);

/** A client that answers from a script and records what it was asked. */
function scriptedClient(replies) {
  const requests = [];
  return {
    requests,
    messages: {
      create: async (request) => {
        requests.push(request);
        return replies[requests.length - 1];
      },
    },
  };
}

const compute = {
  content: [
    { type: "text", text: "I'll compute that." },
    { type: "tool_use", id: "t1", name: "compute", input: { source: "1 3 DIV" } },
  ],
  stop_reason: "tool_use",
};
const noTool = { content: [{ type: "text", text: "That isn't arithmetic." }], stop_reason: "end_turn" };

const client = scriptedClient([compute, noTool]);
const bilingual = {
  id: "exact-fraction",
  prompts: { en: "Compute one third exactly.", ja: "3 分の 1 を厳密に計算して。" },
};
const positive = await captureCase(client, {
  model: DEFAULT_MODEL,
  tools,
  testCase: bilingual,
  language: "en",
});
assert.deepEqual(positive.selectedTool, "compute");
assert.deepEqual(positive.arguments, { source: "1 3 DIV" });
assert.deepEqual(positive.observed, { toolCalls: 1, stopReason: "tool_use" });
assert.deepEqual(
  positive.toolCalls,
  [{ name: "compute", arguments: { source: "1 3 DIV" } }],
  "the whole turn is recorded, not only the call that happened to be first",
);
assert.equal(positive.language, "en", "a trace records which asking produced it");

// The negative half of the corpus. A model that answers an irrelevant prompt
// without touching Ajisai is *correct*, so this has to record `null` rather
// than treat the absence as a missing trace.
const negative = await captureCase(client, {
  model: DEFAULT_MODEL,
  tools,
  testCase: {
    id: "irrelevant-translation",
    prompts: { en: "Translate hello into French.", ja: "hello をフランス語に訳して。" },
  },
  language: "ja",
});
assert.equal(negative.selectedTool, null);
assert.deepEqual(negative.arguments, {});
assert.deepEqual(negative.toolCalls, [], "restraint records an empty turn, not a missing one");

// A turn holding a lookup and then the call that answers the request. Keeping
// only the first scored the lookup as the model's choice, which is what made
// the first real capture report 32% tool selection for a model that had
// reached the right tool in most of those very turns.
const exploring = scriptedClient([{
  content: [
    { type: "tool_use", id: "l1", name: "word_contract", input: { word: "FLOOR" } },
    { type: "tool_use", id: "c1", name: "compute", input: { source: "10 'A' BIND 4 'B' BIND A A B DIV FLOOR B MUL SUB" } },
  ],
  stop_reason: "tool_use",
}]);
const twoCall = await captureCase(exploring, {
  model: DEFAULT_MODEL,
  tools,
  testCase: { id: "integer-modulus", prompts: { en: "10 mod 4?", ja: "10 を 4 で割った余りは?" } },
  language: "en",
});
assert.deepEqual(
  twoCall.toolCalls.map(({ name }) => name),
  ["word_contract", "compute"],
  "both calls survive, in order",
);
assert.equal(twoCall.selectedTool, "word_contract", "`selectedTool` stays the first call");
assert.equal(callsOf(twoCall).length, 2);

const [request, japaneseRequest] = client.requests;
assert.equal(request.system, SYSTEM_PROMPT);
assert.equal(
  request.messages[0].content,
  bilingual.prompts.en,
  "the English asking sends the English prompt",
);
assert.ok(
  /フランス語/.test(japaneseRequest.messages[0].content),
  "the Japanese asking sends the Japanese prompt, not a translated-at-runtime one",
);
// Forcing a tool call would make every irrelevant-intent case an automatic
// failure of the one metric that measures restraint.
assert.deepEqual(request.tool_choice, { type: "auto" });
assert.equal(request.tools.length, 4);
assert.ok(!("temperature" in request), "sampling parameters are rejected by current models");

// What gets written must be a document the scorers accept — and must be
// classified as a model capture, so no run of it can ever be asserted perfect.
const corpus = JSON.parse(
  (await import("node:fs")).readFileSync(new URL("./eval/cases.json", import.meta.url), "utf8"),
);
const ids = validateCorpus(corpus);
const document = await captureAll({
  client: scriptedClient([compute, compute]),
  model: DEFAULT_MODEL,
  tools,
  cases: [corpus.cases[0]],
});
assert.equal(
  indexTraces(document, ids).size,
  LANGUAGES.length,
  "one case is asked once per language, and both askings validate as traces",
);
assert.deepEqual(
  document.traces.map(({ language }) => language),
  [...LANGUAGES],
  "a pair is captured case-major, so an interrupted run never leaves half a pair",
);
assert.equal(document.provenance.source, "model");
assert.equal(mayAssertPerfect(document), false, "a capture can be reported, never asserted perfect");
for (const field of ["modelId", "promptTemplateDigest", "toolChoice", "capturedAt", "serverVersion", "engineVersion", "registryDigest"]) {
  assert.ok(document.provenance[field], `provenance records ${field}`);
}

console.log("capture harness checks passed (scripted client — not a model measurement)");
