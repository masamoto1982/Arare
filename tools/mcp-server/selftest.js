#!/usr/bin/env node
import Ajv2020 from "ajv/dist/2020.js";
import { atPointer, connectInMemory } from "./eval-common.js";
import {
  CAPACITY_WAIT_MS,
  createBackend,
  ExecutionGate,
  LIMIT_PROFILE,
  LIMITS,
  serverVersion,
} from "./index.js";
import { runCli } from "./doctor.js";
import { coveredLimitNames, limitCases } from "./golden/limit-cases.js";
import { HOST_ERRORS } from "./host-error.js";
import { SKILL_BOUNDARY } from "./sync-assets.js";
import { NativeCliBackend } from "./backend/native-cli.js";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

let failures = 0;
function check(label, condition) {
  console.log(`${condition ? "PASS" : "FAIL"}  ${label}`);
  if (!condition) failures += 1;
}

function canonicalJson(value) {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, child]) => [key, canonicalJson(child)]),
    );
  }
  return value;
}

const gate = new ExecutionGate(2);
check("execution gate admits up to its capacity", gate.tryAcquire() && gate.tryAcquire());
check("execution gate rejects excess concurrent work", gate.tryAcquire() === false);
gate.release();
check("execution gate restores capacity on release", gate.tryAcquire());
gate.release();
gate.release();

// Back-pressure: a saturated gate holds a caller briefly rather than turning
// every burst into a retry loop, and a released slot goes to whoever has been
// waiting longest.
const queued = new ExecutionGate(1);
queued.tryAcquire();
const waiting = queued.acquire(1_000);
queued.release();
check("execution gate hands a released slot to a queued caller", (await waiting) === true);
queued.release();
const timedOut = new ExecutionGate(1);
timedOut.tryAcquire();
check("execution gate gives up after its wait window", (await timedOut.acquire(20)) === false);
timedOut.release();

const { client, close } = await connectInMemory("selftest", "0.0.0");

const { tools } = await client.listTools();
check(
  "exposes the four focused agent tools",
  JSON.stringify(tools.map(({ name }) => name).sort()) ===
    JSON.stringify(["check", "compute", "infer_contracts", "word_contract"]),
);
check(
  "every tool rejects undeclared input",
  tools.every(({ inputSchema }) => inputSchema.additionalProperties === false),
);
check(
  "every tool advertises the same structured output contract",
  tools.every(({ outputSchema }) =>
    JSON.stringify(canonicalJson(outputSchema)) === JSON.stringify(canonicalJson(tools[0].outputSchema)) &&
    outputSchema?.required?.includes("status")
  ),
);
// One `$id` embedded once per tool is a duplicate schema to any client that
// registers compiled validators by `$id`; the embedded copies carry none.
check(
  "an embedded output schema carries no $id a validator cache would collide on",
  tools.every(({ outputSchema }) => !("$id" in (outputSchema ?? {}))) &&
    (() => {
      const ajv = new Ajv2020();
      for (const { outputSchema } of tools) ajv.compile(outputSchema);
      return true;
    })(),
);
check(
  "the source input schema states the unit its limit is measured in",
  tools
    .filter(({ name }) => name !== "word_contract")
    .every(({ inputSchema }) =>
      inputSchema.properties.source.maxLength === LIMITS.sourceBytes &&
      inputSchema.properties.source.description.includes("UTF-8 bytes")
    ),
);
check(
  "tools advertise safe selection hints",
  tools.every(({ annotations }) =>
    annotations?.readOnlyHint === true &&
    annotations?.destructiveHint === false &&
    annotations?.idempotentHint === true &&
    annotations?.openWorldHint === false
  ),
);

const contract = await client.callTool({ name: "word_contract", arguments: { word: "map" } });
check(
  "word_contract returns the complete canonical contract",
  contract.structuredContent?.matches?.some((entry) =>
    entry.name === "MAP" &&
    entry.purity === "conditional" &&
    entry.stack?.inputs === 2
  ),
);
const missWithSuggestion = await client.callTool({
  name: "word_contract",
  arguments: { word: "LENGHT" },
});
check(
  "word_contract answers an unmatched name with its closest known Words",
  missWithSuggestion.structuredContent?.matches?.length === 0 &&
    missWithSuggestion.structuredContent?.suggestions?.[0] === "LENGTH",
);
check(
  "word_contract carries the same provenance as an execution result",
  contract.structuredContent?.mcp?.assetDigest?.length === 64 &&
    !("registryDigest" in (contract.structuredContent?.mcp ?? {})) &&
    contract.structuredContent?.mcp?.serverVersion === serverVersion() &&
    contract.structuredContent?.suggestions?.length === 0,
);
// A Word's cost is what lets a caller budget a phrase before running it, so it
// has to be on the entry the caller already reads — not derivable only by
// executing something.
check(
  "word_contract publishes what the Word charges, per axis",
  contract.structuredContent?.matches?.[0]?.cost?.steps?.class === "unbounded" &&
    contract.structuredContent?.matches?.[0]?.cost?.numeric?.class === "unbounded" &&
    contract.structuredContent?.matches?.[0]?.cost?.collection?.class === "unbounded",
);
const addContract = await client.callTool({ name: "word_contract", arguments: { word: "ADD" } });
// `MAP` is unbounded on every axis, so it cannot show that the axes are read
// independently; `ADD` is the case where they differ, and its `numeric` bound
// is the one `runtime_limits.rs` prices limb×limb and therefore attains.
check(
  "a Word's cost axes are published independently, with their exactness",
  (() => {
    const cost = addContract.structuredContent?.matches?.[0]?.cost;
    return cost?.steps?.class === "const" && cost.steps.exact === true &&
      cost.numeric?.class === "linear" && cost.numeric.exact === true &&
      cost.collection?.class === "const" && cost.collection.exact === true;
  })(),
);

