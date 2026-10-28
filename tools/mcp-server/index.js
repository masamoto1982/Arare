#!/usr/bin/env node
// A deliberately narrow, source-only MCP boundary around the Ajisai CLI.

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListResourcesRequestSchema,
  ListResourceTemplatesRequestSchema,
  ListToolsRequestSchema,
  ReadResourceRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, realpathSync, statSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { NativeCliBackend } from "./backend/native-cli.js";
import { WasmWorkerBackend } from "./backend/wasm-worker.js";
import { HostError, logHostError } from "./host-error.js";
import { phrasebook, preludeFor, withoutPrelude } from "./phrasebook.js";
import { suggestWords } from "./word-candidates.js";

const here = dirname(fileURLToPath(import.meta.url));
const assetsPath = join(here, "assets");
const manifestPath = join(assetsPath, "word-manifest.json");
const contractsPath = join(assetsPath, "words.json");
const skillPath = join(assetsPath, "quickstart.md");
const metadataPath = join(assetsPath, "metadata.json");
const serverPackagePath = join(here, "package.json");
const resultSchemaPath = join(here, "result.schema.json");
// `AJISAI_PHRASEBOOK` names another file in the phrasebook's format to serve
// for `words` and `ajisai://phrasebook` — an experiment's evolving lexicon, a
// team's own phrases. Every Word in it still needs its `#:digest` line.
const phrasebookPath = process.env.AJISAI_PHRASEBOOK
  ? resolve(process.env.AJISAI_PHRASEBOOK)
  : join(assetsPath, "phrasebook.ajisai");
// The one envelope every Ajisai MCP tool result is wrapped in
// (`result.schema.json`) carries one version: the backend's report
// (`rust/src/agent/report.rs::SCHEMA_VERSION`), which the adapter's own
// envelopes — a host error, a `word_contract` answer — repeat rather than
// numbering separately. The selftest compares a `word_contract` envelope
// against a backend report, not this constant, so a bump in report.rs is
// mirrored here by hand.
const ENVELOPE_SCHEMA_VERSION = 5;

export const LIMITS = Object.freeze({
  sourceBytes: 64 * 1024,
  wallTimeMs: 5_000,
  responseBytes: 1024 * 1024,
  executionSteps: 100_000,
  concurrentExecutions: 4,
  materializedElements: 100_000,
  numericLiteralDigits: 4_096,
  numericWork: 10_000_000,
  collectionWork: 20_000_000,
  bigintBits: 262_144,
  algebraicTerms: 512,
  nestingDepth: 256,
  callDepth: 256,
});
/**
 * How long a saturated server waits for an execution slot before answering
 * `capacityExhausted`.
 *
 * A ceiling that answers "full, try again" the instant it is reached turns
 * every burst into a retry loop the caller has to write. A short queue absorbs
 * the burst instead: the ceiling on *concurrent* work is unchanged, and only
 * a caller that would have waited longer than this ever sees the error.
 */
export const CAPACITY_WAIT_MS = 1_000;

/**
 * The Ajisai checkout this server may discover a native binary in, or `null`.
 *
 * Discovery used to look at `../../rust/target` from wherever the package sat.
 * From a checkout that is this repository; from an installed package it is two
 * directories above `node_modules/ajisai-mcp-server` — somewhere in the user's
 * own project, where a binary named `ajisai` is nothing this package built.
 * So the parent counts only when this package is the checkout's
 * `tools/mcp-server` (not a copy under `node_modules`) and the parent is an
 * Ajisai source tree. `AJISAI_REPO` names a checkout explicitly.
 */
function checkoutRoot() {
  if (process.env.AJISAI_REPO) return resolve(process.env.AJISAI_REPO);
  let self;
  try {
    self = realpathSync(here);
  } catch {
    return null;
  }
  if (self.split(sep).includes("node_modules")) return null;
  const candidate = resolve(self, "..", "..");
  return existsSync(join(candidate, "rust", "Cargo.toml")) ? candidate : null;
}

/**
 * A native binary for this server to run, or `null` for the packaged WASM
 * backend: `AJISAI_BIN` when set, otherwise the most recently built of the
 * checkout's release and debug binaries. A fixed order let whichever profile
 * came first shadow a fresher build of the other.
 */
export function resolveAjisaiBin() {
  if (process.env.AJISAI_BIN) return process.env.AJISAI_BIN;
  const root = checkoutRoot();
  if (!root) return null;
  const built = ["release", "debug"]
    .map((profile) => join(root, "rust", "target", profile, "ajisai"))
    .filter((candidate) => existsSync(candidate))
    .map((candidate) => ({ candidate, mtimeMs: statSync(candidate).mtimeMs }))
    .sort((a, b) => b.mtimeMs - a.mtimeMs);
  return built[0]?.candidate ?? null;
}

