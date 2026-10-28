#!/usr/bin/env node
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { coverageEntryMatches, coverageSurfaces, normalizeSurface, readJson, readText, repoRoot } from './lib/common.mjs';

const allowedStatuses = new Set([
  'Formalized',
  'Sketched',
  'HostedEffect',
  'Exploratory',
  'NotPortableYet',
  'Deprecated',
]);

const allowedSemanticRoles = new Set([
  'Primitive',
  'Derived',
  'Sugar',
  'HostedEffect',
  'Exploratory',
  'NotPortableYet',
  'Extension',
  'Deprecated',
]);

const allowedPrimitiveStatuses = new Set([
  'accepted',
  'provisional',
  'deprecated',
]);

const allowedAlgebraicFamilies = new Set([
  'state-transformer',
  'stack',
  'dictionary',
  'exact-scalar',
  'exact-arithmetic',
  'boolean-truth',
  'absence',
  'structure-lift',
  'hosted-effect',
  'syntax-sugar',
  'observation',
  'reflection',
  'record',
]);

const allowedCoreTiers = new Set([
  'identity',
  'flow',
  'material',
  'sugar',
]);

function fail(message) {
  throw new Error(`[formalization-coverage] ${message}`);
}

function hasNonEmptyString(value) {
  return typeof value === 'string' && value.trim() !== '';
}

function hasNonEmptyStringArray(value) {
  return Array.isArray(value) && value.length > 0 && value.every((item) => hasNonEmptyString(item));
}

function hasSchemaValue(value) {
  if (hasNonEmptyString(value)) return true;
  if (hasNonEmptyStringArray(value)) return true;
  return value && typeof value === 'object' && !Array.isArray(value) && Object.keys(value).length > 0;
}


function validateUniqueEntryIds(entries) {
  const seenIds = new Map();
  for (const [index, entry] of entries.entries()) {
    const where = entry?.id ?? `entry #${index}`;
    if (!entry || typeof entry !== 'object') fail(`${where}: entry must be an object`);
    if (typeof entry.id !== 'string' || entry.id.trim() === '') {
      fail(`${where}: missing non-empty id`);
    }
    if (seenIds.has(entry.id)) {
      const firstIndex = seenIds.get(entry.id);
      fail(`${entry.id}: duplicate id at entries[${index}] (first seen at entries[${firstIndex}])`);
    }
    if (!allowedCoreTiers.has(entry.core_tier)) {
      fail(`${entry.id}: entry needs a known core_tier (identity|flow|material|sugar)`);
    }
    seenIds.set(entry.id, index);
  }
}

