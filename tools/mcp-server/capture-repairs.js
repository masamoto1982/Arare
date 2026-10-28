#!/usr/bin/env node
// Capture whether a real model repairs a failure *from its error report*.
//
// `capture-traces.js` asks one question and records what the model reached for.
// A repair is not one question: the model attempts, the attempt fails, the
// failure carries a structured error (`message`, and `error` naming its
// category, Word and position), and the question is whether that report is
// what produces the corrected attempt. Nothing captured that —
// `score-repairs.js` has graded a hand-written fixture since it was written, so
// the repair rate has been "unmeasured" in the tracker for every round since.
//
// So this runs the loop for real: ask, execute the model's call against the
// actual server, hand the result back as a `tool_result`, and record the second
// attempt. Both attempts are recorded as *calls*, not as outcomes, because the
// scorer replays them itself — a capture that recorded its own verdict would be
// grading the model with the same code that produced its answer.
//
// Usage:
//   node capture-repairs.js [--model <id>] [--out <path>] [--limit <n>]
//
// Credentials resolve the way the Anthropic SDK resolves them; this file reads
// none of them itself.

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { argValue, captureFailure, connectInMemory, digest, readEval } from "./eval-common.js";
import { engineVersion, registryDigest, serverVersion } from "./index.js";
import { DEFAULT_MODEL, SYSTEM_PROMPT, toolDefinitions } from "./capture-traces.js";
import { LANGUAGES, validateCorpus } from "./evaluation-contract.js";

const here = dirname(fileURLToPath(import.meta.url));

/** Every `tool_use` block of a turn, in order. */
function toolUses(response) {
  return (response.content ?? []).filter((block) => block.type === "tool_use");
}

/** The first `tool_use` block of a turn, or `null` when the model made none. */
function firstToolUse(response) {
  return toolUses(response)[0] ?? null;
}

/**
 * What the model is handed back between the two turns.
 *
 * The whole structured result, serialized — not a summary and not the message
 * line. The claim under test is that the *diagnosis* drives the repair, and a
 * harness that pre-digested the failure into prose would be testing its own
 * summary instead. This is what a connected MCP client would deliver.
 */
function toolResultBlock(toolUseId, result) {
  return {
    type: "tool_result",
    tool_use_id: toolUseId,
    content: JSON.stringify(result?.structuredContent ?? result ?? null),
    is_error: false,
  };
}

/**
 * One repair case, in one language: two turns with a real tool execution
 * between them.
 */
