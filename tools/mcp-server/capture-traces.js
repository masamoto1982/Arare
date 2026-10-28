#!/usr/bin/env node
// Capture what a real model does with the Ajisai tools.
//
// Everything else in `eval/` grades a trace. Nothing produced one: the
// reference fixtures only prove the scorers run, and the readiness tracker has
// said for three rounds that the harness is not the bottleneck — real traces
// are. This is the missing half.
//
// It drives the actual model over the actual MCP tool definitions, one turn per
// corpus case per language, and writes a trace the scorers accept. A turn may
// hold several tool calls and all of them are recorded: a model that looks a
// Word up before computing has made one attempt, not a wrong choice.
//
// What it does *not* do is produce a number when it cannot measure one: with no
// resolvable credentials it exits non-zero having written nothing, because a
// file that looks like a trace and isn't is worse than no file.
//
// Usage:
//   node capture-traces.js [--model <id>] [--out <path>] [--limit <n>]
//
// `--limit` counts corpus cases, not prompts: each case is asked once per
// language, so `--limit 1` makes two requests.
//
// Credentials resolve the way the Anthropic SDK resolves them (ANTHROPIC_API_KEY,
// ANTHROPIC_AUTH_TOKEN, or an `ant auth login` profile) — this file reads none
// of them itself.

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { argValue, captureFailure, connectInMemory, digest, readEval } from "./eval-common.js";
import { engineVersion, registryDigest, serverVersion } from "./index.js";
import { LANGUAGES, validateCorpus } from "./evaluation-contract.js";

const here = dirname(fileURLToPath(import.meta.url));

/**
 * What the model is told, verbatim.
 *
 * Deliberately thin. The point of a baseline is to measure what the published
 * tool descriptions and quickstart achieve on their own — a system prompt that
 * coaches tool selection would measure the prompt instead, and every later
 * comparison would be against a moving target. Its digest is recorded with the
 * trace so a changed prompt cannot be mistaken for a changed model.
 */
export const SYSTEM_PROMPT =
  "You have access to Ajisai, a bounded exact-arithmetic engine, through the " +
  "tools below. Use a tool when it is the right instrument for the request, " +
  "and answer directly when it is not.";

export const DEFAULT_MODEL = "claude-opus-5";

/**
 * The tools, exactly as a connected client would see them.
 *
 * Read from the server's own declaration rather than restated here: a baseline
 * measured against a hand-copied description would drift from the product the
 * moment either changed, and would then be comparing two different things
 * across the PRs it exists to compare.
 */
export async function toolDefinitions() {
  const { client, close } = await connectInMemory("ajisai-trace-capture");
  try {
    const { tools } = await client.listTools();
    return tools.map(({ name, description, inputSchema }) => ({
      name,
      description,
      input_schema: inputSchema,
    }));
  } finally {
    await close();
  }
}

/**
 * One case: ask once, record what the model reached for.
 *
 * `tool_choice` is `auto` and must stay that way — a third of the corpus is
 * irrelevant-intent cases whose correct answer is *no* tool call, and forcing a
 * call would score the one metric that measures restraint as a guaranteed
 * failure.
 */
export async function captureCase(client, { model, tools, testCase, language }) {
  const response = await client.messages.create({
    model,
    // Thinking is on by default on current models and shares this budget with
    // the response, so the ceiling is generous: a truncated turn would be
    // recorded as a model that declined to call a tool.
    max_tokens: 16000,
    system: SYSTEM_PROMPT,
    tools,
    tool_choice: { type: "auto" },
    messages: [{ role: "user", content: testCase.prompts[language] }],
  });
  const toolUses = (response.content ?? []).filter((block) => block.type === "tool_use");
  const [first] = toolUses;
  return {
    caseId: testCase.id,
    // Every call the turn made, not only the first. The first capture threw
    // the rest away and the headline number measured the discard: 93 of 130
    // turns made two calls, overwhelmingly a `word_contract` lookup followed by
    // the `compute` that answers the request, and scoring the lookup as the
    // model's choice reported 32% tool selection for a model that had reached
    // the right tool in most of those turns. Looking a Word up before using it
    // is the behaviour the registry tool exists to invite.
    toolCalls: toolUses.map(({ name, input }) => ({ name, arguments: input ?? {} })),
    // Which asking this was. Both halves of a pair carry the same expectations,
    // so without this the two answers cannot be told apart and the comparison
    // the pairing exists for is unrecoverable after the run.
    language,
    // `null` is a real answer, not a missing one: it is what a correct response
    // to an irrelevant prompt looks like.
    selectedTool: first?.name ?? null,
    arguments: first?.input ?? {},
    // Observations the scorers do not consume yet, recorded because they cannot
    // be recovered later: a second call in one turn, and a turn that stopped
    // for a reason other than finishing.
    observed: { toolCalls: toolUses.length, stopReason: response.stop_reason ?? null },
  };
}

/**
 * Resolve the SDK client, or explain why there is nothing to run.
 *
 * Constructing a client does not check credentials — the SDK resolves them
 * when it builds the first request's headers, so a missing key surfaces as a
 * stack trace partway through a run rather than as a refusal to start. This
 * turns that into one actionable line, which `captureFailure` completes for
 * the failure that arrives late.
 */
async function connect() {
  let Anthropic;
  try {
    ({ default: Anthropic } = await import("@anthropic-ai/sdk"));
  } catch {
    throw new Error(
      "the Anthropic SDK is not installed; run `npm install` in tools/mcp-server",
    );
  }
  return new Anthropic();
}

export async function captureAll({
  client,
  model,
  tools,
  cases,
  languages = LANGUAGES,
  onCase = () => {},
}) {
  const traces = [];
  // Case-major rather than language-major, so a run interrupted partway leaves
  // both halves of every completed pair rather than a complete English half
  // and no Japanese one — a shape that scores as a language gap of 100%.
  for (const testCase of cases) {
    for (const language of languages) {
      const trace = await captureCase(client, { model, tools, testCase, language });
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

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const model = argValue(process.argv, "--model", DEFAULT_MODEL);
  const limit = Number(argValue(process.argv, "--limit", "0")) || Infinity;
  const corpus = readEval("./eval/cases.json");
  validateCorpus(corpus);
  const cases = corpus.cases.slice(0, limit);
  // Real traces live apart from the committed fixtures, so no directory listing
  // ever presents the two as the same kind of artifact.
  const out = argValue(
    process.argv,
    "--out",
    `${here}/eval/traces/${model}-${new Date().toISOString().replaceAll(":", "-")}.json`,
  );

  let document;
  try {
    const client = await connect();
    document = await captureAll({
      client,
      model,
      tools: await toolDefinitions(),
      cases,
      onCase: ({ caseId, language, selectedTool }) =>
        console.error(`  ${caseId} [${language}] -> ${selectedTool ?? "(no tool)"}`),
    });
  } catch (error) {
    console.error(captureFailure(error));
    process.exit(2);
  }
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, `${JSON.stringify(document, null, 2)}\n`);
  console.log(out);
  console.error(
    `captured ${document.traces.length} traces for ${model}; score with:\n` +
      `  node score-traces.js ${out}`,
  );
}
