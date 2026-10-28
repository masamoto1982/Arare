#!/usr/bin/env node
import { connectInMemory, readEval } from "./eval-common.js";

function numberResult(operation, operands) {
  const [left, right] = operands.map(Number);
  if (operation === "add") return left + right;
  if (operation === "divide") return left / right;
  if (operation === "modulo") return left % right;
  throw new Error(`unsupported Number baseline operation: ${operation}`);
}

function canonicalAjisaiScalar(value) {
  if (value?.type !== "number" || !value.value) return null;
  const { numerator, denominator } = value.value;
  return denominator === "1" ? numerator : `${numerator}/${denominator}`;
}

const corpus = readEval("./eval/number-baseline.json");
if (corpus.schemaVersion !== 1 || !Array.isArray(corpus.cases) || corpus.cases.length === 0) {
  throw new Error("Number baseline corpus is invalid");
}
const ids = new Set();
for (const testCase of corpus.cases) {
  if (!testCase.id || ids.has(testCase.id) || !Array.isArray(testCase.operands) ||
      testCase.operands.length !== 2 || typeof testCase.source !== "string" ||
      typeof testCase.expectedCanonical !== "string") {
    throw new Error(`invalid or duplicate Number baseline case: ${testCase.id}`);
  }
  ids.add(testCase.id);
}

const { client, close } = await connectInMemory("ajisai-number-baseline");
let ajisaiExact = 0;
let numberExact = 0;
const cases = [];
try {
  for (const testCase of corpus.cases) {
    const response = await client.callTool({ name: "compute", arguments: { source: testCase.source } });
    const ajisai = canonicalAjisaiScalar(response.structuredContent?.stack?.[0]);
    const number = String(numberResult(testCase.operation, testCase.operands));
    const ajisaiMatches = response.isError !== true && ajisai === testCase.expectedCanonical;
    const numberMatches = number === testCase.expectedCanonical;
    if (ajisaiMatches) ajisaiExact += 1;
    if (numberMatches) numberExact += 1;
    cases.push({
      id: testCase.id,
      expectedCanonical: testCase.expectedCanonical,
      ajisai,
      number,
      ajisaiMatches,
      numberMatches,
    });
  }
} finally {
  await close();
}

const report = {
  schemaVersion: 1,
  scope: "selected exact rational and decimal operations; not a general language benchmark",
  cases,
  metrics: {
    cases: cases.length,
    ajisaiCanonicalMatchRate: ajisaiExact / cases.length,
    javascriptNumberCanonicalMatchRate: numberExact / cases.length,
  },
};
console.log(JSON.stringify(report, null, 2));
if (process.argv.includes("--require-ajisai-exact") && ajisaiExact !== cases.length) process.exit(1);
