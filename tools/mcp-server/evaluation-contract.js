const TOOL_NAMES = new Set(["compute", "check", "infer_contracts", "word_contract"]);

/**
 * The languages every corpus case is asked in.
 *
 * Ajisai is written in Japanese and its tool descriptions are published in
 * English, so "does a Japanese prompt reach the same tool with the same source
 * as its English twin" is a product question rather than a translation detail.
 * Pairing is what makes it answerable: both prompts name the same task, so the
 * expected tool and the expected result are shared, and any difference in the
 * score is a difference in the model's reading of the *language* and nothing
 * else. A corpus with only one language cannot ask the question at all, which
 * is the state this replaced.
 */
export const LANGUAGES = Object.freeze(["en", "ja"]);

/** The key a trace is filed under: one case, asked once per language. */
export function traceKey(caseId, language) {
  return `${caseId}:${language}`;
}

/**
 * Every tool call a turn made, oldest first.
 *
 * A turn is one attempt, and a model that looks a Word up before computing has
 * made one attempt containing two calls. `toolCalls` is what a capture records;
 * `selectedTool`/`arguments` remain the first of them, which is all the
 * hand-written fixtures ever had. Reading through this helper is what lets the
 * two shapes be scored by the same code.
 */
export function callsOf(trace) {
  if (Array.isArray(trace?.toolCalls)) return trace.toolCalls;
  if (!trace?.selectedTool) return [];
  return [{ name: trace.selectedTool, arguments: trace.arguments ?? {} }];
}

/**
 * What produced a set of traces — the field that decides what its score means.
 *
 * `referenceFixture` is a hand-written conformance trace: scoring it proves the
 * scorer runs, and its perfect result says nothing whatever about a model.
 * `model` is a real capture. Until this field existed the two were the same
 * shape, so a fixture's `toolSelectionAccuracy: 1` could be read — or reported
 * — as a model result, which is the single claim this evaluation harness is
 * least entitled to make. The scorers now refuse to blur them.
 */
const TRACE_SOURCES = Object.freeze(["referenceFixture", "model"]);

/**
 * What a `model` trace must record to be re-runnable and comparable.
 *
 * A number without these is not a measurement: comparing PR 2 and PR 3 means
 * comparing the same corpus under the same model, prompt and tool-choice
 * setting, against the same engine. Anything missing here is a comparison that
 * cannot be made honestly later.
 */
const MODEL_PROVENANCE_FIELDS = Object.freeze([
  "modelId",
  "promptTemplateDigest",
  "toolChoice",
  "capturedAt",
  "serverVersion",
  "engineVersion",
  "registryDigest",
]);

function requireProvenance(document, label) {
  const provenance = document?.provenance;
  if (!provenance || typeof provenance !== "object" || Array.isArray(provenance)) {
    throw new Error(`${label} must carry a provenance block naming what produced it`);
  }
  if (!TRACE_SOURCES.includes(provenance.source)) {
    throw new Error(
      `${label} provenance.source must be one of ${TRACE_SOURCES.join(", ")}`,
    );
  }
  if (provenance.source !== "model") return provenance;
  const missing = MODEL_PROVENANCE_FIELDS.filter((field) =>
    typeof provenance[field] !== "string" || provenance[field].length === 0
  );
  if (missing.length) {
    throw new Error(
      `${label} is a model trace and must record ${missing.join(", ")}`,
    );
  }
  return provenance;
}

function requireDocument(document, label, collection) {
  if (document?.schemaVersion !== 1 || !Array.isArray(document?.[collection])) {
    throw new Error(`${label} must have schemaVersion 1 and a ${collection} array`);
  }
}

function requirePointers(expect, label) {
  if (!expect || typeof expect !== "object" || Array.isArray(expect)) {
    throw new Error(`${label}.expect must be an object`);
  }
  for (const pointer of Object.keys(expect)) {
    if (!pointer.startsWith("/")) throw new Error(`${label} has invalid JSON pointer: ${pointer}`);
  }
}

/**
 * Every case is asked in every language, and the two askings are not the same
 * string.
 *
 * The identical-prompt check is the one that earns its place. A pair whose
 * Japanese side is the English side copied across still validates as "both
 * languages present" and still scores twice, so the corpus would report a
 * language comparison it never made — the same class of defect as a fixture
 * scored as a model result. Requiring them to differ does not prove a
 * translation is good; it proves one was attempted.
 */
