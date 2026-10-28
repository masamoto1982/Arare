// Plumbing every scripts/*.mjs gate and generator shares: where the repository
// is, how a committed file is read, how a failure is reported, how the tree is
// walked, and how a generated file is written or checked. Each script keeps its
// own tag and message text — those are what CI logs show — and passes them in;
// what lives here is only the mechanism. Nothing here decides anything about
// the language.

import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

export const repoRoot = resolve(import.meta.dirname, '../..');

/** A repository file as text; `path` is relative to the repository root. */
export const readText = (path) => readFileSync(resolve(repoRoot, path), 'utf8');

/** A repository file parsed as JSON. */
export const readJson = (path) => JSON.parse(readText(path));

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
