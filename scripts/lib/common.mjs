// Plumbing every scripts/*.mjs gate and generator shares: where the repository
// is, how a committed file is read, how a failure is reported, how the tree is
// walked, how the native CLI is run, and how a generated file is written or
// checked. Each script keeps its own tag and message text — those are what CI
// logs show — and passes them in; what lives here is only the mechanism.
//
// Nothing here decides anything about the language. A rule that belongs to
// Ajisai (an outcome id's spelling, a family's laws) is here only where
// several scripts must read it identically, and it says so where it is defined.

import { execFileSync, spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

export const repoRoot = resolve(import.meta.dirname, '../..');

/** A repository file as text; `path` is relative to the repository root. */
export const readText = (path) => readFileSync(resolve(repoRoot, path), 'utf8');

/** A repository file parsed as JSON. */
export const readJson = (path) => JSON.parse(readText(path));

let wordsDocument;
/**
 * spec/words.json, parsed once per process. A Word's name is its identity
 * everywhere downstream (the registry, the manifest, the reference, the
 * gates), so the one thing asserted here, for every reader at once, is that
 * no name appears twice: a duplicate throws instead of letting each script
 * discover it on its own.
 */
export function words() {
  if (wordsDocument === undefined) {
    const document = readJson('spec/words.json');
    const seen = new Set();
    for (const { name } of document.entries) {
      if (seen.has(name)) throw new Error(`spec/words.json names the Word ${name} twice`);
      seen.add(name);
    }
    wordsDocument = document;
  }
  return wordsDocument;
}

/**
 * The semantic families, derived from spec/words.json: one per distinct
 * `family` a Word names, in order of first appearance, whose `clauses` are the
 * clauses every member cites. A family is the laws its Words share, so its
 * clause list is the intersection of theirs — no more, which would claim a law
 * some member is not subject to, and no fewer.
 */
export function families() {
  const entries = words().entries;
  return [...new Set(entries.map((word) => word.family))].map((id) => {
    const members = entries.filter((word) => word.family === id);
    const clauses = members[0].clauses.filter((clause) => members.every((word) => word.clauses.includes(clause)));
    return { id, clauses };
  });
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

/** Print `[tag] message` to stderr and exit 1. */
export function fatal(tag, message) {
  console.error(`[${tag}] ${message}`);
  process.exit(1);
}

/**
 * A failure collector for one script. `fail` prints `[tag] message` at once and
 * marks the run failed; `done` exits 1 if anything failed (after a
 * `[tag] N failure(s)` line when `countSummary` is set), and otherwise prints
 * the success line.
 */
export function reporter(tag, { countSummary = false } = {}) {
  let count = 0;
  return {
    get count() {
      return count;
    },
    get failed() {
      return count > 0;
    },
    fail(message) {
      count += 1;
      process.exitCode = 1;
      console.error(`[${tag}] ${message}`);
    },
    fatal: (message) => fatal(tag, message),
    done(message) {
      if (count > 0) {
        if (countSummary) console.error(`[${tag}] ${count} failure(s)`);
        process.exit(1);
      }
      if (message !== undefined) console.log(`[${tag}] ${message}`);
    },
  };
}

// ---------------------------------------------------------------------------
// Walking the tree
// ---------------------------------------------------------------------------

/** Build output, dependencies and dot-directories are never a script's corpus. */
export const skipBuildAndHidden = (name) =>
  name === 'node_modules' || name === 'target' || name === 'dist' || name.startsWith('.');

/**
 * Every file under `dir` whose name ends in one of `match` (an array of
 * suffixes) or matches it (a RegExp), in directory order. A name `skip`
 * accepts is not entered or collected; a missing `dir` yields nothing.
 */
export function walk(dir, match, skip = skipBuildAndHidden, out = []) {
  if (!existsSync(dir)) return out;
  for (const name of readdirSync(dir)) {
    if (skip(name)) continue;
    const full = join(dir, name);
    if (statSync(full).isDirectory()) walk(full, match, skip, out);
    else if (Array.isArray(match) ? match.some((ext) => name.endsWith(ext)) : match.test(name)) out.push(full);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Text extraction
// ---------------------------------------------------------------------------

/** Decode the five HTML entities the conformance suite writes. */
export function decodeEntities(value) {
  return value
    .replaceAll('&lt;', '<')
    .replaceAll('&gt;', '>')
    .replaceAll('&quot;', '"')
    .replaceAll('&#39;', "'")
    .replaceAll('&apos;', "'")
    .replaceAll('&amp;', '&');
}

/**
 * The index of the `}` closing the `{` at `openIndex` in Rust source, scanned
 * by brace depth so a nested block or a string literal containing `}` cannot
 * cut the scan short; -1 if it never closes.
 */
export function matchBrace(source, openIndex) {
  let depth = 0;
  let inString = false;
  for (let i = openIndex; i < source.length; i += 1) {
    const char = source[i];
    if (inString) {
      if (char === '\\') i += 1;
      else if (char === '"') inString = false;
      continue;
    }
    if (char === '"') inString = true;
    else if (char === '{') depth += 1;
    else if (char === '}') {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

// ---------------------------------------------------------------------------
// docs/formalization-coverage.json <-> docs/word-manifest.json
// ---------------------------------------------------------------------------

export function normalizeSurface(value) {
  return typeof value === 'string' ? value.trim().toUpperCase() : '';
}

/** Every surface spelling a formalization-coverage entry names. */
export function coverageSurfaces(entry) {
  const surfaces = [];
  if (typeof entry.surface === 'string') surfaces.push(entry.surface);
  if (Array.isArray(entry.surfaces)) surfaces.push(...entry.surfaces.filter((value) => typeof value === 'string'));
  return surfaces;
}

/**
 * Whether a formalization-coverage entry classifies a manifest entry: the same
 * id, or a coverage surface that is — or contains as a token — the manifest
 * entry's surface or one of its `coverage_aliases`. The manifest generator
 * attaches semantic metadata by this rule and the coverage gate counts a
 * surface classified by it, so the two cannot disagree about a match.
 */
export function coverageEntryMatches(coverageEntry, manifestEntry) {
  if (coverageEntry.id === manifestEntry.id) return true;
  const manifestSurface = normalizeSurface(manifestEntry.surface);
  const aliases = Array.isArray(manifestEntry.coverage_aliases)
    ? manifestEntry.coverage_aliases.map(normalizeSurface).filter(Boolean)
    : [];
  for (const surface of coverageSurfaces(coverageEntry)) {
    const coverageSurface = normalizeSurface(surface);
    if (coverageSurface === manifestSurface || aliases.includes(coverageSurface)) return true;
    const tokens = coverageSurface.match(/[A-Z0-9@?>=<!&+*/%.,;#$'\[\]{}()-]+/g) ?? [];
    if (tokens.includes(manifestSurface) || aliases.some((alias) => tokens.includes(alias))) return true;
  }
  return false;
}

// ---------------------------------------------------------------------------
// The native CLI
// ---------------------------------------------------------------------------

/**
 * The `ajisai` binary: `AJISAI_BIN` when set, otherwise the debug build,
 * built on demand. Progress and failure are reported under `tag`.
 */
export function resolveAjisaiBin(tag) {
  if (process.env.AJISAI_BIN) {
    if (!existsSync(process.env.AJISAI_BIN)) fatal(tag, `AJISAI_BIN not found: ${process.env.AJISAI_BIN}`);
    return process.env.AJISAI_BIN;
  }
  const debugBin = resolve(repoRoot, 'rust/target/debug/ajisai');
  if (!existsSync(debugBin)) {
    console.error(`[${tag}] building ajisai CLI (cargo build --bin ajisai)...`);
    execFileSync('cargo', ['build', '--bin', 'ajisai'], {
      cwd: resolve(repoRoot, 'rust'),
      stdio: ['ignore', 'inherit', 'inherit'],
    });
  }
  if (!existsSync(debugBin)) fatal(tag, 'ajisai CLI binary not found after build');
  return debugBin;
}

// Every script runs programs under the trusted profile, the one the committed
// semantics table was generated under. The program travels on stdin (`-`), as
// tools/mcp-server/backend/native-cli.js sends it; the report is the same one a
// file argument produces.
const agentArgv = (command, args) => ['agent', command, '-', '--limits', 'trusted', ...args];

/** Run `ajisai agent <command>` on `source` and return the raw spawnSync result. */
export function spawnAgent(bin, source, { command = 'compute', args = [] } = {}) {
  return spawnSync(bin, agentArgv(command, args), { input: `${source}\n`, encoding: 'utf8' });
}

/**
 * The same, as a child process for a caller running several at once. Its
 * stdout/stderr and `close` are the caller's to read.
 */
export function spawnAgentAsync(bin, source, { command = 'compute', args = [] } = {}) {
  const child = spawn(bin, agentArgv(command, args));
  // A child that dies before reading its program reports through `close`;
  // the pipe error that follows it says nothing more.
  child.stdin.on('error', () => {});
  child.stdin.end(`${source}\n`);
  return child;
}

/**
 * Run `ajisai agent <command>` on `source` and return its parsed JSON report.
 * Exit 0 is OK and exit 1 is a language ERROR, both with a report on stdout;
 * anything else is a failure of the CLI itself and throws `exitMessage(result)`.
 */
export function runAgent(bin, source, { command, args, exitMessage = (r) => `exit ${r.status}: ${r.stderr}` } = {}) {
  const result = spawnAgent(bin, source, { command, args });
  if (result.error) throw result.error;
  if (result.status !== 0 && result.status !== 1) throw new Error(exitMessage(result));
  return JSON.parse(result.stdout);
}

/**
 * A run's outcome as a stable id: `value`, `nil:<reason>` or
 * `error:<category>` — never the human-readable `message`, which can be
 * reworded without the outcome changing. The table generator and the
 * bijection gate classify with this one rule, so an outcome the table records
 * is spelled the way the gates compare it. A report that carries none throws:
 * every report the CLI writes classifies, so one that does not is a broken
 * CLI, not an outcome.
 */
export function classifyOutcome(json) {
  // The engine names its own outcome (`ajisai agent compute` emits it as
  // `outcome`), so the gates read that field instead of re-deriving it from
  // the report's shape.
  const outcome = json.outcome;
  if (typeof outcome !== 'string' || outcome === '') {
    throw new Error(`compute report carries no outcome id: ${JSON.stringify(json)}`);
  }
  return outcome;
}

// ---------------------------------------------------------------------------
// Generated files
// ---------------------------------------------------------------------------

/**
 * The tail every generator shares. With `--check`, compare each output against
 * the committed file and exit 1 with its `stale` message (or `missing`, when
 * given and the file is absent) on the first difference, else print `current`.
 * Without it, write every output and print `wrote`. Messages print as
 * `[tag] message`.
 */
export function writeOrCheck(tag, outputs, { current, wrote }) {
  if (process.argv.includes('--check')) {
    for (const { path, content, stale, missing } of outputs) {
      const full = resolve(repoRoot, path);
      const committed = existsSync(full) ? readFileSync(full, 'utf8') : null;
      if (committed === null && missing !== undefined) fatal(tag, missing);
      if (committed !== content) fatal(tag, stale);
    }
    console.log(`[${tag}] ${current}`);
    return;
  }
  for (const { path, content } of outputs) {
    const full = resolve(repoRoot, path);
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, content);
  }
  console.log(`[${tag}] ${wrote}`);
}
