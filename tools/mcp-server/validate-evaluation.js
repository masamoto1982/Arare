#!/usr/bin/env node
import { readEval, referenceTraces } from "./eval-common.js";
import { LANGUAGES, indexTraces, validateCorpus } from "./evaluation-contract.js";

const corpus = readEval("./eval/cases.json");
const repairCorpus = readEval("./eval/repair-cases.json");

/**
 * No prompt may show the retired block syntax.
 *
 * The `static-check` and `contract-inference` prompts asked, in both locales,
 * about `{ [ 1 ] ADD } 'INC' DEF` — the retired block form — while their own
 * `arguments.source` carried the correct `[ [ 1 ] ADD ] 'INC' DEF`. The fixtures
 * therefore passed: the corpus scores the tool call, and nothing read the
 * prompt. But this corpus is the text a model is shown, and what a model is
 * shown is what it imitates. A corpus for an AI-first language is the last
 * place a retired form should survive.
 *
 * This list used to be read out of the packaged manifest's `retired_form`
 * entries, so that retiring another form extended the gate for free. There are
 * no such entries any more: `{`, `}` and a bare `|` were freed into ordinary
 * name characters along with `(` and `)`, and the manifest carries only live
 * forms. The gate's reason is untouched by that — `{ [ 1 ] ADD } 'INC' DEF`
 * still does not define anything, because `{` resolves to no Word — so what
 * changed is only which layer refuses it, the dictionary rather than the lexer.
 * The pair is named here because it is now a closed historical fact rather than
 * a registry that can grow: code blocks and vectors were unified onto `[` and
 * `]`, and nothing is queued to be retired behind them.
 *
 * Two deliberate narrowings, each of which a first draft of this gate got
 * wrong:
 *
 * - The block delimiters only. `(` and `)` are ordinary punctuation in both
 *   prompt languages, so forbidding them rejects correct prose — and they are
 *   ordinary name characters in Ajisai now besides.
 * - Only cases that expect an Ajisai tool call. A negative case exists to check
 *   that a question about *another* language does not reach an Ajisai tool, so
 *   `irrelevant-debug` shows a C-style `for` loop on purpose. Code that is not
 *   claiming to be Ajisai is not a lexical claim about Ajisai.
 *
 * Scope is the characters and not the whole fragment for the same reason:
 * prompts legitimately carry English prose like "the vector [1, 2]", which is
 * mathematical notation, so a tokenize-everything rule would reject them.
 */
const RETIRED_BLOCK_DELIMITERS = Object.freeze(["{", "}"]);

function assertPromptsAreLexicallyValid(cases, label) {
  const retired = RETIRED_BLOCK_DELIMITERS;
  if (retired.length === 0) {
    throw new Error(
      "no retired forms to check for: the prompt lexis gate would pass vacuously",
    );
  }
  const findings = [];
  for (const testCase of cases) {
    // A repair case names the tool through its own fixture shape; a selection
    // case that expects no tool call is a negative case, exempt per above.
    if (label === "cases" && testCase.expectedTool === null) continue;
    for (const [language, prompt] of Object.entries(testCase.prompts ?? {})) {
      for (const surface of retired) {
        if (prompt.includes(surface)) {
          findings.push(
            `${label}/${testCase.id}/${language}: prompt shows '${surface}', a retired ` +
              "block delimiter that resolves to no Word — a model shown it will imitate it",
          );
        }
      }
    }
  }
  if (findings.length > 0) {
    throw new Error(
      `evaluation prompts must not show retired Ajisai source:\n  ${findings.join("\n  ")}`,
    );
  }
}

assertPromptsAreLexicallyValid(corpus.cases, "cases");
assertPromptsAreLexicallyValid(repairCorpus.cases, "repair-cases");
const traces = indexTraces(referenceTraces(corpus), validateCorpus(corpus));
const repairTraces = indexTraces(
  readEval("./eval/reference-repair-traces.json"),
  validateCorpus(repairCorpus, { repair: true }),
  { repair: true },
);
// Once per case *per language*: a fixture missing one half of a pair scores as
// a language gap rather than as the incomplete fixture it is, which is the one
// way this document could report a finding it never measured.
const expectedSelection = corpus.cases.length * LANGUAGES.length;
const expectedRepair = repairCorpus.cases.length * LANGUAGES.length;
if (traces.size !== expectedSelection || repairTraces.size !== expectedRepair) {
  throw new Error(
    "committed reference traces must cover every evaluation case once per language: " +
      `expected ${expectedSelection} selection and ${expectedRepair} repair, ` +
      `got ${traces.size} and ${repairTraces.size}`,
  );
}
console.log(
  `evaluation prompt lexis valid (no retired source characters shown to a model)`,
);
console.log(
  `evaluation contracts valid (${corpus.cases.length} selection, ${repairCorpus.cases.length} repair, ` +
    `asked in ${LANGUAGES.join("/")} = ${expectedSelection + expectedRepair} prompts)`,
);
