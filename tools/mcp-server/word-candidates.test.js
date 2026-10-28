#!/usr/bin/env node
// Pins the "did you mean" suggester `word_contract` answers an unmatched name
// with, over the same registry asset the tool reads in production.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { suggestWords } from "./word-candidates.js";

const registry = JSON.parse(
  readFileSync(new URL("./assets/words.json", import.meta.url), "utf8"),
);

// The cases live in word-candidates.cases.json, which the Rust suggester's
// test (rust/src/diagnostic_hint_tests.rs) reads too.
const { cases: CASES } = JSON.parse(
  readFileSync(new URL("./word-candidates.cases.json", import.meta.url), "utf8"),
);

for (const [word, expected] of CASES) {
  assert.deepEqual(suggestWords(word, registry.entries), expected, `suggestWords(${JSON.stringify(word)})`);
}

console.log(`word-candidates: ${CASES.length} cases.`);