const resources = await client.listResources();
check(
  "publishes guide, vocabulary, contracts, result schema, host profile and phrasebook as resources",
  JSON.stringify(resources.resources.map(({ uri }) => uri).sort()) ===
    JSON.stringify([
      "ajisai://contracts",
      "ajisai://guide/quickstart",
      "ajisai://limits",
      "ajisai://phrasebook",
      "ajisai://schema/result",
      "ajisai://vocabulary",
    ]),
);
// The vocabulary is the inventory, not the repository's bookkeeping: retired
// classification axes and source paths served as-is read to an agent as live
// language concepts.
const vocabularyResource = JSON.parse(
  (await client.readResource({ uri: "ajisai://vocabulary" })).contents[0]?.text ?? "{}",
);
// The versions a result must name are the repository's, read where they are
// written rather than repeated here.
const rootPackageVersion = JSON.parse(readFileSync(new URL("../../package.json", import.meta.url), "utf8")).version;
const lockedSpecificationVersion = JSON.parse(
  readFileSync(new URL("../../spec/version-lock.json", import.meta.url), "utf8"),
).specificationVersion;
const registryNames = JSON.parse(readFileSync(new URL("./assets/words.json", import.meta.url), "utf8"))
  .entries.map(({ name }) => name).sort();
check(
  "the vocabulary resource lists every Word, and only name, kind, family and tier",
  vocabularyResource.wordCount === registryNames.length &&
    JSON.stringify(vocabularyResource.entries.filter(({ kind }) => kind === "coreword").map(({ name }) => name).sort()) ===
      JSON.stringify(registryNames) &&
    vocabularyResource.entries.every((entry) =>
      Object.keys(entry).every((key) => ["name", "kind", "family", "vocabularyTier"].includes(key))
    ),
);
// Bounding a phrase means joining the bounds of every Word in it, which 65
// separate `word_contract` probes cannot practically deliver. The bulk read is
// what makes the published cost usable, so it is checked as such: every entry
// carries a complete, well-formed cost.
const contractsResource = JSON.parse(
  (await client.readResource({ uri: "ajisai://contracts" })).contents[0]?.text ?? "{}",
);
const COST_CLASSES = ["const", "linear", "superlinear", "unbounded"];
check(
  "the contracts resource serves every Word's cost in one read",
  Array.isArray(contractsResource.entries) &&
    contractsResource.entries.length > 0 &&
    contractsResource.entries.every((entry) =>
      ["steps", "numeric", "collection"].every((axis) =>
        COST_CLASSES.includes(entry.cost?.[axis]?.class) &&
        typeof entry.cost?.[axis]?.exact === "boolean"
      )
    ),
);
check(
  "the contracts resource and word_contract answer with the same entry",
  JSON.stringify(contractsResource.entries.find((entry) => entry.name === "MAP")) ===
    JSON.stringify(contract.structuredContent?.matches?.[0]),
);
const limitsResource = JSON.parse(
  (await client.readResource({ uri: "ajisai://limits" })).contents[0]?.text ?? "{}",
);
check(
  "the host profile resource serves exactly the limits tool results report",
  limitsResource.profile === "mcp-local-stdio" &&
    JSON.stringify(limitsResource.limits) === JSON.stringify(LIMITS),
);
// The adapter and the engine are separately versioned, and a stored result
// naming only the engine cannot say which adapter produced it.
check(
  "the host profile resource names the adapter version as well as the engine",
  limitsResource.serverVersion === serverVersion() &&
    limitsResource.engineVersion !== undefined &&
    typeof serverVersion() === "string",
);
const guide = await client.readResource({ uri: "ajisai://guide/quickstart" });
const guideText = guide.contents[0]?.text ?? "";
check("quickstart resource reads generated guidance", guideText.includes("Agent Writing Protocol"));