function requirePrompts(testCase) {
  const prompts = testCase.prompts;
  if (!prompts || typeof prompts !== "object" || Array.isArray(prompts)) {
    throw new Error(`${testCase.id} needs a prompts object keyed by language`);
  }
  const declared = Object.keys(prompts).sort();
  if (JSON.stringify(declared) !== JSON.stringify([...LANGUAGES].sort())) {
    throw new Error(
      `${testCase.id} must be asked in exactly ${LANGUAGES.join(", ")}; got ${declared.join(", ") || "nothing"}`,
    );
  }
  for (const language of LANGUAGES) {
    if (typeof prompts[language] !== "string" || prompts[language].trim().length === 0) {
      throw new Error(`${testCase.id} needs a non-empty ${language} prompt`);
    }
  }
  const distinct = new Set(LANGUAGES.map((language) => prompts[language].trim()));
  if (distinct.size !== LANGUAGES.length) {
    throw new Error(
      `${testCase.id} repeats the same prompt text in more than one language, so its ` +
        `per-language scores would compare nothing`,
    );
  }
}

export function validateCorpus(document, { repair = false } = {}) {
  requireDocument(document, "evaluation corpus", "cases");
  const ids = new Set();
  for (const testCase of document.cases) {
    if (typeof testCase?.id !== "string" || testCase.id.length === 0) {
      throw new Error("every evaluation case needs a non-empty id");
    }
    if (ids.has(testCase.id)) throw new Error(`duplicate evaluation case id: ${testCase.id}`);
    ids.add(testCase.id);
    requirePrompts(testCase);
    if (repair) {
      requirePointers(testCase.firstAttempt?.expect, `${testCase.id}.firstAttempt`);
      requirePointers(testCase.repairedAttempt?.expect, `${testCase.id}.repairedAttempt`);
    } else {
      if (testCase.expectedTool !== null && !TOOL_NAMES.has(testCase.expectedTool)) {
        throw new Error(`${testCase.id} has unknown expectedTool: ${testCase.expectedTool}`);
      }
      if (testCase.expectedTool === null) {
        if (testCase.expect !== null || testCase.arguments !== null) {
          throw new Error(`${testCase.id} negative case must have null arguments and expect`);
        }
      } else {
        if (!testCase.arguments || typeof testCase.arguments !== "object" ||
            Array.isArray(testCase.arguments)) {
          throw new Error(`${testCase.id} needs an arguments object`);
        }
        requirePointers(testCase.expect, testCase.id);
      }
    }
  }
  return ids;
}

function validateAttempt(attempt, label, allowNull) {
  if (allowNull && attempt?.selectedTool === null) return;
  if (!attempt || !TOOL_NAMES.has(attempt.selectedTool) ||
      !attempt.arguments || typeof attempt.arguments !== "object" || Array.isArray(attempt.arguments)) {
    throw new Error(`${label} must select a known tool with an arguments object`);
  }
}

export function indexTraces(document, caseIds, { repair = false } = {}) {
  requireDocument(document, "evaluation traces", "traces");
  requireProvenance(document, "evaluation traces");
  const traces = new Map();
  for (const trace of document.traces) {
    if (!caseIds.has(trace?.caseId)) throw new Error(`trace has unknown caseId: ${trace?.caseId}`);
    // A trace that does not say which language produced it cannot be filed
    // against the right half of a pair, and silently filing it against both
    // would report one asking as two.
    if (!LANGUAGES.includes(trace?.language)) {
      throw new Error(
        `${trace.caseId} trace must record which of ${LANGUAGES.join(", ")} it was asked in`,
      );
    }
    const key = traceKey(trace.caseId, trace.language);
    if (traces.has(key)) throw new Error(`duplicate trace for ${key}`);
    if (repair) {
      // `null` is allowed on both attempts, and has to be: a real capture can
      // record a model that never called a tool, or that gave up after reading
      // the diagnosis. Rejecting those would leave the harness able to record
      // only the cases that went well, and a repair rate computed over those is
      // not a repair rate. The scorer counts a null attempt as a failure, which
      // is what it is.
      validateAttempt(trace.firstAttempt, `${key}.firstAttempt`, true);
      validateAttempt(trace.repairedAttempt, `${key}.repairedAttempt`, true);
    } else {
      validateAttempt(trace, key, true);
    }
    traces.set(key, trace);
  }
  return traces;
}

/** The provenance of a validated trace document. */
export function traceProvenance(document) {
  return requireProvenance(document, "evaluation traces");
}

/**
 * Whether a perfect score may be *asserted* for this document.
 *
 * `--require-perfect` exists to prove the scorer end-to-end against a fixture
 * built to pass it. Pointing it at a model trace turns a measurement into a
 * pass/fail gate on the model, which converts "we have not measured this" into
 * "this is perfect" the first time a run happens to be clean. A model trace is
 * scored and reported; it is never asserted.
 */
export function mayAssertPerfect(document) {
  return traceProvenance(document).source === "referenceFixture";
}
