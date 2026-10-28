#!/usr/bin/env node
import { performance } from "node:perf_hooks";
import { atPointer, connectInMemory, readEval } from "./eval-common.js";
import { validateCorpus } from "./evaluation-contract.js";

function percentile(sorted, fraction) {
  return sorted[Math.max(0, Math.ceil(sorted.length * fraction) - 1)];
}

const corpus = readEval("./eval/cases.json");
validateCorpus(corpus);
const config = readEval("./eval/performance.json");
if (config.schemaVersion !== 1 || !Number.isInteger(config.measuredRuns) || config.measuredRuns < 1 ||
    !Number.isInteger(config.warmupRuns) || config.warmupRuns < 0 || !(config.p95BudgetMs > 0) ||
    !(config.medianResponseBytesBudget > 0) ||
    !Array.isArray(config.caseIds) || config.caseIds.length === 0) {
  throw new Error("performance configuration is invalid");
}
const cases = new Map(corpus.cases.map((testCase) => [testCase.id, testCase]));
const selected = config.caseIds.map((id) => {
  const testCase = cases.get(id);
  if (!testCase || testCase.expectedTool === null) throw new Error(`unknown performance case: ${id}`);
  return testCase;
});

const { client, close } = await connectInMemory("ajisai-performance-eval");
const measurements = [];
try {
  for (let round = -config.warmupRuns; round < config.measuredRuns; round += 1) {
    for (const testCase of selected) {
      const start = performance.now();
      const result = await client.callTool({
        name: testCase.expectedTool,
        arguments: testCase.arguments,
      });
      const elapsedMs = performance.now() - start;
      const semanticsOk = result.isError !== true && Object.entries(testCase.expect)
        .every(([pointer, expected]) =>
          JSON.stringify(atPointer(result.structuredContent, pointer)) === JSON.stringify(expected));
      if (!semanticsOk) throw new Error(`performance case failed: ${testCase.id}`);
      // What a caller actually receives, both halves of it: the structured
      // result and the text block mirroring it. Measuring only one would hide
      // the duplication that makes a response its real size, and measuring
      // nothing is how a response grows a third larger without anyone noticing
      // — which is what the two-space indentation this used to be serialized
      // with had already done.
      const responseBytes = Buffer.byteLength(JSON.stringify(result), "utf8");
      // The text half on its own, because it is the half that was padded and
      // the half a text-only client is limited to.
      const contentBytes = Buffer.byteLength(
        result.content?.map(({ text }) => text ?? "").join("") ?? "",
        "utf8",
      );
      if (round >= 0) {
        measurements.push({
          caseId: testCase.id,
          tool: testCase.expectedTool,
          elapsedMs,
          responseBytes,
          contentBytes,
        });
      }
    }
  }
} finally {
  await close();
}

const elapsed = measurements.map(({ elapsedMs }) => elapsedMs).sort((a, b) => a - b);
const byTool = Object.fromEntries([...new Set(measurements.map(({ tool }) => tool))].map((tool) => {
  const values = measurements.filter((sample) => sample.tool === tool)
    .map(({ elapsedMs }) => elapsedMs).sort((a, b) => a - b);
  return [tool, {
    samples: values.length,
    p50Ms: percentile(values, 0.5),
    p95Ms: percentile(values, 0.95),
    maxMs: values.at(-1),
  }];
}));
// One response per case, not per sample: repeated rounds of the same case
// return the same bytes, so a median over samples would just weight the cases
// by how often they ran.
const perCase = (field) => [...new Map(
  measurements.map((sample) => [sample.caseId, sample[field]]),
).values()].sort((a, b) => a - b);
const responseBytes = perCase("responseBytes");
const contentBytes = perCase("contentBytes");
const report = {
  schemaVersion: 1,
  measuredRuns: config.measuredRuns,
  cases: selected.length,
  samples: measurements.length,
  budget: {
    p95Ms: config.p95BudgetMs,
    medianResponseBytes: config.medianResponseBytesBudget,
  },
  overall: {
    p50Ms: percentile(elapsed, 0.5),
    p95Ms: percentile(elapsed, 0.95),
    maxMs: elapsed.at(-1),
  },
  responseSize: {
    medianBytes: percentile(responseBytes, 0.5),
    maxBytes: responseBytes.at(-1),
    medianContentBytes: percentile(contentBytes, 0.5),
    maxContentBytes: contentBytes.at(-1),
  },
  byTool,
};
console.log(JSON.stringify(report, null, 2));
if (process.argv.includes("--require-budget")) {
  const failures = [];
  if (report.overall.p95Ms > config.p95BudgetMs) {
    failures.push(`p95 ${report.overall.p95Ms.toFixed(1)} ms exceeds ${config.p95BudgetMs} ms budget`);
  }
  // A ceiling, not a target: it exists so a later change cannot quietly give
  // back what this one removed. Lowering it when a response genuinely shrinks
  // is the intended edit; raising it to make a regression pass is not.
  if (report.responseSize.medianBytes > config.medianResponseBytesBudget) {
    failures.push(
      `median response ${report.responseSize.medianBytes} bytes exceeds ${config.medianResponseBytesBudget} byte budget`,
    );
  }
  if (failures.length) {
    for (const failure of failures) console.error(failure);
    process.exit(1);
  }
}