export async function captureRepair(client, mcp, { model, tools, testCase, language }) {
  const messages = [{ role: "user", content: testCase.prompts[language] }];
  const first = await client.messages.create({
    model,
    max_tokens: 16000,
    system: SYSTEM_PROMPT,
    tools,
    tool_choice: { type: "auto" },
    messages,
  });
  const firstUse = firstToolUse(first);
  // A turn is one attempt, and it may hold several calls — the same reading
  // `score-traces.js` was corrected to in the first baseline round. Recording
  // only the first `tool_use` mis-scored a model that got *better*: after the
  // entry-surface round it began statically checking before running, and
  // `check` on `1 ADD` reports nothing, because a stack underflow is a runtime
  // fact. The failing `compute` right behind it was the attempt, and the
  // harness threw it away.
  const attempt = (uses) => ({
    selectedTool: uses[0]?.name ?? null,
    arguments: uses[0]?.input ?? {},
    toolCalls: uses.map(({ name, input }) => ({ name, arguments: input ?? {} })),
  });

  // No first call means no failure to read a diagnosis from, so there is no
  // second turn to run. Recorded rather than skipped: a model that never
  // attempts is a repair failure, and a harness that could only capture
  // attempts would report a rate computed over the cases that went well.
  if (!firstUse) {
    return {
      caseId: testCase.id,
      language,
      firstAttempt: attempt([]),
      repairedAttempt: attempt([]),
      observed: { turns: 1, stopReason: first.stop_reason ?? null, sawDiagnosis: false },
    };
  }

  // Every `tool_use` in the turn gets executed and answered, not just the one
  // recorded as the attempt: the API requires a `tool_result` for each block,
  // and a model exploring two calls in one turn is common enough that
  // answering only the first turned the whole capture into a 400. The recorded
  // attempt stays the first, matching `capture-traces.js`.
  const uses = toolUses(first);
  const results = [];
  for (const use of uses) {
    let outcome = null;
    try {
      outcome = await mcp.callTool({ name: use.name, arguments: use.input ?? {} });
    } catch {
      outcome = null;
    }
    results.push({ use, outcome });
  }
  const result = results[0].outcome;

  messages.push({ role: "assistant", content: first.content });
  messages.push({
    role: "user",
    content: results.map(({ use, outcome }) => toolResultBlock(use.id, outcome)),
  });

  const second = await client.messages.create({
    model,
    max_tokens: 16000,
    system: SYSTEM_PROMPT,
    tools,
    tool_choice: { type: "auto" },
    messages,
  });
  return {
    caseId: testCase.id,
    language,
    firstAttempt: attempt(uses),
    repairedAttempt: attempt(toolUses(second)),
    observed: {
      turns: 2,
      firstTurnToolCalls: uses.length,
      stopReason: second.stop_reason ?? null,
      // Whether *any* call in the first turn failed. A turn where none did
      // never tested a repair, and the scorer will score it as no diagnosis
      // observed — this records why.
      sawDiagnosis: results.some(
        ({ outcome }) => outcome?.structuredContent?.status === "error",
      ),
    },
  };
}

export async function captureAllRepairs({
  client,
  mcp,
  model,
  tools,
  cases,
  languages = LANGUAGES,
  onCase = () => {},
}) {
  const traces = [];
  for (const testCase of cases) {
    for (const language of languages) {
      const trace = await captureRepair(client, mcp, { model, tools, testCase, language });
      traces.push(trace);
      onCase(trace);
    }
  }
  return {
    schemaVersion: 1,
    provenance: {
      source: "model",
      modelId: model,
      promptTemplateDigest: digest(SYSTEM_PROMPT),
      toolChoice: "auto",
      capturedAt: new Date().toISOString(),
      serverVersion: serverVersion(),
      engineVersion: engineVersion(),
      registryDigest: registryDigest(),
    },
    traces,
  };
}

/** An in-process MCP client over the real server, for the middle of the loop. */
export async function connectMcp() {
  const { client, close } = await connectInMemory("ajisai-repair-capture");
  return { mcp: client, close };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const model = argValue(process.argv, "--model", DEFAULT_MODEL);
  const limit = Number(argValue(process.argv, "--limit", "0")) || Infinity;
  const corpus = readEval("./eval/repair-cases.json");
  validateCorpus(corpus, { repair: true });
  const cases = corpus.cases.slice(0, limit);
  const out = argValue(
    process.argv,
    "--out",
    `${here}/eval/traces/${model}-repairs-${new Date().toISOString().replaceAll(":", "-")}.json`,
  );

  let document;
  let session;
  try {
    const { default: Anthropic } = await import("@anthropic-ai/sdk");
    session = await connectMcp();
    document = await captureAllRepairs({
      client: new Anthropic(),
      mcp: session.mcp,
      model,
      tools: await toolDefinitions(),
      cases,
      onCase: ({ caseId, language, repairedAttempt, observed }) =>
        console.error(
          `  ${caseId} [${language}] diagnosis=${observed.sawDiagnosis} ` +
            `retry=${repairedAttempt.selectedTool ?? "(none)"}`,
        ),
    });
  } catch (error) {
    console.error(captureFailure(error));
    process.exit(2);
  } finally {
    await session?.close();
  }
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, `${JSON.stringify(document, null, 2)}\n`);
  console.log(out);
  console.error(
    `captured ${document.traces.length} repair traces for ${model}; score with:\n` +
      `  node score-repairs.js ${out}`,
  );
}
