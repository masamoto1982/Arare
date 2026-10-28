#!/usr/bin/env node
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { coverageEntryMatches, readJson, readText, repoRoot, words, writeOrCheck } from './lib/common.mjs';


function fail(message) {
  throw new Error(`[word-manifest] ${message}`);
}

// The returned body *includes* the closing `\n];`. The entry patterns below
// delimit each item by looking ahead to either the next item or that
// terminator, so cutting it off silently dropped whichever entry happened to be
// last — `^` -> OR-NIL and the final surface-form entry went missing from the
// manifest that
// way, and the drift check could not see it because the committed file matched
// the generator's own truncated output.
function constArrayBody(source, constName) {
  const startPattern = new RegExp(`(?:pub\\([^)]*\\)\\s+)?(?:pub\\s+)?const\\s+${constName}[^=]*=\\s*&\\[`);
  const start = source.search(startPattern);
  if (start < 0) fail(`could not find const array ${constName}`);
  const open = source.indexOf('[', source.indexOf('&[', start));
  const end = source.indexOf('\n];', open);
  if (end < 0) fail(`could not find end of const array ${constName}`);
  return source.slice(open + 1, end + '\n];'.length);
}

// A struct literal per entry, so the count of `Name {` openings is how many
// entries the table has. Comparing that against what the item pattern extracted
// turns a lookahead that quietly matches too little into a failed build.
function assertExtractedEveryEntry(body, structName, extracted, sourcePath) {
  const declared = body.match(new RegExp(`\\b${structName}\\s*{`, 'g'))?.length ?? 0;
  if (declared !== extracted) {
    fail(`${sourcePath}: ${structName} declares ${declared} entries but ${extracted} were extracted`);
  }
}

function slug(value) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
}

function symbolSlug(value) {
  const names = {
    '+': 'plus',
    '-': 'minus',
    '*': 'asterisk',
    '/': 'slash',
    '%': 'percent',
    '=': 'equals',
    '<': 'less-than',
    '<=': 'less-than-or-equal',
    '>': 'greater-than',
    '>=': 'greater-than-or-equal',
    '<>': 'legacy-not-equal',
    '!=': 'not-equal',
    '!': 'bang',
    '&': 'ampersand',
    '.': 'dot',
    '..': 'dot-dot',
    ',': 'comma',
    ',,': 'comma-comma',
    "'": 'quote',
    '?': 'question',
    '~': 'tilde',
    '^': 'caret',
    '#': 'hash',
    '|': 'pipe',
    '[': 'left-bracket',
    ']': 'right-bracket',
    '{': 'left-brace',
    '}': 'right-brace',
    ';': 'semicolon',
    ';;': 'semicolon-semicolon',
    '(': 'left-paren',
    ')': 'right-paren',
  };
  return names[value] ?? slug(value);
}


function loadCoverageEntries() {
  if (!existsSync(resolve(repoRoot, 'docs/formalization-coverage.json'))) return [];
  const coverage = readJson('docs/formalization-coverage.json');
  if (!Array.isArray(coverage.entries)) return [];
  return coverage.entries;
}

function canonicalForEntry(entry, coverageEntry) {
  if (typeof entry.canonical === 'string' && entry.canonical.trim() !== '') return entry.canonical;
  if (typeof coverageEntry?.canonical === 'string' && coverageEntry.canonical.trim() !== '') return coverageEntry.canonical;
  if (typeof coverageEntry?.desugars_to === 'string' && coverageEntry.desugars_to.trim() !== '') return coverageEntry.desugars_to;
  if (typeof entry.concept === 'string' && entry.concept.trim() !== '') return entry.concept;
  return entry.surface;
}

function semanticMetadataForEntry(entry, coverageEntries) {
  const exact = coverageEntries.find((candidate) => candidate.id === entry.id);
  const coverageEntry = exact ?? coverageEntries.find((candidate) => coverageEntryMatches(candidate, entry));
  const metadata = {
    canonical: canonicalForEntry(entry, coverageEntry),
  };
  if (coverageEntry) {
    metadata.coverage_entry_id = coverageEntry.id;
    for (const key of [
      'semantic_role',
      'algebraic_family',
      'core_tier',
      'derived_from',
      'desugars_to',
      'capability',
      'effect_schema',
      'reason',
      'exit_options',
      'review_gate',
      'implementation_schema',
      'classification',
    ]) {
      if (key in coverageEntry) metadata[key] = coverageEntry[key];
    }
  }
  return metadata;
}