// Backend selection: the packaged WASM worker needs neither `AJISAI_REPO` nor
// `AJISAI_BIN` and is the self-contained default. A native binary — found via
// the explicit `AJISAI_BIN` override, or discovered under a checked-out
// `rust/` (local development, Docker images that build it in) — takes
// precedence when present. Both backends return the identical result
// envelope (`docs/dev/agent-cli-output-contract.md`), verified case-by-case in
// `backend/parity-test.js`, so the two agree on every result.
//
// Resolved **once**, at server construction, and reported in `mcp.backend`.
// Re-resolving per request meant a `cargo build` finishing mid-session
// silently moved later calls from the WASM backend to the native one: a
// deterministic kernel whose execution path could change under it between two
// identical calls, with nothing in the response saying so. Parity keeps the
// two answers equal; provenance is what makes an unequal one diagnosable.
// `overrides` exists for one caller: the selftest's `wallTimeMs` case. That
// ceiling is a deadline the adapter holds around execution, not a budget the
// engine spends, so the only honest way to exercise it is to build a backend
// with a deadline short enough that a real program misses it — the server's own
// admission path, with one number moved. Every other caller passes nothing and
// gets the declared profile.
export function createBackend(overrides = {}) {
  const profile = { ...LIMITS, ...overrides };
  const bin = resolveAjisaiBin();
  if (bin) {
    return new NativeCliBackend({
      bin,
      wallTimeMs: profile.wallTimeMs,
      responseBytes: profile.responseBytes,
      executionSteps: profile.executionSteps,
    });
  }
  if (WasmWorkerBackend.isAvailable()) {
    return new WasmWorkerBackend({
      wallTimeMs: profile.wallTimeMs,
      executionSteps: profile.executionSteps,
      responseBytes: profile.responseBytes,
    });
  }
  return null;
}

let selectedBackend;
let backendResolved = false;
function backend() {
  if (!backendResolved) {
    selectedBackend = createBackend();
    backendResolved = true;
  }
  return selectedBackend;
}

