#!/usr/bin/env node
// The repair capture harness, driven by a scripted client.
//
// Like `capture-traces.test.js`, this tests the harness and **not** a model:
// the client answers from a script. What it has to prove is the part a
// single-turn capture does not have — that the model is handed the real tool
// result between the two turns, and that a turn which produces no call is
// recorded rather than dropped.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { captureAllRepairs, captureRepair, connectMcp } from "./capture-repairs.js";
import { DEFAULT_MODEL, toolDefinitions } from "./capture-traces.js";
import { LANGUAGES, indexTraces, mayAssertPerfect, validateCorpus } from "./evaluation-contract.js";

const tools = await toolDefinitions();
const { mcp, close } = await connectMcp();

function scriptedClient(replies) {
  const requests = [];
  return {
    requests,
    messages: {
      create: async (request) => {
        requests.push(structuredClone(request));
        return replies[requests.length - 1];
      },
    },
  };
}

const call = (id, source) => ({
  content: [{ type: "tool_use", id, name: "compute", input: { source } }],
  stop_reason: "tool_use",
});
const noCall = { content: [{ type: "text", text: "I would rather not." }], stop_reason: "end_turn" };

const testCase = {
  id: "repair-unknown-word",
  prompts: { en: "Repair `1 2 AD` using its error report.", ja: "`1 2 AD` をエラー報告を使って直して。" },
};

// ── the loop: attempt, execute, hand the result back, attempt again ──────
const client = scriptedClient([call("t1", "1 2 AD"), call("t2", "1 2 ADD")]);
const trace = await captureRepair(client, mcp, {
  model: DEFAULT_MODEL,
  tools,
  testCase,
  language: "en",
});
assert.deepEqual(trace.firstAttempt, {
  selectedTool: "compute",
  arguments: { source: "1 2 AD" },
  toolCalls: [{ name: "compute", arguments: { source: "1 2 AD" } }],
});
assert.deepEqual(trace.repairedAttempt, {
  selectedTool: "compute",
  arguments: { source: "1 2 ADD" },
  toolCalls: [{ name: "compute", arguments: { source: "1 2 ADD" } }],
});
assert.equal(trace.language, "en");
assert.equal(trace.observed.sawDiagnosis, true, "the executed first attempt really did fail");

// The second turn must carry the *structured* failure, not a summary of it:
// the claim under test is that the error report drives the repair, so a harness
// that pre-digested it would be grading its own prose.
const [, secondRequest] = client.requests;
assert.equal(secondRequest.messages.length, 3, "attempt, assistant turn, tool result");
const [resultBlock] = secondRequest.messages[2].content;
assert.equal(resultBlock.type, "tool_result");
assert.equal(resultBlock.tool_use_id, "t1");
const delivered = JSON.parse(resultBlock.content);
assert.equal(delivered.status, "error");
assert.equal(delivered.error.category, "unknownWord");
assert.equal(
  delivered.error.word,
  "AD",
  "the model sees the whole structured error the server produced",
);

// ── every call in a turn is answered, not just the recorded one ──────────
// A model exploring two calls in one turn is ordinary, and the API rejects a
// follow-up that answers only the first: "`tool_use` ids were found without
// `tool_result` blocks immediately after". Answering one turned the entire
// capture into a 400 before this was fixed.
const twoCalls = {
  content: [
    { type: "tool_use", id: "u1", name: "compute", input: { source: "1 2 AD" } },
    { type: "tool_use", id: "u2", name: "compute", input: { source: "1 2 SUBTRACT" } },
  ],
  stop_reason: "tool_use",
};
const exploring = scriptedClient([twoCalls, call("u3", "1 2 ADD")]);
const explored = await captureRepair(exploring, mcp, {
  model: DEFAULT_MODEL,
  tools,
  testCase,
  language: "en",
});
assert.equal(explored.observed.firstTurnToolCalls, 2);
assert.deepEqual(
  explored.firstAttempt.toolCalls.map(({ arguments: args }) => args.source),
  ["1 2 AD", "1 2 SUBTRACT"],
  "both calls of the turn are recorded, in order",
);
assert.equal(
  explored.firstAttempt.selectedTool,
  "compute",
  "`selectedTool` stays the first call",
);

