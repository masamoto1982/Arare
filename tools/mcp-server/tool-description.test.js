#!/usr/bin/env node
// The tool descriptions are the entry surface, so they are checked like one.
//
// With `tool_choice: auto` these four strings are the only text a caller reads
// before its first call — the quickstart and the vocabulary are resources it
// has to *decide* to fetch, and the measured baseline says it does not. That
// makes a description that omits part of the product indistinguishable, from
// the caller's side, from a product that lacks it: of 118 prompts, 21 produced
// no call at all, twelve of those asking for collection work the description
// never mentioned.
//
// So two properties are pinned here, both cheap and both checkable without a
// model:
//
//   * every Word family the registry declares is represented, so a family
//     added later cannot silently go unannounced;
//   * every Word name the descriptions mention actually exists, so the entry
//     surface never does to a caller what the caller did to us — send it after
//     a name that was never there.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createBackend, TOOLS } from "./index.js";

const registry = JSON.parse(
  readFileSync(new URL("./assets/words.json", import.meta.url), "utf8"),
);
const known = new Set();
for (const entry of registry.entries) {
  known.add(entry.name);
}
// Input-schema property descriptions count too: `source` carries the syntax
// rules, and a rule that names a Word which does not exist misleads exactly as
// a tool description would.
const descriptions = TOOLS.flatMap(({ description, inputSchema }) => [
  description,
  ...Object.values(inputSchema?.properties ?? {}).map((property) => property.description ?? ""),
]).join("\n");

// ── every family is represented ─────────────────────────────────────────
//
// A family counts as announced when the surface names at least one of its
// Words. Requiring the family's own label instead would pin the marketing
// wording rather than the coverage, and `knowledgeOrder` is not a phrase anyone
// should have to write.
const families = new Map();
for (const entry of registry.entries) {
  if (!families.has(entry.family)) families.set(entry.family, []);
  families.get(entry.family).push(entry.name);
}

// The families a caller reaches through `source` text. `dictionary`, `output`,
// `reflection`, `control` and `stackModifier` are spelled inside a program
// rather than chosen between, and `DEF` is named for the one case — defining a
// Word — a caller has to know is possible at all.
const MUST_ANNOUNCE = [
  "knowledgeOrder",
  "truthOrder",
  "collection",
  "higherOrder",
  "text",
  "absence",
];

const missing = MUST_ANNOUNCE.filter((family) => {
  const names = families.get(family) ?? [];
  return !names.some((name) => new RegExp(`\\b${escapeRegExp(name)}\\b`).test(descriptions));
});
assert.deepEqual(
  missing,
  [],
  `the tool descriptions announce no Word of ${missing.join(", ")}; a caller reading only them ` +
    "cannot know the engine does that, and the baseline measured exactly that cost",
);

// ── and every name they mention is real ─────────────────────────────────
//
// Uppercase tokens that look like Words. `NIL?` and `NIL-REASON` carry
// punctuation, so the pattern has to admit `?` and `-` without swallowing
// ordinary prose or the `RPN`/`MCP` style acronyms, which are excluded by
// being absent from the registry only if they were meant as Words — hence the
// allow-list rather than a cleverer regex.
const NOT_WORDS = new Set(["RPN", "MCP", "JSON", "UTF-", "UTF", "ASCII", "I", "O"]);
const mentioned = [...descriptions.matchAll(/\b[A-Z][A-Z-]*\??/g)]
  .map(([token]) => token)
  .filter((token) => token.length > 1 && !NOT_WORDS.has(token));
const unknown = [...new Set(mentioned)].filter((token) => !known.has(token));
assert.deepEqual(
  unknown,
  [],
  `the tool descriptions name ${unknown.join(", ")}, which the registry does not have. ` +
    "Sending a caller after a Word that does not exist is the failure the baseline " +
    "spent 46 of 118 turns on; the entry surface must not commit it too",
);

function escapeRegExp(text) {
  return text.replaceAll(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// ── and every literal Ajisai fragment in them actually parses ──────────────
//
// A description that shows example source is teaching syntax by
// demonstration — a caller trusts it as literally correct without running it
// first. "A block passed to MAP/FILTER/FOLD/ANY/ALL is in braces
// (`[ 1 2 3 4 ] { 2 MOD 0 = } FILTER`)" shipped for a time describing a
// bracket the tokenizer had already retired (`{`/`}` are not valid Ajisai
// source characters), because nothing here ever executed the sample the
// prose showed — only the Word *names* mentioned above were checked, not the
// code around them. Any backtick span shaped like source (it contains `[`,
// `'`, or is a bare postfix expression of digits/uppercase Words/operators)
// is run against the live backend the server itself answers with, the same
// guarantee mcp-quickstart.md's fenced examples get from `selftest.js` and
// SKILL.md's §6 examples get from `generate-skill-md.mjs`.
const looksLikeAjisaiSource = (span) =>
  /['[\]]/.test(span) || /^[0-9A-Z][0-9A-Z+\-*/<>=%!?. ]*$/.test(span);

const codeSpans = [
  ...new Set(
    [...descriptions.matchAll(/`([^`]+)`/g)]
      .map(([, span]) => span)
      .filter(looksLikeAjisaiSource),
  ),
];
assert.ok(
  codeSpans.length >= 5,
  "expected several source-shaped backtick spans across the tool descriptions to verify",
);

const liveBackend = createBackend();
assert.ok(liveBackend, "no execution backend available to verify tool-description code spans against");
for (const span of codeSpans) {
  const result = await liveBackend.check(span);
  assert.equal(
    result.status,
    "ok",
    `a tool description shows \`${span}\` as valid Ajisai, but check answered status=${result.status}` +
      (result.message ? ` (${result.message})` : ""),
  );
}

// ── and what they claim about the vocabulary is what the registry holds ────
//
// The compute description and the quickstart preface both count the Words and
// both call their list the whole vocabulary. Each claim was once hand-typed:
// "78 Words" beside a list of 72, and "case-sensitive" beside an engine that
// canonicalizes every name to upper case. Both are claims a caller acts on, so
// both are measured here instead of trusted.
const computeDescription = TOOLS.find(({ name }) => name === "compute").description;
const quickstartPreface = readFileSync(new URL("./mcp-quickstart.md", import.meta.url), "utf8");
for (const [surface, text, countPattern] of [
  ["the compute description", computeDescription, /Its (\d+) Words/],
  ["the quickstart preface", quickstartPreface, /The (\d+) Words are/],
]) {
  const claimed = Number(text.match(countPattern)?.[1]);
  assert.equal(
    claimed,
    known.size,
    `${surface} counts ${claimed} Words; the registry holds ${known.size}`,
  );
  const unnamed = [...known].filter(
    (name) => !new RegExp(`(^|[\\s(\`/])${escapeRegExp(name)}($|[\\s),\`/.;])`).test(text),
  );
  assert.deepEqual(unnamed, [], `${surface} calls its list the whole vocabulary but omits ${unnamed.join(", ")}`);
  assert.ok(
    !/case-sensitive/i.test(text),
    `${surface} calls Word names case-sensitive; the engine canonicalizes every name to upper case`,
  );
}

const liveCase = await liveBackend.compute("1 2 add");
assert.equal(
  liveCase.status,
  "ok",
  "the descriptions say Word names are case-insensitive, but `1 2 add` did not run",
);

console.log(
  `tool descriptions announce ${MUST_ANNOUNCE.length} Word families and name ` +
    `${new Set(mentioned).size} Words, all of which exist; ${codeSpans.length} source-shaped ` +
    "examples all verified against the live backend",
);