const sourceSchema = {
  type: "object",
  additionalProperties: false,
  properties: {
    source: {
      type: "string",
      // JSON Schema counts characters; the server counts UTF-8 bytes, and one
      // character is never fewer than one byte — so this is the coarse
      // character bound the byte budget implies, and never rejects source the
      // byte check would have accepted. Source that passes here can still be
      // rejected as `sourceTooLarge`, which is why the effective unit is
      // stated in the description rather than left to be inferred.
      maxLength: LIMITS.sourceBytes,
      // The four rules below are not a syntax summary — they are the four
      // mistakes a real model actually made, counted. After the entry-surface
      // round moved tool selection from 0.469 to 0.862, 31 of 130 prompts
      // reached `compute` and handed it source that did not run, and eight of
      // those were the quote character alone. Each rule was executed against
      // the engine before being written here, which corrected two guesses:
      // `[1 2 3]` without inner spaces is fine, and so is `[ 1, 2, 3 ]`.
      // Stating rules that are not real would cost the caller the same turn the
      // missing ones do.
      description:
        `Ajisai source text (file paths are not accepted). The effective limit is ${LIMITS.sourceBytes} UTF-8 bytes, so non-ASCII text reaches it at fewer characters than maxLength suggests. ` +
        "Syntax is postfix: operands first, then the Word — `1 2 ADD`, `[ 1 2 3 ] LENGTH`. " +
        "A string is single-quoted (`'hi'`, never \"hi\"). " +
        "A block passed to MAP/FILTER/FOLD/SCAN is a Vector like any other, written with `[ ]` — there is no separate block bracket (`[ 1 2 3 4 ] [ 2 GT ] FILTER`). " +
        "A Word's operand shape is part of its contract and is worth checking with word_contract when unsure — several take a vector where one number looks natural, e.g. `0 4 RANGE` and `[ [ 1 2 ] [ 3 4 ] ] ZIP`.",
    },
    words: {
      type: "array",
      items: { type: "string", minLength: 1 },
      uniqueItems: true,
      maxItems: 32,
      description:
        "Optional names of phrasebook User Words to define before `source` runs — phrases programs write again and again, such as a remainder, " +
        "a mean or a disjunction; ajisai://phrasebook lists them with their contracts and examples. " +
        "Each comes with the phrasebook Words it calls and its verified `#:contract`. The result's `prelude` names every Word added with its content " +
        "digest, error lines count from the first line of `source`, and the definitions count against the source byte limit.",
    },
  },
  required: ["source"],
};
// Embedded once per tool, so without its `$id`: four copies of one schema
// under one `$id` make a client that caches validators by `$id` (Ajv, with a
// single instance) refuse the second as a duplicate. The resource
// `ajisai://schema/result` still serves the file as written, `$id` included.
const { $id: _resultSchemaId, ...envelopeSchema } = JSON.parse(readFileSync(resultSchemaPath, "utf8"));
// Counted, not written: a hand-typed "78" is a claim the registry can
// outgrow, and `tool-description.test.js` pins that every Word counted here is
// also named in the description.
const REGISTRY_WORD_COUNT = JSON.parse(readFileSync(contractsPath, "utf8")).entries.length;
const READ_ONLY_ANNOTATIONS = Object.freeze({
  readOnlyHint: true,
  destructiveHint: false,
  idempotentHint: true,
  openWorldHint: false,
});
export const TOOLS = [
  {
    name: "compute",
    // Every word of this is load-bearing, and the length is deliberate: with
    // `tool_choice: auto` these tool descriptions are the *only* text a caller
    // reads before deciding. The measured baseline (`eval/traces/`) says what
    // the previous one cost. It named the numeric domain and stopped there, so
    // of 118 prompts, 21 produced no call at all — twelve of them collection
    // work, which the description never mentioned — and another 46 were spent
    // guessing Word names in the registry (`vec-add`, `group-by`, `nil-or`,
    // `dict`: none exist) because nothing readable before the first call said
    // what the Words are called. Naming the families and the Words in them is
    // the cheapest thing that answers both.
    // A third failure mode showed up only in Japanese. Live evaluation
    // against claude-opus-5 (eval/traces/claude-opus-5-full-corpus.json)
    // found three negative cases — irrelevant-currency, irrelevant-estimate,
    // irrelevant-debug, none of them Ajisai's domain — that passed restraint
    // in English but failed in Japanese: the model invented a plausible
    // real-world number (an exchange rate, characters-per-page, a step count
    // for someone else's for-loop) and ran it through this tool, so the
    // guess came back dressed as an exact result. The description at the
    // time was identical in both languages, and irrelevantToolRate was 0.10
    // en vs. 0.25 ja on that same text, so the gap was the model's response
    // to one weak sentence differing by language, not missing Japanese
    // wording. The closing sentence below names the failure directly instead
    // of leaving "out of domain" to be inferred.
    description:
      "Execute a bounded Ajisai program and return its stack. Ajisai is postfix (RPN) and its numbers are exact rationals closed under square root — no floats, so results are reproducible and comparisons decide. " +
      `Its ${REGISTRY_WORD_COUNT} Words cover arithmetic (ADD SUB MUL DIV FLOOR ROUND MIN MAX SQRT POW GCD RATIO — negate with -1 MUL), comparison (EQ LT GT — not-equal is EQ NOT, at-most is GT NOT), three-valued logic (TRUE FALSE AND NOT, a NIL read as unknown — or is a NOT b NOT AND NOT; RECONCILE answers what two sources agree on, a NIL(conflict) for two different values), ` +
      "vectors — arithmetic broadcasts element-wise — collections (SORT ORDER UNIQUE ZIP RANGE FILL TAKE DROP CONCAT REVERSE LENGTH GET PUT BSEARCH COLLECT SHAPE RESHAPE FLATTEN DEPTH), " +
      "records — keyed data, built by RECORD from a key vector and a value vector, read by GET KEYS VALUES HAS?, rewritten by PUT WITHOUT MERGE; TALLY and GROUP answer Records — " +
      "higher-order blocks (MAP FILTER FOLD SCAN), text (CHARS JOIN TOKENIZE TRIM UPPER LOWER SEARCH REPLACE NUM STR FORMAT), JSON (JSON-DECODE JSON-ENCODE — objects are Records, numbers exact, nothing rounded), absence (NIL NIL? NIL-REASON ABSENT), control (SELECT EXEC FAIL), reflection (DIGEST CONTRACT — a Word's or a block's contract as a Record, inferred without running it), naming (DEF to name your own Word, BIND DEL) and output (PRINT). " +
      "That is the whole vocabulary. Word names are case-insensitive (`add` runs as ADD) but otherwise exact — a name not listed here does not exist under another spelling; word_contract answers a near-miss with suggestions, so look a name up rather than guessing it. " +
      "Each call runs one source in a fresh session with no User Words: a DEF lasts for that source only, so a program that needs a Word it defines carries the DEF in the same source. " +
      "The result's `outcome` names which of the language's three results the run produced — `value`, `nil:<reason>` (a reasoned absence; the call still succeeds) or `error:<category>`; an error also carries `error` — its category, the ceiling it hit (`resource`), the Word it is attributed to and its source line and column. " +
      "Reach for this whenever the request is one of those operations and the answer should be exact and checkable rather than recalled. Out of domain: floats, file or network I/O, and general-purpose programming. " +
      "Ajisai also has no external or real-world reference data of its own — no exchange rates, no calendars, no reading speeds, no other language's syntax semantics. Do not invent a plausible-looking number for one of those and run it through this tool to dress a guess up as an exact answer; if the question needs a real-world fact rather than a value already given or derivable from first principles inside this domain, answer directly without a call, or say you don't know.",
    inputSchema: sourceSchema,
    outputSchema: envelopeSchema,
    annotations: READ_ONLY_ANNOTATIONS,
  },
  {
    name: "check",
    description: "Parse and resolve Ajisai source without executing it; also verifies declared contracts conservatively, and refuses a block written right before MAP, FILTER, FOLD or SCAN that cannot leave exactly one value, as running it would.",
    inputSchema: sourceSchema,
    outputSchema: envelopeSchema,
    annotations: READ_ONLY_ANNOTATIONS,
  },
  {
    name: "infer_contracts",
    description: "Infer each user-defined Word's stack contract without executing its body: `inputs` and `outputs` (a count, or `variable`), `field` (`closed`, or `leaving` with the `fieldExits` that leave it) and a paste-ready `#:contract` line in `suggested`. Source that does not parse is the same `malformedSource` error check reports.",
    inputSchema: sourceSchema,
    outputSchema: envelopeSchema,
    annotations: READ_ONLY_ANNOTATIONS,
  },
  {
    name: "word_contract",
    // The baseline's second failure mode was 46 turns spent here, two guesses
    // at a time, at names no Ajisai vocabulary ever had. Answering a miss with
    // `suggestions` was already right; what was missing was any hint that the
    // whole list is one resource read away, so a caller that does not know the
    // name has something better to do than guess again.
    description:
      "Return the generated canonical registry entry for a Word — its arity, purity, NIL policy, contract, " +
      "and `cost`: what the Word charges on each metered resource (`steps`/`numeric`/`collection`), as a growth class " +
      "in its input. Cost classes join pointwise under concatenation, so a phrase's bound is the widest bound among " +
      "its Words — read them here to budget a program before running it rather than discovering the ceiling by hitting it. " +
      "A class is how the charge *grows*, never how large it is: two programs of the same class can differ by orders of " +
      "magnitude. Measured, `[ 1 2 3 4 5 ] [ 0 ] [ ADD ] FOLD` spends 5 `numericWork` while `2 SQRT 3 SQRT ADD` — also `const` — " +
      "spends 2048, because an algebraic value carries a multiquadratic normal form and every operation on one rebuilds it. " +
      "An axis whose charge also grows with an irrational's term count says so with `valueDependent: \"algebraicTerms\"` " +
      "(measured, never guessed: `MUL` on two 16-term values charges 262,144 where two rationals charge 1); for those, run " +
      "a small case and read `resourceUsage.algebraicTerms`, the largest term count a result reached, rather than inferring " +
      "the size from the class. " +
      "An unmatched name answers with the closest known Words in `suggestions`. " +
      "To read every Word's full contract at once instead of probing one name at a time, read the ajisai://contracts " +
      "resource; ajisai://vocabulary lists the inventory — every name, its family and its tier.",
    inputSchema: {
      type: "object",
      additionalProperties: false,
      properties: { word: { type: "string", minLength: 1 } },
      required: ["word"],
    },
    // The same envelope every other tool answers with. `matches` used to be
    // declared as an array of untyped objects on a tool whose entire purpose
    // is returning contracts, so a caller could not know the shape of the
    // answer without making the call first; the registry entry is now
    // described in `result.schema.json` beside everything else.
    outputSchema: envelopeSchema,
    annotations: READ_ONLY_ANNOTATIONS,
  },
];