function validateWordManifest(coverage) {
  const manifest = readJson('docs/word-manifest.json');
  if (manifest.schemaVersion !== 2) fail('word manifest schemaVersion must be 2');
  if (!Array.isArray(manifest.entries)) fail('word manifest entries must be an array');

  const manifestById = new Map();
  const manifestBySurface = new Map();
  for (const [index, entry] of manifest.entries.entries()) {
    const where = entry?.id ?? `manifest entry #${index}`;
    if (!entry || typeof entry !== 'object') fail(`${where}: manifest entry must be an object`);
    for (const key of ['id', 'kind', 'surface']) {
      if (typeof entry[key] !== 'string' || entry[key].trim() === '') {
        fail(`${where}: manifest entry missing non-empty ${key}`);
      }
    }
    if (!hasNonEmptyString(entry.canonical)) fail(`${where}: manifest entry missing non-empty canonical`);
    if (!hasNonEmptyString(entry.semantic_role)) fail(`${where}: manifest entry missing non-empty semantic_role`);
    if (!allowedSemanticRoles.has(entry.semantic_role)) {
      fail(`${where}: manifest entry has invalid semantic_role ${entry.semantic_role}`);
    }
    if (entry.semantic_role === 'Sugar' && !hasNonEmptyString(entry.desugars_to)) {
      fail(`${where}: Sugar manifest entries need desugars_to`);
    }
    if (entry.semantic_role === 'HostedEffect' && (!hasSchemaValue(entry.capability) || !hasSchemaValue(entry.effect_schema))) {
      fail(`${where}: HostedEffect manifest entries need capability and effect_schema`);
    }
    if (manifestById.has(entry.id)) fail(`${where}: duplicate manifest id`);
    manifestById.set(entry.id, entry);
    const surfaceKey = normalizeSurface(entry.surface);
    if (!manifestBySurface.has(surfaceKey)) manifestBySurface.set(surfaceKey, []);
    manifestBySurface.get(surfaceKey).push(entry);
    if (typeof entry.short_surface === 'string') {
      const shortKey = normalizeSurface(entry.short_surface);
      if (!manifestBySurface.has(shortKey)) manifestBySurface.set(shortKey, []);
      manifestBySurface.get(shortKey).push(entry);
    }
  }

  const coveredManifestIds = new Set();
  for (const entry of coverage.entries) {
    for (const manifestEntry of manifest.entries) {
      if (coverageEntryMatches(entry, manifestEntry)) coveredManifestIds.add(manifestEntry.id);
    }

    const isSurfaceEntry = ['coreword', 'moduleword', 'delimiter_sugar', 'literal_sugar', 'modifier_sugar', 'source_directive', 'control_directive', 'reserved_marker', 'retired_form', 'conversion_word'].includes(entry.kind);
    if (isSurfaceEntry && !manifestById.has(entry.id)) {
      const hasSurfaceMatch = coverageSurfaces(entry).some((surface) => {
        const normalized = normalizeSurface(surface);
        return [...manifestBySurface.keys()].some((manifestSurface) => normalized.includes(manifestSurface));
      });
      if (!hasSurfaceMatch) fail(`${entry.id}: coverage surface entry is not present in word manifest`);
    }
  }

  const allowlist = Array.isArray(coverage.unclassified_allowlist) ? coverage.unclassified_allowlist : [];
  const allowlistedIds = new Set();
  for (const id of allowlist) {
    if (typeof id !== 'string' || id.trim() === '') fail('unclassified_allowlist entries must be non-empty strings');
    if (!manifestById.has(id)) fail(`unclassified_allowlist references unknown manifest id ${id}`);
    if (allowlistedIds.has(id)) fail(`unclassified_allowlist contains duplicate id ${id}`);
    allowlistedIds.add(id);
  }

  // Every symbol the language allocates gets its *own* ledger entry, matched by
  // id rather than by the loose surface matching above. Without this, a sugar
  // entry could be "classified" only because some other entry's example happened
  // to contain the character — which is how `/`, `^` and `)` sat in the manifest
  // with no entry of their own.
  const SUGAR_KINDS = new Set([
    'delimiter_sugar',
    'literal_sugar', 'source_directive', 'control_directive', 'reserved_marker',
    'retired_form',
  ]);
  const coverageIds = new Set(coverage.entries.map((entry) => entry.id));
  const unledgered = manifest.entries
    .filter((entry) => SUGAR_KINDS.has(entry.kind) && !coverageIds.has(entry.id))
    .map((entry) => `${entry.surface} (${entry.id})`);
  if (unledgered.length > 0) {
    fail(`${unledgered.length} surface form(s) have no coverage entry of their own: ${unledgered.join(', ')}`);
  }

  const unclassified = [];
  for (const entry of manifest.entries) {
    if (!coveredManifestIds.has(entry.id)) unclassified.push(entry.id);
  }
  const unexpectedUnclassified = unclassified.filter((id) => !allowlistedIds.has(id));
  if (unexpectedUnclassified.length > 0) {
    fail(`${unexpectedUnclassified.length} surface word(s) are unclassified and not allowlisted: ${unexpectedUnclassified.join(', ')}`);
  }

  const staleAllowlist = [...allowlistedIds].filter((id) => !unclassified.includes(id));
  if (staleAllowlist.length > 0) {
    fail(`unclassified_allowlist contains already-classified id(s): ${staleAllowlist.join(', ')}`);
  }

  const classifiedCount = manifest.entries.length - unclassified.length;
  const percent = manifest.entries.length === 0 ? 100 : (classifiedCount / manifest.entries.length) * 100;
  console.log(`[formalization-coverage] ${classifiedCount}/${manifest.entries.length} surface words classified (${percent.toFixed(1)}%)`);
  if (unclassified.length > 0) {
    console.log(`[formalization-coverage] ${unclassified.length} unclassified surface word(s) currently allowlisted: ${unclassified.join(', ')}`);
  }
}