function rustEnumVariantToSnake(value) {
  return value.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase();
}

function extractCoreWords() {
  const sourcePath = 'spec/words.json';
  const parsed = words().entries.map((word) => ({
    name: word.name,
    family: word.family,
    vocabularyTier: word.vocabularyTier,
  }));
  if (parsed.length === 0) fail('no core words extracted');

  const baseCounts = new Map();
  for (const { name } of parsed) {
    const base = slug(name);
    baseCounts.set(base, (baseCounts.get(base) ?? 0) + 1);
  }
  return parsed.map(({ name, family, vocabularyTier }) => {
    const base = slug(name);
    const dropped = name.replace(/[a-zA-Z0-9]+/g, '');
    let id = `core.${base}`;
    if (baseCounts.get(base) > 1 && dropped) {
      id = `core.${base}${dropped.includes('?') ? '-p' : `-${slug(dropped) || 'x'}`}`;
    }
    return { id, kind: 'coreword', surface: name, family, vocabularyTier, source: sourcePath };
  });
}

function extractSurfaceForms() {
  const sourcePath = 'rust/src/surface_forms.rs';
  const body = constArrayBody(readText(sourcePath), 'SURFACE_FORMS');
  const entries = [];
  const pattern = /SurfaceForm\s*{([\s\S]*?)(?=\n\s*SurfaceForm\s*{|\n\s*\];)/g;
  for (const match of body.matchAll(pattern)) {
    const item = match[1];
    const surface = item.match(/\bsurface:\s*"([^"]+)"/)?.[1];
    const concept = item.match(/\bconcept:\s*"([^"]+)"/)?.[1];
    const kind = item.match(/\bkind:\s*SurfaceFormKind::([A-Za-z0-9_]+)/)?.[1];
    const runtimeWord = item.match(/\bruntime_word:\s*(true|false)/)?.[1];
    if (!surface || !concept || !kind || !runtimeWord) continue;
    entries.push({
      id: `surface.${symbolSlug(surface)}`,
      kind: rustEnumVariantToSnake(kind),
      surface,
      concept,
      runtime_word: runtimeWord === 'true',
      source: sourcePath,
    });
  }
  if (entries.length === 0) fail('no surface forms extracted');
  assertExtractedEveryEntry(body, 'SurfaceForm', entries.length, sourcePath);
  return entries;
}

const entries = [
  ...extractCoreWords(),
  ...extractSurfaceForms(),
];

const contracts = words();

const coverageEntries = loadCoverageEntries();
for (const entry of entries) {
  Object.assign(entry, semanticMetadataForEntry(entry, coverageEntries));
}

const seen = new Set();
for (const entry of entries) {
  if (seen.has(entry.id)) fail(`duplicate manifest id ${entry.id}`);
  seen.add(entry.id);
}

const manifest = {
  schemaVersion: 2,
  generatedFrom: [
    'spec/words.json',
    'rust/src/surface_forms.rs',
  ],
  semanticMetadataFrom: 'docs/formalization-coverage.json',
  counts: {
    canonicalWords: contracts.entries.length,
    semanticKernelWords: contracts.entries.filter((entry) => entry.vocabularyTier === 'kernel').length,
    standardWords: contracts.entries.filter((entry) => entry.vocabularyTier === 'standard').length,
    corewords: entries.filter((entry) => entry.kind === 'coreword').length,
    surface_forms: entries.filter((entry) => entry.kind !== 'coreword').length,
    // Deliberately no grand total: a surface form is not a Word, so summing
    // it with `canonicalWords` would publish a vocabulary size the language
    // does not have. `manifestEntries` counts rows
    // in this file and is named so it cannot be read as a Word count.
    manifestEntries: entries.length,
  },
  entries,
};

const json = `${JSON.stringify(manifest, null, 2)}\n`;
if (process.argv.includes('--stdout') && !process.argv.includes('--check')) {
  process.stdout.write(json);
} else {
  // The check is the CI drift guard: fail if the committed manifest is out of
  // sync with what the generator now produces, so a new/last BuiltinSpec (e.g.
  // SUPERVISE) can never silently go missing again.
  writeOrCheck(
    'word-manifest',
    [{
      path: 'docs/word-manifest.json',
      content: json,
      stale: 'docs/word-manifest.json is stale. Run `npm run word:manifest` and commit the result.',
    }],
    {
      current: `docs/word-manifest.json is up to date (${entries.length} entries).`,
      wrote: `wrote ${entries.length} entries to docs/word-manifest.json`,
    },
  );
}