// The turn a model that checks before running produces: `check` on `1 ADD`
// reports nothing, because a stack underflow is a runtime fact, and the
// failing `compute` behind it is the attempt. Recording only the first call
// scored two cases 1.0 -> 0.0 on a strategy change that was an improvement.
const careful = scriptedClient([
  {
    content: [
      { type: "tool_use", id: "k1", name: "check", input: { source: "1 ADD" } },
      { type: "tool_use", id: "k2", name: "compute", input: { source: "1 ADD" } },
    ],
    stop_reason: "tool_use",
  },
  call("k3", "1 2 ADD"),
]);
const checked = await captureRepair(careful, mcp, {
  model: DEFAULT_MODEL,
  tools,
  testCase,
  language: "en",
});
assert.deepEqual(
  checked.firstAttempt.toolCalls.map(({ name }) => name),
  ["check", "compute"],
  "a check-then-run turn keeps the run that actually failed",
);
assert.equal(
  checked.observed.sawDiagnosis,
  true,
  "the turn saw a diagnosis even though its first call did not produce one",
);
const [, exploringFollowUp] = exploring.requests;
assert.deepEqual(
  exploringFollowUp.messages[2].content.map(({ type, tool_use_id }) => [type, tool_use_id]),
  [["tool_result", "u1"], ["tool_result", "u2"]],
  "every tool_use in the turn is answered, in order",
);

// ── a turn that calls nothing is a recorded failure, not a missing trace ──
const silent = await captureRepair(scriptedClient([noCall]), mcp, {
  model: DEFAULT_MODEL,
  tools,
  testCase,
  language: "ja",
});
assert.equal(silent.firstAttempt.selectedTool, null);
assert.deepEqual(silent.firstAttempt.toolCalls, []);
assert.equal(silent.repairedAttempt.selectedTool, null);
assert.equal(silent.observed.turns, 1, "there is no second turn without a first call");
assert.equal(silent.observed.sawDiagnosis, false);

// A model that attempts and then gives up is the other half of the same point.
const gaveUp = await captureRepair(scriptedClient([call("t1", "1 2 AD"), noCall]), mcp, {
  model: DEFAULT_MODEL,
  tools,
  testCase,
  language: "en",
});
assert.equal(gaveUp.firstAttempt.selectedTool, "compute");
assert.equal(gaveUp.repairedAttempt.selectedTool, null);

// ── what is written validates, and can never be asserted perfect ─────────
const corpus = JSON.parse(readFileSync(new URL("./eval/repair-cases.json", import.meta.url), "utf8"));
const ids = validateCorpus(corpus, { repair: true });
const document = await captureAllRepairs({
  client: scriptedClient([
    call("a", "1 2 AD"), call("b", "1 2 ADD"),
    call("c", "1 2 AD"), call("d", "1 2 ADD"),
  ]),
  mcp,
  model: DEFAULT_MODEL,
  tools,
  cases: [corpus.cases[0]],
});
assert.equal(
  indexTraces(document, ids, { repair: true }).size,
  LANGUAGES.length,
  "one case is repaired once per language",
);
assert.equal(document.provenance.source, "model");
assert.equal(mayAssertPerfect(document), false, "a capture is reported, never asserted perfect");

// And a document holding a give-up still validates, or the harness could only
// ever record the runs that went well.
assert.equal(
  indexTraces(
    { ...document, traces: [{ ...document.traces[0], repairedAttempt: { selectedTool: null, arguments: {} } }] },
    ids,
    { repair: true },
  ).size,
  1,
  "an unrepaired attempt is representable",
);

await close();
console.log("repair capture harness checks passed (scripted client — not a model measurement)");