const coverage = readJson('docs/formalization-coverage.json');
if (coverage.version !== 1) fail('version must be 1');
if (!Array.isArray(coverage.entries)) fail('entries must be an array');
validateUniqueEntryIds(coverage.entries);
validateWordManifest(coverage);

// Optional algebra-primitive registry. When present it closes the
// `derived_from` vocabulary: every derived word must resolve to a declared
// semantic primitive, so the derivation graph is checked rather than free-form.
// Absent => the reference check is skipped (backward compatible).
let primitiveIds = null;
const knownSurfaces = new Set([
  ...readJson('spec/words.json').entries.map((word) => word.name),
  ...coverage.entries.map((entry) => entry.surface),
]);
if ('algebra_primitives' in coverage) {
  if (!Array.isArray(coverage.algebra_primitives)) fail('algebra_primitives must be an array');
  primitiveIds = new Set();
  for (const [index, prim] of coverage.algebra_primitives.entries()) {
    const pw = prim?.id ?? `algebra_primitive #${index}`;
    if (!prim || typeof prim !== 'object') fail(`${pw}: primitive must be an object`);
    if (typeof prim.id !== 'string' || prim.id.trim() === '') fail(`${pw}: missing non-empty id`);
    if (primitiveIds.has(prim.id)) fail(`${pw}: duplicate primitive id`);
    primitiveIds.add(prim.id);
    if (
      typeof prim.algebraic_family !== 'string' ||
      !allowedAlgebraicFamilies.has(prim.algebraic_family)
    ) {
      fail(`${pw}: primitive needs a known algebraic_family`);
    }
    if (!allowedCoreTiers.has(prim.core_tier)) {
      fail(`${pw}: primitive needs a known core_tier (identity|flow|material|sugar)`);
    }
    if (!hasNonEmptyString(prim.description)) fail(`${pw}: primitive needs description`);
    if (!hasNonEmptyString(prim.kind)) fail(`${pw}: primitive needs kind`);
    if (!hasNonEmptyString(prim.admission_reason)) fail(`${pw}: primitive needs admission_reason`);
    if (!hasNonEmptyStringArray(prim.introduced_by)) fail(`${pw}: primitive needs non-empty introduced_by`);
    if (!hasNonEmptyStringArray(prim.can_derive)) fail(`${pw}: primitive needs non-empty can_derive`);
    if (!hasNonEmptyString(prim.status) || !allowedPrimitiveStatuses.has(prim.status)) {
      fail(`${pw}: primitive needs status accepted|provisional|deprecated`);
    }
    // The names a primitive is introduced by and can derive are Words the
    // language has or surfaces this file covers. Unchecked, the two lists kept
    // 156 names of retired Words and module Words long after they were gone.
    for (const key of ['introduced_by', 'can_derive']) {
      for (const name of prim[key]) {
        if (!knownSurfaces.has(name)) fail(`${pw}: ${key} names ${name}, which is neither a Word nor a covered surface`);
      }
    }
  }
}

// `core_tier_summary.entry_counts` restates how many entries sit in each tier;
// it is checked against the entries rather than trusted.
if (coverage.core_tier_summary?.entry_counts) {
  const counted = {};
  for (const entry of coverage.entries) counted[entry.core_tier] = (counted[entry.core_tier] ?? 0) + 1;
  for (const [tier, stated] of Object.entries(coverage.core_tier_summary.entry_counts)) {
    if ((counted[tier] ?? 0) !== stated) {
      fail(`core_tier_summary.entry_counts.${tier} is ${stated}, but ${counted[tier] ?? 0} entries sit in that tier`);
    }
  }
}