// The served guide leads with the MCP interface, not with the CLI run loop the
// generated protocol opens on. A connected client cannot issue `ajisai run`,
// and a model that met that first learned the language before learning which
// of the tools to call.
const [preface] = guideText.split(SKILL_BOUNDARY);
check(
  "the quickstart opens with MCP tool selection, not the CLI run loop",
  preface.length > 0 && preface.length < guideText.length &&
    ["compute", "check", "infer_contracts", "word_contract"].every((tool) =>
      preface.includes(`\`${tool}\``)
    ) &&
    !preface.includes("ajisai run"),
);
check(
  "the quickstart distinguishes every outcome class a caller must branch on",
  ["`ok`", "`error`", "`hostError`", "NIL", "exactTerms"].every((token) =>
    preface.includes(token)
  ),
);
// The algebraic section teaches the exact terms before the approximation that
// misleads, so the order is part of the guidance rather than a stylistic
// choice.
const algebraicSection = preface
  .split(/^## /m)
  .find((section) => section.includes("exactTerms") && section.includes("approximate")) ?? "";
check(
  "the quickstart teaches the exact terms before the approximation",
  algebraicSection.length > 0 &&
    algebraicSection.indexOf("exactTerms") < algebraicSection.indexOf("approximate") &&
    algebraicSection.includes("stackDisplay"),
);

// Every example in the hand-written preface runs against the live backend. The
// generated half is verified by its generator against the real interpreter;
// this is the same guarantee for the half a human wrote, so guidance the model
// is told to trust cannot quietly stop being true.
const prefaceExamples = [...preface.matchAll(/^```ajisai([^\n]*)\n([\s\S]*?)^```$/gm)].map(
  ([, attributes, source]) => ({
    source: source.trim(),
    expect: Object.fromEntries(
      [...attributes.matchAll(/(\w+)=("[^"]*"|\S+)/g)].map(([, key, raw]) => [
        key,
        raw.startsWith('"') ? raw.slice(1, -1) : raw,
      ]),
    ),
  }),
);
check("the quickstart preface carries executable examples", prefaceExamples.length >= 5);
for (const { source, expect } of prefaceExamples) {
  const observed = await client.callTool({
    name: expect.tool ?? "compute",
    arguments: { source },
  });
  const structured = observed.structuredContent;
  const statusAgrees = structured?.status === (expect.status ?? "ok");
  const stackAgrees =
    expect.stack === undefined || (structured?.stackDisplay ?? []).join(" ") === expect.stack;
  check(`quickstart example: ${source.replaceAll("\n", " ")}`, statusAgrees && stackAgrees);
  if (!statusAgrees || !stackAgrees) {
    console.error(`  got status=${structured?.status} stack=${JSON.stringify(structured?.stackDisplay)}`);
  }
}
const schemaResource = await client.readResource({ uri: "ajisai://schema/result" });
const resultSchema = JSON.parse(schemaResource.contents[0]?.text ?? "{}");
const validateResult = new Ajv2020().compile(resultSchema);
check(
  "result resource publishes the exact algebraic wire schema",
  resultSchema.$id === "ajisai://schema/result" &&
    resultSchema.$defs?.exactTerm?.required?.includes("radicand"),
);
check(
  "tool output and result resource use the same schema",
  JSON.stringify(canonicalJson(tools.find(({ name }) => name === "compute")?.outputSchema)) ===
    JSON.stringify(canonicalJson((({ $id, ...rest }) => rest)(resultSchema))),
);
const templates = await client.listResourceTemplates();
check(
  "publishes canonical Word contracts as a resource template",
  templates.resourceTemplates.some(({ uriTemplate }) => uriTemplate === "ajisai://words/{name}"),
);
const mapResource = await client.readResource({ uri: "ajisai://words/MAP" });
const mapContract = JSON.parse(mapResource.contents[0]?.text ?? "{}");
check(
  "Word resource resolves the canonical contract",
  mapContract.matches?.[0]?.name === "MAP" && mapContract.matches[0]?.nilPolicy,
);

const golden = JSON.parse(
  readFileSync(new URL("./golden/cases.json", import.meta.url), "utf8"),
);
for (const goldenCase of golden.cases) {
  const observed = await client.callTool({
    name: "compute",
    arguments: { source: goldenCase.source },
  });
  const mismatches = Object.entries(goldenCase.expect).filter(
    ([pointer, expected]) =>
      JSON.stringify(atPointer(observed.structuredContent, pointer)) !==
      JSON.stringify(expected),
  );
  // A field that must *not* be there. `expect` cannot say this: a missing
  // pointer and a pointer holding `null` both stringify to the same thing, so
  // "absent" and "present and null" were indistinguishable. `exactTerms` is
  // meaningless on a rational — terms of a number that has no radical would
  // be a field inviting a reader to wonder what it means — and this is what
  // pins its absence.
  const unexpected = (goldenCase.expectAbsent ?? []).filter(
    (pointer) => atPointer(observed.structuredContent, pointer) !== undefined,
  );
  check(
    `golden: ${goldenCase.name}`,
    observed.isError !== true && mismatches.length === 0 && unexpected.length === 0,
  );
  if (mismatches.length || unexpected.length) {
    console.error(`  mismatched=${JSON.stringify(mismatches)} unexpectedly present=${JSON.stringify(unexpected)}`);
  }
}

// Every declared limit must be accounted for by name. A limit an agent plans
// against and a limit the regression suite pins have to be the same set —
// otherwise the server can add a ceiling nobody ever exercises, which is how
// `responseBytes` came to be declared for two backends and enforced by one.
check(
  "every declared limit has a coverage entry, and every entry a declared limit",
  JSON.stringify(coveredLimitNames()) === JSON.stringify(Object.keys(LIMITS).sort()),
);
for (const limit of limitCases()) {
  if (limit.coverage === "boundary") {
    check(
      `limit ${limit.name}: declared value matches the served profile`,
      limit.declared === LIMITS[limit.name],
    );
    for (const probe of limit.probes) {
      const observed = await client.callTool({
        name: "compute",
        arguments: { source: probe.source },
      });
      const mismatches = Object.entries(probe.expect).filter(
        ([pointer, expected]) =>
          JSON.stringify(atPointer(observed.structuredContent, pointer)) !==
          JSON.stringify(expected),
      );
      check(`limit ${limit.name} (${probe.edge})`, mismatches.length === 0);
      if (mismatches.length) {
        const actual = mismatches.map(([pointer]) => [pointer, atPointer(observed.structuredContent, pointer)]);
        console.error(`  expected ${JSON.stringify(mismatches)}, got ${JSON.stringify(actual)}`);
      }
    }
  } else {
    check(
      `limit ${limit.name}: declared value matches the served profile`,
      limit.declared === LIMITS[limit.name],
    );
    check(
      `limit ${limit.name}: non-boundary coverage explains itself`,
      typeof limit.note === "string" && limit.note.length > 0 &&
        (limit.coverage !== "injectedLimit" || typeof limit.rustTest === "string"),
    );
  }
}

// `responseBytes` bounds the response as sent — the result twice (structured
// and serialized) plus provenance — not the one copy a backend produces. The
// gap between the two used to let a 2.29 MB answer through a 1 MiB ceiling.
// A success that would not fit is no longer refused: the engine elides the
// slots that exceed its stack budget (`valueStackBudget`) and sends the rest
// whole, so the caller learns what it left on the stack rather than only that
// its answer was large. `0 5000 RANGE` (433 KB of stack) still arrives whole.
for (const [source, mustElide] of [["0 5000 RANGE", false], ["0 7000 RANGE 1 2 ADD", true], ["0 20000 RANGE", true]]) {
  const observed = await client.callTool({ name: "compute", arguments: { source } });
  const sent = Buffer.byteLength(
    JSON.stringify({ content: observed.content, structuredContent: observed.structuredContent }),
    "utf8",
  );
  const result = observed.structuredContent;
  const elided = result?.stackElided?.reason === "valueStackBudget";
  check(
    `responseBytes bounds the whole response to \`${source}\` (${sent} bytes sent${elided ? ", elided" : ""})`,
    sent <= LIMITS.responseBytes && result?.status === "ok" && elided === mustElide &&
      (!mustElide || (result.stack?.[0]?.elided?.reason === "valueStackBudget" &&
        result.stack[0].value === null && result.stackDisplay?.[0]?.startsWith("<elided vector of"))) &&
      validateResult(result),
  );
}
check(
  "an elided success keeps the values that fit, whole",
  (await client.callTool({ name: "compute", arguments: { source: "0 7000 RANGE 1 2 ADD" } }))
    .structuredContent?.stackDisplay?.[1] === "3/1",
);
// The ceiling itself is still enforced, through the backend's own measure:
// a backend built with a response budget no result fits refuses the way the
// declared one would.
const narrow = createBackend({ responseBytes: 256 });
if (narrow) {
  let refused = null;
  try {
    await narrow.compute("0 10 RANGE");
  } catch (error) {
    refused = error;
  }
  check(
    "limit responseBytes (hostGate): a backend refuses a result its budget cannot hold",
    refused?.code === "responseTooLarge" && refused?.retryable === false,
  );
} else {
  check("limit responseBytes (hostGate): a backend is available to exercise it", false);
}

// `wallTimeMs` is a deadline the adapter holds around execution, not a budget
// the engine spends, and since the collection meter landed no source program
// reaches it — the named ceilings answer first, which is the outcome they exist
// for. So it is exercised the way the other host gate is: through the server's
// own machinery, with the deadline moved to where a real program misses it. A
// ceiling that cannot be observed at all is a claim, whichever kind it is.
const impatient = createBackend({ wallTimeMs: 1 });
if (impatient) {
  let timedOut = null;
  try {
    await impatient.compute("0 99999 RANGE SORT LENGTH");
  } catch (error) {
    timedOut = error;
  }
  check(
    "limit wallTimeMs (hostGate): the adapter deadline answers `timeout`",
    timedOut?.code === "timeout" && timedOut?.retryable === true,
  );
} else {
  check("limit wallTimeMs (hostGate): a backend is available to exercise it", false);
}

// A native child that dies of a signal the deadline did not send crashed: it is
// a non-retryable `backendFailure`, not a `timeout` claiming the wall time ran
// out after a few milliseconds.
if (process.platform !== "win32") {
  const crashDir = mkdtempSync(join(tmpdir(), "ajisai-selftest-"));
  const crashBin = join(crashDir, "crash.sh");
  writeFileSync(crashBin, "#!/bin/sh\nkill -SEGV $$\n", { mode: 0o755 });
  const crashing = new NativeCliBackend({ bin: crashBin, wallTimeMs: 5_000, responseBytes: 1_000, executionSteps: 1 });
  let crashed = null;
  try {
    await crashing.compute("1");
  } catch (error) {
    crashed = error;
  }
  rmSync(crashDir, { recursive: true, force: true });
  check(
    "a native backend killed by a signal it was not sent is a backendFailure, not a timeout",
    crashed?.code === "backendFailure" && crashed?.retryable === false,
  );
}

// The two halves of a result, and the padding between them.
//
// MCP asks a tool with an output schema to also return the serialized JSON in
// a text block, so a text-only client receives the whole result rather than a
// summary of it. That makes the text a *mirror*: if it ever stops being the
// same object, the two kinds of client stop seeing the same answer. These pin
// the mirror, and pin the padding that used to cost a third of it.
const mirrored = await client.callTool({ name: "compute", arguments: { source: "1 3 DIV" } });
check(
  "the text block is the structured result, not a summary of it",
  JSON.stringify(JSON.parse(mirrored.content?.[0]?.text ?? "null")) ===
    JSON.stringify(mirrored.structuredContent),
);
check(
  "the text block carries no indentation padding",
  typeof mirrored.content?.[0]?.text === "string" &&
    !/\n\s/.test(mirrored.content[0].text),
);
// An optional field with nothing in it is absent, not `null`. A plain success
// used to advertise `message`, `error` and `contractDecls`, all empty —
// diagnostic-sounding fields pointing at nothing.
check(
  "a successful result carries no null-valued fields",
  Object.values(mirrored.structuredContent ?? {}).every((field) => field !== null) &&
    !("error" in (mirrored.structuredContent ?? {})),
);
// Compaction must not cost a text-only client the outcome distinction, which
// is the whole reason the text is the serialized result rather than prose.
for (const [label, call] of [
  ["a value", { name: "compute", arguments: { source: "1 3 DIV" } }],
  ["a reason-carrying NIL", { name: "compute", arguments: { source: "-1 SQRT" } }],
  ["a language error", { name: "compute", arguments: { source: "FROBNICATE" } }],
  ["a host failure", { name: "compute", arguments: { source: 42 } }],
]) {
  const observed = await client.callTool(call);
  const text = JSON.parse(observed.content?.[0]?.text ?? "null");
  check(
    `a text-only client can still identify ${label}`,
    typeof text?.status === "string" &&
      text.status === observed.structuredContent?.status &&
      (text.status !== "hostError" || typeof text.error?.code === "string") &&
      (text.status !== "error" || typeof text.error?.category === "string"),
  );
}

// A run names its outcome, so the three results are told apart without
// reading the top stack node; an error's outcome and its `error.category` are
// one category.
for (const [source, expected] of [
  ["1 3 DIV", "value"],
  ["-1 SQRT", "nil:domainMiss"],
  ["1 0 DIV", "value"],
  ["NIL", "nil:literal"],
  ["FROBNICATE", "error:unknownWord"],
  ["1 ADD", "error:stackUnderflow"],
  ["[ 1 2", "error:malformedSource"],
]) {
  const run = await client.callTool({ name: "compute", arguments: { source } });
  check(
    `compute names the outcome of \`${source}\` as ${expected}`,
    run.structuredContent?.outcome === expected &&
      (!expected.startsWith("error:") ||
        run.structuredContent?.error?.category === expected.slice("error:".length)) &&
      validateResult(run.structuredContent),
  );
}

// `words` defines phrasebook Words before the source, names each with the
// digest its `#:digest` line records, and keeps error lines the caller's own.
{
  const phrasebookText = readFileSync(new URL("./assets/phrasebook.ajisai", import.meta.url), "utf8");
  const recordedDigest = (name) =>
    phrasebookText.split("\n").find((line) => line.startsWith(`#:digest ${name} `))?.split(" ")[2];
  const mean = await client.callTool({ name: "compute", arguments: { source: "[ 1 2 3 ] MEAN", words: ["mean"] } });
  check(
    "words defines a phrasebook Word, and the Words it calls, before the source",
    mean.structuredContent?.status === "ok" &&
      mean.structuredContent?.stackDisplay?.[0] === "2/1" &&
      JSON.stringify(mean.structuredContent?.prelude?.map((p) => p.name)) === JSON.stringify(["SUM", "MEAN"]),
  );
  check(
    "each prelude digest is the one ajisai://phrasebook records",
    mean.structuredContent?.prelude?.every((p) => p.digest === recordedDigest(p.name)),
  );
  const resource = await client.readResource({ uri: "ajisai://phrasebook" });
  check("ajisai://phrasebook serves the packaged phrasebook", resource.contents?.[0]?.text === phrasebookText);
  const failing = await client.callTool({ name: "compute", arguments: { source: "1\n[ 1 2 ] MEAN 0 FOO", words: ["MEAN"] } });
  check(
    "an error under words counts its line from the first line of source",
    failing.structuredContent?.error?.line === 2,
  );
  const inferred = await client.callTool({ name: "infer_contracts", arguments: { source: "[ MEAN 2 MUL ] 'DOUBLE-MEAN' DEF", words: ["MEAN"] } });
  check(
    "infer_contracts under words reports only the caller's Words",
    JSON.stringify(inferred.structuredContent?.contracts?.map((c) => c.name)) === JSON.stringify(["DOUBLE-MEAN"]),
  );
  const checked = await client.callTool({ name: "check", arguments: { source: "[ 1 2 ] MEAN", words: ["MEAN"] } });
  check(
    "check under words reports no declaration the caller did not write",
    checked.structuredContent?.status === "ok" && !("contractDecls" in (checked.structuredContent ?? {})),
  );
  const unknown = await client.callTool({ name: "compute", arguments: { source: "1", words: ["MEEN"] } });
  check(
    "words refuses a name the phrasebook does not hold, and suggests the nearest",
    unknown.structuredContent?.status === "hostError" &&
      unknown.structuredContent?.error?.code === "invalidRequest" &&
      /MEEN \(did you mean MEAN/.test(unknown.structuredContent?.error?.message ?? ""),
  );
}

const compute = await client.callTool({
  name: "compute",
  arguments: { source: "[ 2 ] SQRT" },
});
const oversized = await client.callTool({
  name: "compute",
  arguments: { source: " ".repeat(LIMITS.sourceBytes + 1) },
});
check("compute rejects source beyond its UTF-8 byte limit", oversized.isError === true);
// Host failures are structured, so a caller branches on a code rather than on
// an English sentence — this file used to match on "CLI not found" itself.
check(
  "a host failure is machine-readable and schema-valid",
  oversized.structuredContent?.status === "hostError" &&
    oversized.structuredContent?.error?.code === "sourceTooLarge" &&
    oversized.structuredContent?.error?.retryable === false &&
    validateResult(oversized.structuredContent),
);
check(
  "a host failure message carries no host paths or environment names",
  !/AJISAI_(BIN|REPO)|\/(home|usr|tmp)\//.test(oversized.structuredContent?.error?.message ?? ""),
);
const badRequest = await client.callTool({ name: "compute", arguments: { source: 42 } });
check(
  "an invalid request is a host error, not a language error",
  badRequest.structuredContent?.error?.code === "invalidRequest",
);
// `additionalProperties: false` is a promise the server itself keeps: an
// argument no tool declares is refused by name, never read and never ignored.
const undeclared = await client.callTool({
  name: "compute",
  arguments: { source: "1 2 ADD", limits: "trusted" },
});
check(
  "an argument no tool declares is refused as an invalid request",
  undeclared.isError === true &&
    undeclared.structuredContent?.error?.code === "invalidRequest" &&
    undeclared.structuredContent?.error?.message.includes("`limits`"),
);
// A `#:contract` declaration is checked before anything runs, by `check` and
// by `compute` alike, and a violated one is an error in the shape every other
// error has — not a bare `status: error` with the finding buried in
// `contractDecls.findings`.
const declared = "#:contract DOUBLE inputs=2 outputs=1\n[ 2 MUL ] 'DOUBLE' DEF 5 DOUBLE";
for (const tool of ["check", "compute"]) {
  const violated = await client.callTool({ name: tool, arguments: { source: declared } });
  const result = violated.structuredContent;
  check(
    `${tool} refuses a violated #:contract declaration in the common error shape`,
    violated.isError !== true &&
      result?.status === "error" &&
      result?.message?.startsWith("Contract declaration violated:") &&
      result?.error?.category === "contractViolation" &&
      result?.error?.word === "DOUBLE" &&
      result?.contractDecls?.outcome === "error" &&
      result?.contractDecls?.findings?.[0]?.severity === "error" &&
      (tool === "check" ? !("outcome" in result) : result?.outcome === "error:contractViolation") &&
      (tool === "check" || (result?.stackDisplay?.length === 0 && result?.output?.length === 0)) &&
      validateResult(result),
  );
}
const verifiedDeclaration = await client.callTool({
  name: "compute",
  arguments: { source: "#:contract DOUBLE inputs=1 outputs=1 field=closed\n[ 2 MUL ] 'DOUBLE' DEF 21 DOUBLE" },
});
check(
  "compute runs a program whose declaration verifies, and says the check happened",
  verifiedDeclaration.structuredContent?.status === "ok" &&
    verifiedDeclaration.structuredContent?.stackDisplay?.[0] === "42/1" &&
    verifiedDeclaration.structuredContent?.contractDecls?.outcome === "value" &&
    verifiedDeclaration.structuredContent?.contractDecls?.declarations?.[0]?.word === "DOUBLE" &&
    validateResult(verifiedDeclaration.structuredContent),
);
// A directive accepts `inputs`, `outputs` and `field` and nothing else; any
// other term is a malformed declaration, refused rather than ignored.
const malformedDeclaration = await client.callTool({
  name: "check",
  arguments: { source: "#:contract DOUBLE inputs=1 purity=pure\n[ 2 MUL ] 'DOUBLE' DEF" },
});
check(
  "a #:contract term outside inputs/outputs/field is a contract violation",
  malformedDeclaration.structuredContent?.status === "error" &&
    malformedDeclaration.structuredContent?.error?.category === "contractViolation" &&
    malformedDeclaration.structuredContent?.contractDecls?.findings?.[0]?.message?.includes("purity=pure") &&
    validateResult(malformedDeclaration.structuredContent),
);
// Empty source and whitespace-only source are the same empty program; one used
// to be a host error and the other a value.
for (const source of ["", "   "]) {
  const empty = await client.callTool({ name: "compute", arguments: { source } });
  check(
    `an empty program (${JSON.stringify(source)}) is a value, not a host error`,
    empty.structuredContent?.status === "ok" && empty.structuredContent?.outcome === "value",
  );
}
check(
  "every declared host-error code is retryable or not, explicitly",
  Object.values(HOST_ERRORS).every(({ retryable }) => typeof retryable === "boolean") &&
    HOST_ERRORS.capacityExhausted.retryable === true &&
    HOST_ERRORS.timeout.retryable === true &&
    CAPACITY_WAIT_MS > 0,
);
if (compute.structuredContent?.error?.code === "backendUnavailable") {
  check("compute requires a real Ajisai backend", false);
} else {
  const sqrt = compute.structuredContent?.stack?.[0]?.value?.[0];
  const [exactTerm] = sqrt?.semantics?.exactTerms ?? [];
  check(
    "compute preserves the exact algebraic normal form",
    exactTerm?.numerator === "1" &&
      exactTerm?.denominator === "1" &&
      exactTerm?.radicand === "2",
  );
  // `stackDisplay` writes those same terms; only `value` (a rational
  // approximation) is not the number.
  check(
    "compute renders the algebraic value from the terms it carries",
    compute.structuredContent?.stackDisplay?.[0] === "[ sqrt(2) ]",
  );
  // The ceilings travel once, in `ajisai://limits`; a result names the
  // profile and carries none of the thirteen numbers.
  check(
    "compute reports engine provenance and names the applied profile",
    compute.structuredContent?.mcp?.serverVersion === serverVersion() &&
      compute.structuredContent?.mcp?.engineVersion === rootPackageVersion &&
      compute.structuredContent?.specificationVersion === lockedSpecificationVersion &&
      compute.structuredContent?.mcp?.limitProfile === LIMIT_PROFILE &&
      compute.structuredContent?.mcp?.limitProfile === limitsResource.profile &&
      !("limits" in (compute.structuredContent?.mcp ?? {})),
  );
  check(
    "compute names which backend answered",
    ["nativeCli", "wasmWorker"].includes(compute.structuredContent?.mcp?.backend?.kind),
  );
  const second = await client.callTool({ name: "compute", arguments: { source: "[ 2 ] SQRT" } });
  check(
    "the backend is fixed for the life of the server",
    second.structuredContent?.mcp?.backend?.kind ===
      compute.structuredContent?.mcp?.backend?.kind,
  );
  check("compute satisfies the published result schema", validateResult(compute.structuredContent));

  const languageError = await client.callTool({
    name: "compute",
    arguments: { source: "FROBNICATE" },
  });
  check(
    "Ajisai language errors remain structured non-MCP errors",
    languageError.isError !== true && languageError.structuredContent?.status === "error",
  );
  const misspelled = await client.callTool({
    name: "compute",
    arguments: { source: "[ 1 2 3 ] LENGHT" },
  });
  check(
    "an unknown Word error names the Word and where it was written",
    misspelled.structuredContent?.error?.category === "unknownWord" &&
      misspelled.structuredContent?.error?.word === "LENGHT" &&
      misspelled.structuredContent?.error?.line === 1 &&
      misspelled.structuredContent?.error?.column === 11 &&
      validateResult(misspelled.structuredContent),
  );

  // A ceiling an agent plans against has to report the number it is judged by.
  // `runtimeMetrics.executionSteps` was read from a struct field nothing ever
  // wrote, so every program that ever ran reported 0 steps — beside the counter
  // every limit check increments.
  const spent = await client.callTool({
    name: "compute",
    arguments: { source: "1 20 RANGE 1 [ MUL ] FOLD" },
  });
  const usage = spent.structuredContent?.resourceUsage;
  check(
    "a run reports the resources it actually spent",
    usage?.executionSteps === 22 &&
      usage?.numericWork === 20 &&
      // 20 elements materialized by `RANGE`, at 17 units each: the collection
      // meter sees the vector the fold walks, which no ceiling counted before.
      usage?.collectionWork === 340 &&
      validateResult(spent.structuredContent),
  );
  check(
    "every reported resource names a declared limit",
    Object.keys(usage ?? {}).every((key) => key in LIMITS),
  );
  // The optimizer counters are the engine's own business; an agent reads
  // budgets, and `resourceUsage` is the one place they are reported.
  check(
    "a budget is reported once, under resourceUsage, and the optimizer counters not at all",
    !("runtimeMetrics" in (spent.structuredContent ?? {})),
  );
  // What the constant blocks cost, measured: `1 2 ADD` was 3,732 bytes as
  // sent, of which the answer was 85; the constants left are the provenance.
  // The ceiling is a ceiling, not a target — lower it when a response
  // genuinely shrinks; `eval/performance.json` holds the exact median.
  const small = await client.callTool({ name: "compute", arguments: { source: "1 2 ADD" } });
  const smallBytes = Buffer.byteLength(JSON.stringify(small), "utf8");
  check(
    `a small result is small as sent (${smallBytes} bytes for \`1 2 ADD\`)`,
    small.structuredContent?.stackDisplay?.[0] === "3/1" && smallBytes <= 2048,
  );
  // One envelope, one version: the adapter's own answers (`word_contract`)
  // carry the backend report's schema version, not a number of their own.
  check(
    "every envelope carries the one schema version",
    Number.isInteger(spent.structuredContent?.schemaVersion) &&
      addContract.structuredContent?.schemaVersion === spent.structuredContent.schemaVersion,
  );

  // An error report's answer is its error; the stack is residual state.
  // `0 99999 RANGE LENGHT` is a one-character typo holding a
  // 100,000-element vector, which serialized in full is ~27 MB — so before the
  // stack was elided the whole result became `responseTooLarge` and the reader
  // was told its answer was too big rather than that it had misspelled
  // `LENGTH`. The residue is what gives way, never the reason.
  const hugeResidue = await client.callTool({
    name: "compute",
    arguments: { source: "0 99999 RANGE LENGHT" },
  });
  const hugeResidueBytes = Buffer.byteLength(
    JSON.stringify(hugeResidue.structuredContent ?? {}),
    "utf8",
  );
  check(
    "the error survives a failing stack too large to send",
    hugeResidue.isError !== true &&
      hugeResidue.structuredContent?.status === "error" &&
      hugeResidue.structuredContent?.error?.word === "LENGHT" &&
      hugeResidueBytes < LIMITS.responseBytes &&
      validateResult(hugeResidue.structuredContent),
  );
  check(
    "an elided slot keeps its position and says what it dropped",
    hugeResidue.structuredContent?.stackElided?.reason === "errorStackBudget" &&
      hugeResidue.structuredContent?.stackElided?.slots?.[0]?.index === 0 &&
      hugeResidue.structuredContent?.stackElided?.slots?.[0]?.elements === 100000 &&
      hugeResidue.structuredContent?.stack?.[0]?.type === "vector" &&
      hugeResidue.structuredContent?.stack?.[0]?.value === null &&
      hugeResidue.structuredContent?.stack?.length ===
        hugeResidue.structuredContent?.stackDisplay?.length,
  );
  check(
    "an ordinary error is not elided at all",
    misspelled.structuredContent?.stackElided === undefined &&
      !("elided" in (misspelled.structuredContent?.stack?.[0] ?? {})),
  );

  const checked = await client.callTool({
    name: "check",
    arguments: { source: "[ [ 1 ] ADD ] 'INC' DEF" },
  });
  check("check is execution-free and structured", checked.structuredContent?.status === "ok");

  const inferred = await client.callTool({
    name: "infer_contracts",
    arguments: { source: "[ [ 1 ] ADD ] 'INC' DEF" },
  });
  check(
    "infer_contracts returns the user Word contract",
    JSON.stringify(inferred.structuredContent?.contracts) === JSON.stringify([{
      field: "closed",
      fieldExits: [],
      inputs: 1,
      name: "INC",
      outputs: 1,
      suggested: "#:contract INC inputs=1 outputs=1 field=closed",
    }]),
  );
  check(
    "infer_contracts satisfies the published result schema",
    validateResult(inferred.structuredContent),
  );
}

// The terminal command line. A stdio server is silent whether it is healthy or
// wedged, so `--doctor` is the only thing that tells an operator which; and it
// has to answer through an exit code, because that is what a script reads.
async function cli(...argv) {
  const lines = [];
  const code = await runCli(argv, (line) => lines.push(line));
  return { code, text: lines.join("\n") };
}
const version = await cli("--version");
check(
  "--version names the adapter, the engine and the registry",
  version.code === 0 &&
    version.text.includes(`ajisai-mcp-server ${serverVersion()}`) &&
    /engine \d/.test(version.text) &&
    /registry [a-f0-9]{16}/.test(version.text),
);
const help = await cli("--help");
check(
  "--help documents every command the binary accepts",
  help.code === 0 && ["--doctor", "--version", "--help"].every((flag) => help.text.includes(flag)),
);
const doctorRun = await cli("--doctor");
check(
  "--doctor exercises the real backend and exits 0 when healthy",
  doctorRun.code === 0 &&
    doctorRun.text.includes("checks passed") &&
    !doctorRun.text.includes("FAIL"),
);
check(
  "--doctor proves exactness rather than only reporting that it started",
  doctorRun.text.includes("2 3 DIV 1 3 DIV ADD = 1/1") && doctorRun.text.includes("exactTerms"),
);
const unknownFlag = await cli("--frobnicate");
check(
  "an unrecognized option is named and exits non-zero rather than serving",
  unknownFlag.code === 2 && unknownFlag.text.includes("--frobnicate"),
);

await close();
if (failures) process.exit(1);
console.log("all checks passed");
