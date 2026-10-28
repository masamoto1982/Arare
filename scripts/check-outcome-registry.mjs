#!/usr/bin/env node
// Projects the Rust NilReason and ErrorCategory enums from spec/outcomes.json
// and fails if either side names something the other does not, or if
// spec/words.json's projection.reason / errorWhen values point outside the
// registry.
//
// This is the same shape as check-runtime-metadata-source.mjs: the canonical
// source is the JSON, the Rust enum is the projection, and drift between them
// fails here rather than surfacing as a NilReason/ErrorCategory that
// spec/outcomes.json never declared. Extraction reads the two enums'
// `as_protocol_str` match arms directly out of rust/src/error.rs rather than
// compiling anything, the same tradeoff check-runtime-metadata-source.mjs
// makes for BuiltinSpec.
//
// `ErrorCategory::Declared(condition) => condition` is deliberately excluded
// from the extracted "structural" set: it carries no literal string of its own
// (its whole point is to forward a words.json errorWhen string verbatim), so
// `extractProtocolStrings`'s regex never matches its arm in the first place.
// `ErrorCategory::Custom` — the escape hatch a registry describing the
// *closed* outcome space had no room for — is gone: `AjisaiError::Custom` and
// the `From<String>`/`From<&str>` conversions that fed it no longer exist, so
// every raise site names a declared condition or a fixed structural variant.

import { matchBrace, readJson, readText, reporter, words } from './lib/common.mjs';

const report = reporter('outcome-registry');
const fail = report.fail;

// ---------------------------------------------------------------------------
// Rust extraction: the `as_protocol_str` match body for one enum, scanned by
// brace depth (matchBrace) so a nested arm (`Declared(condition) => condition`)
// or a string literal containing `}` cannot cut the scan short.
// ---------------------------------------------------------------------------

function extractProtocolStrings(source, enumName) {
  const fnMarker = `impl ${enumName} {`;
  const implStart = source.indexOf(fnMarker);
  if (implStart === -1) fail(`could not find "${fnMarker}" in rust/src/error.rs`);
  const fnStart = source.indexOf('pub fn as_protocol_str', implStart);
  if (fnStart === -1) fail(`could not find as_protocol_str in impl ${enumName}`);
  const braceOpen = source.indexOf('{', source.indexOf('match self', fnStart));
  const braceClose = matchBrace(source, braceOpen);
  if (braceClose === -1) fail(`unbalanced braces scanning ${enumName}::as_protocol_str`);
  const body = source.slice(braceOpen, braceClose);

  const pattern = new RegExp(`${enumName}::(\\w+)\\s*=>\\s*"([a-zA-Z0-9]+)"`, 'g');
  const found = new Map();
  let m;
  while ((m = pattern.exec(body)) !== null) {
    found.set(m[1], m[2]);
  }
  return found;
}

const errorRs = readText('rust/src/error.rs');

const nilReasonArms = extractProtocolStrings(errorRs, 'NilReason');
if (nilReasonArms.size === 0) {
  fail('extracted zero NilReason::as_protocol_str arms — the extractor is broken, not the enum');
}
const rustNilReasons = new Set(nilReasonArms.values());

const errorCategoryArms = extractProtocolStrings(errorRs, 'ErrorCategory');
if (errorCategoryArms.size === 0) {
  fail('extracted zero ErrorCategory::as_protocol_str arms — the extractor is broken, not the enum');
}
// `Declared`'s arm never matches the extractor's regex (it forwards a string
// rather than spelling one literally), so every arm this extraction finds is
// already a fixed structural variant — no filtering needed.
const rustStructuralErrorCategories = new Set(errorCategoryArms.values());


// ---------------------------------------------------------------------------
// spec/outcomes.json
// ---------------------------------------------------------------------------

const outcomes = readJson('spec/outcomes.json');
const registryNilReasons = new Set(outcomes.nilReasons.map((r) => r.id));
const registryErrorCategories = new Set(outcomes.errorCategories.map((c) => c.id));
const registryStructuralErrorCategories = new Set(
  outcomes.errorCategories.filter((c) => c.kind === 'structural').map((c) => c.id),
);

function diffSets(label, expected, actual) {
  const missing = [...expected].filter((x) => !actual.has(x)).sort();
  const extra = [...actual].filter((x) => !expected.has(x)).sort();
  if (missing.length > 0) {
    fail(`${label}: registry is missing ${JSON.stringify(missing)}`);
  }
  if (extra.length > 0) {
    fail(`${label}: registry declares ${JSON.stringify(extra)}, which Rust does not have`);
  }
}

// 1. spec/outcomes.json's nilReasons ≡ NilReason::as_protocol_str, exactly.
diffSets('nilReasons vs NilReason', rustNilReasons, registryNilReasons);

// 2. spec/outcomes.json's kind:"structural" errorCategories ≡
//    ErrorCategory::as_protocol_str's fixed variants, exactly.
diffSets(
  'structural errorCategories vs ErrorCategory',
  rustStructuralErrorCategories,
  registryStructuralErrorCategories,
);

// ---------------------------------------------------------------------------
// spec/words.json: every projection.reason and every errorWhen condition has
// to resolve into the registry. This is the half of the check that catches a
// words.json entry naming an outcome nothing else knows about.
// ---------------------------------------------------------------------------

const projectionReasons = new Set();
const errorWhenConditions = new Set();
for (const entry of words().entries) {
  const proj = entry.projection;
  // A projection.reason is one reason or an array of them (POW, GET, ...);
  // every element has to resolve. A null reason means the Word projects none.
  if (proj && typeof proj === 'object' && proj.reason != null) {
    for (const reason of [].concat(proj.reason)) {
      if (typeof reason === 'string') {
        projectionReasons.add(reason);
      } else {
        fail(`spec/words.json ${entry.name}: projection.reason element ${JSON.stringify(reason)} is not a string`);
      }
    }
  }
  for (const condition of entry.errorWhen ?? []) {
    errorWhenConditions.add(condition);
  }
}

const missingProjectionReasons = [...projectionReasons].filter((r) => !registryNilReasons.has(r)).sort();
if (missingProjectionReasons.length > 0) {
  fail(
    `spec/words.json declares projection.reason values not in spec/outcomes.json's nilReasons: ` +
      JSON.stringify(missingProjectionReasons),
  );
}

// A projection.reason names a Word actually projecting that reason, so it has
// to be one the registry marks projectable — `literal` is written by source
// text, never by a Word's own projection, and using it here would be a
// words.json entry claiming a projection nothing implements.
const nonProjectableUsedAsProjection = outcomes.nilReasons
  .filter((r) => projectionReasons.has(r.id) && r.projectable !== true)
  .map((r) => r.id)
  .sort();
if (nonProjectableUsedAsProjection.length > 0) {
  fail(
    `spec/words.json's projection.reason names a reason spec/outcomes.json marks non-projectable: ` +
      JSON.stringify(nonProjectableUsedAsProjection),
  );
}

const missingErrorWhenConditions = [...errorWhenConditions].filter((c) => !registryErrorCategories.has(c)).sort();
if (missingErrorWhenConditions.length > 0) {
  fail(
    `spec/words.json declares errorWhen conditions not in spec/outcomes.json's errorCategories: ` +
      JSON.stringify(missingErrorWhenConditions),
  );
}

// ---------------------------------------------------------------------------

report.done(
  `${registryNilReasons.size} NIL reasons, ${registryErrorCategories.size} error categories ` +
    `(${registryStructuralErrorCategories.size} structural), all cross-checked against Rust and spec/words.json.`,
);
