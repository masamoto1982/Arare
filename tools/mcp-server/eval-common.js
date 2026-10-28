// What the evaluation runners share: reading a corpus, resolving an `expect`
// pointer, connecting an in-process client to the real server, the capture
// scripts' command-line and failure plumbing, and the reference selection
// fixture. Development-only, like every runner that imports it; the published
// package ships none of them.

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { createServer } from "./index.js";
import { LANGUAGES } from "./evaluation-contract.js";

/** A JSON file relative to this directory (`./eval/cases.json`). */
export function readEval(relative) {
  return JSON.parse(readFileSync(new URL(relative, import.meta.url), "utf8"));
}

/** The value a JSON Pointer (`/stack/0/value`) names in `document`, or undefined. */
export function atPointer(document, pointer) {
  return pointer
    .split("/")
    .slice(1)
    .map((part) => part.replaceAll("~1", "/").replaceAll("~0", "~"))
    .reduce((value, part) => value?.[part], document);
}

/** An MCP client connected in-process to a fresh server; `close` ends both. */
export async function connectInMemory(name, version = "1") {
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const server = createServer();
  const client = new Client({ name, version });
  await Promise.all([server.connect(serverTransport), client.connect(clientTransport)]);
  return {
    client,
    close: async () => {
      await client.close();
      await server.close();
    },
  };
}

/** The short digest a capture records for its system prompt. */
export function digest(text) {
  return createHash("sha256").update(text).digest("hex").slice(0, 16);
}

/** The value after `flag` in `argv`, or `fallback` when the flag is absent. */
export function argValue(argv, flag, fallback) {
  const index = argv.indexOf(flag);
  return index === -1 ? fallback : argv[index + 1];
}

const MISSING_CREDENTIALS = /Could not resolve authentication|authentication_error|invalid x-api-key/i;

/** The message for a failed capture run. Nothing is ever written on this path. */
export function captureFailure(error) {
  if (MISSING_CREDENTIALS.test(error?.message ?? "")) {
    return (
      "no Anthropic credentials could be resolved. Set ANTHROPIC_API_KEY, or run `ant auth login`.\n" +
      "Nothing was written — a trace file that was not produced by a model is worse than no file."
    );
  }
  return `capture failed: ${error?.message ?? error}\nNothing was written.`;
}

/**
 * The selection reference fixture: the corpus answering itself.
 *
 * Every case, asked once per language, answered with the case's own reference
 * arguments — which is what a perfect trace *is*. Writing it by hand was
 * busywork that drifted (a case added to `cases.json` without a matching pair
 * scored as two missing traces, and `--require-perfect` then failed for a
 * reason unrelated to the scorer it asserts), and committing a generated copy
 * only moved the drift into a check. It is built from the corpus on demand
 * instead. Its job is to prove `score-traces.js` runs end to end, including the
 * per-language split; what it must never be mistaken for is a model result,
 * which is what `provenance.source` says.
 */
export function referenceTraces(corpus) {
  return {
    schemaVersion: 1,
    provenance: {
      source: "referenceFixture",
      note:
        "Tool-selection trace built from eval/cases.json to pass score-traces.js: every case " +
        "answered with its own reference arguments, in both languages. It proves the scorer runs " +
        "end to end; it is not a model result and its perfect score describes nothing about any model.",
    },
    traces: corpus.cases.flatMap((testCase) =>
      LANGUAGES.map((language) => ({
        caseId: testCase.id,
        language,
        selectedTool: testCase.expectedTool,
        // A negative case selects no tool and carries no arguments; the scorer
        // never calls anything for it, and `{}` keeps the trace shape uniform.
        arguments: testCase.arguments ?? {},
      })),
    ),
  };
}