let manifestCache;
let contractsCache;
let registryDigestCache;
let metadataCache;
let serverVersionCache;

/**
 * Admission control for concurrent executions, with a short waiting queue.
 *
 * `tryAcquire` keeps the non-blocking form the ceiling is defined in terms of;
 * `acquire` adds the queue, so a burst that clears within `waitMs` becomes
 * back-pressure rather than an error the caller has to retry around.
 */
export class ExecutionGate {
  #active = 0;
  #waiting = [];

  constructor(capacity) {
    this.capacity = capacity;
  }

  tryAcquire() {
    if (this.#active >= this.capacity) return false;
    this.#active += 1;
    return true;
  }

  /** Resolves `true` once a slot is held, or `false` if `waitMs` elapses. */
  acquire(waitMs) {
    if (this.tryAcquire()) return Promise.resolve(true);
    if (!(waitMs > 0)) return Promise.resolve(false);
    return new Promise((settle) => {
      const waiter = {
        settle: (acquired) => {
          clearTimeout(timer);
          settle(acquired);
        },
      };
      const timer = setTimeout(() => {
        const index = this.#waiting.indexOf(waiter);
        if (index !== -1) this.#waiting.splice(index, 1);
        settle(false);
      }, waitMs);
      this.#waiting.push(waiter);
    });
  }

  release() {
    if (this.#active === 0) throw new Error("execution gate released without a holder");
    const waiter = this.#waiting.shift();
    if (waiter) {
      // Hand the slot straight over: decrementing first would let a new
      // request overtake the caller that has been queued longest.
      waiter.settle(true);
      return;
    }
    this.#active -= 1;
  }
}
const executionGate = new ExecutionGate(LIMITS.concurrentExecutions);
function manifest() { return manifestCache ??= JSON.parse(readFileSync(manifestPath, "utf8")); }

/**
 * What `ajisai://vocabulary` serves: the inventory, and nothing an agent can
 * be misled by.
 *
 * The manifest is a repository artifact, and it carries the repository's
 * bookkeeping — classification axes the specification retired
 * (`semantic_role`, `algebraic_family`, `core_tier`), coverage ids, source
 * file paths, mixed snake_case and camelCase. Served verbatim, those read to
 * an agent as live language concepts. The projection keeps what the resource
 * is for: every name a program can write, its family, its tier and its kind.
 */
export function vocabulary() {
  const { schemaVersion, entries } = manifest();
  return {
    schemaVersion,
    wordCount: entries.filter((entry) => entry.kind === "coreword").length,
    entries: entries.map(({ surface, family, vocabularyTier, kind }) => ({
      name: surface,
      kind,
      ...(family ? { family } : {}),
      ...(vocabularyTier ? { vocabularyTier } : {}),
    })),
  };
}
function contracts() {
  return contractsCache ??= JSON.parse(readFileSync(contractsPath, "utf8"));
}
export function registryDigest() {
  const calculated = registryDigestCache ??= createHash("sha256")
    .update(readFileSync(contractsPath))
    .digest("hex");
  if (calculated !== metadata().registryDigest) {
    throw new HostError(
      "registryUnavailable",
      "The packaged Ajisai Word registry does not match its recorded digest.",
      { detail: `calculated=${calculated} expected=${metadata().registryDigest}` },
    );
  }
  return calculated;
}
function metadata() { return metadataCache ??= JSON.parse(readFileSync(metadataPath, "utf8")); }
export function engineVersion() { return metadata().engineVersion; }
/** The specification version the packaged engine implements (`spec/version-lock.json`). */
export function specificationVersion() { return metadata().specificationVersion; }
export function serverVersion() {
  return serverVersionCache ??= JSON.parse(readFileSync(serverPackagePath, "utf8")).version;
}
export function packageEngines() {
  return JSON.parse(readFileSync(serverPackagePath, "utf8")).engines ?? {};
}

/**
 * Provenance every answer carries: which adapter, which engine, which
 * registry, which of the two interchangeable backends actually ran, and the
 * limits that were applied.
 *
 * `serverVersion` and `engineVersion` are two independently versioned
 * components — this adapter and the Ajisai language it speaks for — and a
 * saved result used to name only the second. Which adapter produced a stored
 * envelope decided whether a field was absent because the engine cannot
 * answer it or because this version of the server never sent it, and that
 * question was unanswerable from the result alone.
 *
 * The backend identifier does not weaken the abstraction the two backends
 * share — parity testing is what guarantees they agree. It is what makes a
 * disagreement investigable if one ever occurs.
 */
/** The one host profile this server applies, as `ajisai://limits` names it. */
export const LIMIT_PROFILE = "mcp-local-stdio";

function provenance() {
  const selected = backend();
  return {
    serverVersion: serverVersion(),
    engineVersion: engineVersion(),
    // Named for what it digests: the packaged asset, not the engine.
    assetDigest: registryDigest(),
    backend: { kind: selected?.kind ?? null },
    // The profile by name; the thirteen ceilings themselves are one resource
    // read away (`ajisai://limits`) and never change between calls. They
    // used to travel on every result — 300 bytes of constants — and a ceiling that
    // fires names itself in `error.resource` and its value in `message`, so
    // no result needs the table to be read.
    limitProfile: LIMIT_PROFILE,
  };
}

/**
 * Drop the envelope fields whose value is `null`.
 *
 * The backend fills every slot of the envelope on every answer, so a
 * successful `compute` used to advertise `message: null`, `error: null` and
 * `contractDecls: null` — diagnostic-sounding fields inviting a reader to look
 * at nothing. Absence says the same thing in
 * no bytes, and `result.schema.json` requires only `schemaVersion` and
 * `status`, so nothing that was promised stops being there.
 *
 * Top level only, and deliberately so. Recursing would save a few more bytes
 * and would turn one stated rule into a transformation a
 * reader has to apply mentally to every nested object; the rule is worth more
 * than the bytes. The backend's own envelope is untouched — this is the
 * adapter's presentation of it, which is why the native/WASM parity test,
 * which compares what the backends return, is unaffected.
 */
function withoutEmptyFields(value) {
  return Object.fromEntries(Object.entries(value).filter(([, field]) => field !== null));
}

/**
 * One result, in the two shapes MCP asks for.
 *
 * `structuredContent` is what a caller branches on. `content` carries the
 * *same* object serialized, because the specification asks a tool with an
 * output schema to also return the serialized JSON in a text block for
 * backwards compatibility — a text-only client has no other way to reach any
 * of this. Replacing that text with a prose summary would have been the one
 * compaction that actually loses information, so what shrank instead is
 * padding: the two-space indentation this used to be written with cost about a
 * third of the text and told a machine nothing.
 *
 * Both shapes are built from one object, so the mirror cannot drift.
 */
function envelope(value, context = "tool call") {
  const result = withoutEmptyFields(value);
  const toolResult = { content: [{ type: "text", text: JSON.stringify(result) }], structuredContent: result };
  // `responseBytes` bounds what the caller receives, and what it receives is
  // this: the result twice — structured, and serialized into the text block
  // (where every quote is escaped again) — plus provenance. The backends
  // apply the same number to the one copy they produce, which is the right
  // early refusal and the wrong final one: a stack that fit it arrived as
  // more than twice the declared ceiling (`0 12000 RANGE` measured 2.29 MB
  // against 1 MiB). The ceiling is enforced here, on the response as sent.
  if (Buffer.byteLength(JSON.stringify(toolResult), "utf8") > LIMITS.responseBytes) {
    return fail(
      new HostError(
        "responseTooLarge",
        `The result exceeds the ${LIMITS.responseBytes}-byte response limit. Reduce the size of the value left on the stack.`,
      ),
      context,
    );
  }
  return toolResult;
}

/**
 * A host failure as a structured, schema-valid tool result.
 *
 * The text content stays — a client that only renders text still shows
 * something useful — but the machine-readable form is what a caller should
 * branch on. `mcp` provenance is attached when it can be: a registry failure
 * is precisely the case where it cannot.
 */
function fail(error, context = "tool call") {
  logHostError(error, context);
  const payload = {
    schemaVersion: ENVELOPE_SCHEMA_VERSION,
    status: "hostError",
    error: {
      code: error.code,
      message: error.message,
      retryable: error.retryable,
      ...(error.limit ? { limit: error.limit } : {}),
      ...(error.retryAfterMs ? { retryAfterMs: error.retryAfterMs } : {}),
    },
  };
  try {
    payload.mcp = provenance();
  } catch {
    // Provenance itself is what failed; the error block above already says so.
  }
  return {
    content: [{ type: "text", text: JSON.stringify(payload) }],
    structuredContent: payload,
    isError: true,
  };
}

async function runAgent(callerSource, command, words) {
  // An empty program is a program — whitespace alone always ran, and the
  // engine answers "" the same way — so only a missing or non-text `source` is
  // the caller's mistake.
  if (typeof callerSource !== "string") {
    return fail(new HostError("invalidRequest", "Provide `source` as text."), command);
  }
  let prelude = null;
  if (words !== undefined) {
    if (!Array.isArray(words) || words.some((word) => typeof word !== "string" || word.length === 0)) {
      return fail(new HostError("invalidRequest", "Provide `words` as an array of phrasebook Word names."), command);
    }
    const { entries } = phrasebook(phrasebookPath);
    prelude = preludeFor(words, entries);
    if (prelude.unknown) {
      const hints = prelude.unknown.map((name) => {
        const near = suggestWords(name, entries).map((s) => s.name ?? s);
        return near.length ? `${name} (did you mean ${near.slice(0, 3).join(", ")}?)` : name;
      });
      return fail(
        new HostError(
          "invalidRequest",
          `The phrasebook has no Word ${hints.join("; ")}. It holds ${entries.map((e) => e.name).join(" ")} (ajisai://phrasebook).`,
        ),
        command,
      );
    }
  }
  const source = prelude ? prelude.text + callerSource : callerSource;
  if (Buffer.byteLength(source, "utf8") > LIMITS.sourceBytes) {
    return fail(
      new HostError(
        "sourceTooLarge",
        `Source exceeds the ${LIMITS.sourceBytes}-byte limit (measured in UTF-8 bytes, not characters).`,
      ),
      command,
    );
  }
  const selected = backend();
  if (!selected) {
    return fail(
      new HostError(
        "backendUnavailable",
        "No Ajisai execution backend is available on this server.",
        { detail: "no native binary found (build rust/ or set AJISAI_BIN) and the packaged WASM module is missing" },
      ),
      command,
    );
  }
  if (!(await executionGate.acquire(CAPACITY_WAIT_MS))) {
    return fail(
      new HostError(
        "capacityExhausted",
        `All ${LIMITS.concurrentExecutions} execution slots are busy. Retry shortly.`,
        { retryAfterMs: CAPACITY_WAIT_MS },
      ),
      command,
    );
  }
  try {
    const operation = {
      run: "compute",
      check: "check",
      contract: "inferContracts",
    }[command];
    // A `compute` result already names its outcome id (`value`,
    // `nil:<reason>`, `error:<category>`): the engine classifies the run it
    // produced (`rust/src/agent/report.rs::outcome_id`), so this adapter
    // passes the field through rather than re-deriving it from the stack.
    const answer = await selected[operation](source);
    const result = prelude ? withoutPrelude(answer, prelude) : answer;
    result.mcp = provenance();
    return envelope(result, command);
  } catch (error) {
    // A backend throw is always a host failure (timeout, spawn/worker
    // failure, an oversized or non-JSON response) — never a translated Ajisai
    // `ERROR`, which each backend already returns as a normal envelope above.
    return fail(HostError.from(error), command);
  } finally {
    executionGate.release();
  }
}

function wordContract(word) {
  const needle = typeof word === "string" ? word.trim().toUpperCase() : "";
  if (!needle) {
    return fail(new HostError("invalidRequest", "Provide a `word`."), "word_contract");
  }
  const entries = contracts().entries ?? [];
  const matches = entries.filter((entry) => entry.name.toUpperCase() === needle);
  let mcp;
  try {
    mcp = provenance();
  } catch (error) {
    return fail(HostError.from(error), "word_contract");
  }
  return envelope({
    schemaVersion: ENVELOPE_SCHEMA_VERSION,
    status: "ok",
    registrySchemaVersion: contracts().schemaVersion,
    matches,
    // An empty result used to be a bare success — technically correct and
    // practically a dead end, since the caller most likely mistyped a name the
    // server holds the complete list of.
    suggestions: matches.length === 0 ? suggestWords(needle, entries) : [],
    mcp,
  });
}

const RESOURCES = [
  { uri: "ajisai://guide/quickstart", name: "Ajisai agent quickstart", mimeType: "text/markdown" },
  { uri: "ajisai://vocabulary", name: "Ajisai generated Word vocabulary", mimeType: "application/json" },
  // The whole contract registry in one read. `word_contract` answers one name
  // at a time, which is the wrong shape for the question cost exists to answer:
  // bounding a phrase means joining the bounds of every Word in it, and a
  // caller cannot do that from 100 separate probes.
  { uri: "ajisai://contracts", name: "Ajisai canonical Word contracts", mimeType: "application/json" },
  { uri: "ajisai://schema/result", name: "Ajisai MCP result contract", mimeType: "application/json" },
  { uri: "ajisai://limits", name: "Ajisai MCP host profile limits", mimeType: "application/json" },
  // The User Words a call can name in `words`: definitions, verified
  // contracts, examples and the content digest of each.
  { uri: "ajisai://phrasebook", name: "Ajisai phrasebook of User Words", mimeType: "text/plain" },
];
const RESOURCE_TEMPLATES = [
  {
    uriTemplate: "ajisai://words/{name}",
    name: "Ajisai canonical Word contract",
    description: "The complete spec/words.json contract for a Word.",
    mimeType: "application/json",
  },
];

/**
 * Validate everything the server promises to be able to answer, before it
 * accepts a single request.
 *
 * A corrupt packaged registry used to surface as a generic host error on
 * whichever request happened to touch it first. It is a startup fault: the
 * server cannot honour its own provenance claims, and the operator should
 * learn that when they start it.
 */
export function validateAssets() {
  manifest();
  contracts();
  metadata();
  phrasebook(phrasebookPath);
  serverVersion();
  registryDigest();
  readFileSync(skillPath, "utf8");
}

export function createServer() {
  validateAssets();
  // Fix the execution path for the lifetime of the process.
  backend();
  const server = new Server(
    { name: "ajisai", version: serverVersion() },
    { capabilities: { tools: {}, resources: {} } },
  );
  server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools: TOOLS }));
  server.setRequestHandler(CallToolRequestSchema, async ({ params }) => {
    const args = params.arguments ?? {};
    // Every tool's input schema says `additionalProperties: false`, and a
    // host that does not validate against it used to let an extra argument
    // through silently — a `file` or `limits` key a caller believed was
    // read, and was not. The schema is enforced here, as `invalidRequest`.
    const declared = TOOLS.find(({ name }) => name === params.name)?.inputSchema.properties;
    if (declared) {
      const undeclared = Object.keys(args).filter((key) => !(key in declared));
      if (undeclared.length) {
        return fail(
          new HostError(
            "invalidRequest",
            `Unknown argument${undeclared.length > 1 ? "s" : ""} ${undeclared.map((key) => `\`${key}\``).join(", ")}; ${params.name} takes only ${Object.keys(declared).map((key) => `\`${key}\``).join(", ")}.`,
          ),
          params.name,
        );
      }
    }
    if (params.name === "compute") return runAgent(args.source, "run", args.words);
    if (params.name === "check") return runAgent(args.source, "check", args.words);
    if (params.name === "infer_contracts") return runAgent(args.source, "contract", args.words);
    if (params.name === "word_contract") return wordContract(args.word);
    return fail(new HostError("unknownTool", `No such tool: ${params.name}`), "tools/call");
  });
  server.setRequestHandler(ListResourcesRequestSchema, async () => ({ resources: RESOURCES }));
  server.setRequestHandler(ListResourceTemplatesRequestSchema, async () => ({
    resourceTemplates: RESOURCE_TEMPLATES,
  }));
  server.setRequestHandler(ReadResourceRequestSchema, async ({ params }) => {
    const uri = params.uri;
    if (uri === "ajisai://guide/quickstart") return { contents: [{ uri, mimeType: "text/markdown", text: readFileSync(skillPath, "utf8") }] };
    if (uri === "ajisai://vocabulary") return { contents: [{ uri, mimeType: "application/json", text: JSON.stringify(vocabulary(), null, 2) }] };
    if (uri === "ajisai://contracts") return { contents: [{ uri, mimeType: "application/json", text: JSON.stringify(contracts(), null, 2) }] };
    if (uri === "ajisai://phrasebook") return { contents: [{ uri, mimeType: "text/plain", text: phrasebook(phrasebookPath).text }] };
    if (uri === "ajisai://schema/result") return { contents: [{ uri, mimeType: "application/json", text: readFileSync(resultSchemaPath, "utf8") }] };
    if (uri === "ajisai://limits") {
      return {
        contents: [{
          uri,
          mimeType: "application/json",
          text: JSON.stringify(
            {
              profile: LIMIT_PROFILE,
              serverVersion: serverVersion(),
              engineVersion: engineVersion(),
              backend: { kind: backend()?.kind ?? null },
              limits: LIMITS,
              capacityWaitMs: CAPACITY_WAIT_MS,
            },
            null,
            2,
          ),
        }],
      };
    }
    if (uri.startsWith("ajisai://words/")) {
      let name;
      try {
        name = decodeURIComponent(uri.slice("ajisai://words/".length));
      } catch {
        // `%E0` alone is not a URI component; it used to surface as a raw
        // `URIError: URI malformed`, the one resource error that named a
        // JavaScript exception rather than the request.
        throw new Error("invalidRequest: the Word name in the URI is not a valid percent-encoded UTF-8 string");
      }
      const contract = wordContract(name);
      if (contract.isError) throw new Error(contract.structuredContent.error.message);
      return {
        contents: [{
          uri,
          mimeType: "application/json",
          text: JSON.stringify(contract.structuredContent, null, 2),
        }],
      };
    }
    throw new Error(`unknown resource: ${uri}`);
  });
  return server;
}

/**
 * Whether this module was the process entry point.
 *
 * `resolve()` normalizes a path; it does not follow symlinks. Every documented
 * way of launching this package by name — `npx ajisai-mcp-server`, a client
 * spawning the installed `ajisai-mcp-server` command — runs it through
 * `node_modules/.bin/ajisai-mcp-server`, a *symlink*, so `argv[1]` was the
 * link and `import.meta.url` was its target. The comparison was false, the
 * guard did not fire, and the process exited 0 having served nothing: a client
 * saw a server that started, offered no tools and reported no error. Only
 * naming `index.js` directly ever worked, which is why the self-test and the
 * pack smoke test — both of which import `createServer` rather than launch the
 * binary — never saw it. Comparing real paths is what makes the bin entry the
 * same entry point as the file.
 */
function isEntryPoint() {
  const entry = process.argv[1];
  if (!entry) return false;
  const self = fileURLToPath(import.meta.url);
  if (resolve(entry) === self) return true;
  try {
    return realpathSync(entry) === self;
  } catch {
    return false;
  }
}

if (isEntryPoint()) {
  // Any argument means a human at a terminal, not a client opening a session:
  // `--version`, `--doctor`, `--help`, or a mistake worth naming rather than
  // ignoring. The terminal commands live in `doctor.js` so the server path
  // never loads them, and they are never reached once a transport is open —
  // the stdout MCP frames travel over stays free of diagnostics.
  //
  // Deliberately not awaited: `doctor.js` imports back from this module, so
  // awaiting the import here would suspend this module's evaluation waiting
  // for a module that is waiting for this one. Continuing lets evaluation
  // finish first, which is what resolves the cycle.
  if (process.argv.length > 2) {
    import("./doctor.js")
      .then(async ({ runCli }) => process.exit(await runCli(process.argv.slice(2))))
      .catch((error) => {
        console.error(`[ajisai-mcp] ${error.detail ?? error.message}`);
        process.exit(1);
      });
  } else {
    try {
      await createServer().connect(new StdioServerTransport());
    } catch (error) {
      // Startup faults are the operator's to fix, and a stdio server that keeps
      // running while unable to answer is worse than one that does not start.
      console.error(`[ajisai-mcp] failed to start: ${error.detail ?? error.message}`);
      process.exit(1);
    }
  }
}