const conformanceHtml = readText('tests/conformance/index.html');
const caseIds = new Set(
  [...conformanceHtml.matchAll(/<section\b[^>]*\bclass=["'][^"']*\bajisai-case\b[^"']*["'][^>]*\bid=["']([^"']+)["']/g)]
    .map((match) => match[1]),
);

for (const [index, entry] of coverage.entries.entries()) {
  const where = entry?.id ?? `entry #${index}`;
  if (!entry || typeof entry !== 'object') fail(`${where}: entry must be an object`);
  for (const key of ['id', 'kind', 'status']) {
    if (typeof entry[key] !== 'string' || entry[key].trim() === '') {
      fail(`${where}: missing non-empty ${key}`);
    }
  }
  if (!allowedStatuses.has(entry.status)) fail(`${where}: invalid status ${entry.status}`);

  const conformanceCases = Array.isArray(entry.conformance_cases)
    ? entry.conformance_cases.filter(Boolean)
    : [];
  const lawTests = Array.isArray(entry.law_tests) ? entry.law_tests.filter(Boolean) : [];

  if (entry.status === 'Formalized') {
    if (conformanceCases.length === 0 && lawTests.length === 0) {
      fail(`${where}: Formalized entries need conformance_cases or law_tests`);
    }
  }

  if (entry.status === 'NotPortableYet' && entry.classification === 'Core') {
    fail(`${where}: NotPortableYet entries must not be classified as Core`);
  }


  if ('semantic_role' in entry) {
    if (!allowedSemanticRoles.has(entry.semantic_role)) {
      fail(`${where}: invalid semantic_role ${entry.semantic_role}`);
    }

    if (entry.semantic_role === 'Derived') {
      if (!Array.isArray(entry.derived_from)) {
        fail(`${where}: Derived entries need derived_from`);
      }
      if (entry.derived_from.length === 0 && !/derived_from/i.test(entry.notes ?? '')) {
        fail(`${where}: Derived entries with empty derived_from must document the investigation gap in notes`);
      }
    }

    // When the registry is declared, every derived_from reference must
    // resolve to a known semantic primitive (closed derivation vocabulary).
    if (primitiveIds && Array.isArray(entry.derived_from)) {
      for (const ref of entry.derived_from) {
        if (!primitiveIds.has(ref)) {
          fail(`${where}: derived_from references unknown algebra primitive ${ref}`);
        }
      }
    }

    if (entry.semantic_role === 'Primitive' && entry.primitive !== true) {
      fail(`${where}: Primitive entries must set primitive to true`);
    }

    if (entry.semantic_role === 'Sugar' && !hasNonEmptyString(entry.desugars_to)) {
      fail(`${where}: Sugar entries need desugars_to`);
    }

    if (entry.semantic_role === 'HostedEffect' && (!hasSchemaValue(entry.capability) || !hasSchemaValue(entry.effect_schema))) {
      fail(`${where}: HostedEffect entries need capability and effect_schema`);
    }

    if (entry.semantic_role === 'Exploratory') {
      if (!hasNonEmptyString(entry.reason)) fail(`${where}: Exploratory entries need reason`);
      if (!hasNonEmptyStringArray(entry.exit_options)) fail(`${where}: Exploratory entries need non-empty exit_options`);
      if (!hasNonEmptyString(entry.review_gate)) fail(`${where}: Exploratory entries need review_gate`);
      if (entry.classification === 'Core') fail(`${where}: Exploratory entries must not be classified as Core`);
    }

    if (entry.semantic_role === 'HostedEffect' && entry.classification === 'Core') {
      fail(`${where}: HostedEffect semantic roles must not be classified as Core`);
    }

    if (entry.semantic_role === 'NotPortableYet' && entry.classification === 'Core') {
      fail(`${where}: NotPortableYet semantic roles must not be classified as Core`);
    }
  }

  if ('algebraic_family' in entry && !allowedAlgebraicFamilies.has(entry.algebraic_family)) {
    fail(`${where}: unknown algebraic_family ${entry.algebraic_family}`);
  }

  for (const caseId of conformanceCases) {
    if (!caseIds.has(caseId)) fail(`${where}: unknown conformance case ${caseId}`);
  }

  for (const lawTest of lawTests) {
    if (/\.(rs|ts|js|mjs)$/.test(lawTest) && !existsSync(resolve(repoRoot, lawTest))) {
      fail(`${where}: referenced law test file does not exist: ${lawTest}`);
    }
  }
}

if (primitiveIds) {
  const used = new Set();
  for (const entry of coverage.entries) {
    for (const ref of Array.isArray(entry.derived_from) ? entry.derived_from : []) used.add(ref);
  }
  const unused = [...primitiveIds].filter((id) => !used.has(id));
  if (unused.length > 0) {
    // Non-fatal: a declared-but-unused primitive is a hint of dead metadata,
    // not a hard error, so it does not break backward-compatible consumers.
    console.log(`[formalization-coverage] note: ${unused.length} declared primitive(s) unused: ${unused.join(', ')}`);
  }

  // Non-fatal coherence note: an entry's algebraic_family should normally name
  // a family it actually rests on, i.e. one of its derived_from primitives'
  // families. `observation` and `syntax-sugar` are deliberate exceptions: they
  // describe a projection/surface role rather than the family being rested on.
  // Surfacing drift here would have caught e.g. a datetime word mislabeled with
  // the text scalar family while resting only on arithmetic/structure primitives.
  const primitiveFamily = new Map(
    coverage.algebra_primitives.map((p) => [p.id, p.algebraic_family]),
  );
  const projectionFamilies = new Set(['observation', 'syntax-sugar']);
  const incoherent = [];
  for (const entry of coverage.entries) {
    const family = entry.algebraic_family;
    const refs = Array.isArray(entry.derived_from) ? entry.derived_from : [];
    if (typeof family !== 'string' || refs.length === 0) continue;
    if (projectionFamilies.has(family)) continue;
    const restsOn = new Set(refs.map((ref) => primitiveFamily.get(ref)));
    if (!restsOn.has(family)) {
      incoherent.push(`${entry.id} (${family} <= ${[...restsOn].filter(Boolean).join(', ')})`);
    }
  }
  if (incoherent.length > 0) {
    console.log(`[formalization-coverage] note: ${incoherent.length} entry/families not among their derived_from families: ${incoherent.join('; ')}`);
  }

  // Non-fatal traceability note: every declared primitive should be reachable
  // from at least one test. We invert derived_from -> {law_tests, conformance}
  // and flag primitives that no resting word exercises, so a newly admitted
  // primitive cannot stay untested unnoticed.
  const primitiveTested = new Map([...primitiveIds].map((id) => [id, false]));
  for (const entry of coverage.entries) {
    const hasTest =
      (Array.isArray(entry.law_tests) && entry.law_tests.some((t) => hasNonEmptyString(t))) ||
      (Array.isArray(entry.conformance_cases) && entry.conformance_cases.some((c) => hasNonEmptyString(c)));
    if (!hasTest) continue;
    for (const ref of Array.isArray(entry.derived_from) ? entry.derived_from : []) {
      if (primitiveTested.has(ref)) primitiveTested.set(ref, true);
    }
  }
  const untestedPrimitives = [...primitiveTested.entries()].filter(([, t]) => !t).map(([id]) => id);
  if (untestedPrimitives.length > 0) {
    console.log(`[formalization-coverage] note: ${untestedPrimitives.length} primitive(s) exercised by no test: ${untestedPrimitives.join(', ')}`);
  }

  console.log(`[formalization-coverage] ${primitiveIds.size} algebra primitives declared`);
}

console.log(`[formalization-coverage] ${coverage.entries.length} entries validated`);
