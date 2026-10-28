// A generation's inherited lexicon as a phrasebook file, so route B subjects
// reach it through the MCP server's `words` argument instead of pasting its
// definitions into every call.
//
// The pilot's H5 counterexample came from that pasting: a Word was audited by
// expanding its body by hand, and the expansion was not the program that ran.
// Through `words`, the server defines the Words itself and every result's
// `prelude` names each one with its content digest (DIGEST), so the grader can
// check that what a subject ran is, by content, the Word the lexicon holds.

import { writeFileSync } from 'node:fs';
import { closure, norm } from './source.mjs';

/**
 * Write `lexicon.entries` to `path` in the phrasebook format — dependencies
 * first, one `DEF` line per Word followed by its `#:digest` line — and return
 * `{ digests, names }`: each Word's digest as the engine computes it, and the
 * names in the order written.
 */
export async function writeLexiconPhrasebook(ajisai, lexicon, path) {
  const byName = new Map(lexicon.entries.map((entry) => [norm(entry.name), entry]));
  const ordered = closure(lexicon.entries.map((entry) => entry.name), byName);
  const defs = ordered.map(({ name, body }) => `[ ${body} ] '${name}' DEF`);
  const reads = ordered.map(({ name }) => `[ ${name} ] 0 GET DIGEST`).join(' ');
  const result = ordered.length ? await ajisai.compute(`${defs.join('\n')}\n${reads}`) : { status: 'ok', stackDisplay: [] };
  if (result.status !== 'ok') throw new Error(`the lexicon does not define cleanly: ${result.message}`);
  const digests = new Map(ordered.map(({ name }, i) => [norm(name), result.stackDisplay[i].replace(/^'|'$/g, '')]));
  const lines = ['# Inherited lexicon, written by tools/lexicon-emergence for the MCP server\'s `words`.'];
  for (const [i, { name }] of ordered.entries()) {
    lines.push(defs[i], `#:digest ${name} ${digests.get(norm(name))}`);
  }
  writeFileSync(path, `${lines.join('\n')}\n`);
  return { digests, names: ordered.map(({ name }) => name) };
}

/**
 * Whether a result's `prelude` is the lexicon by content: every Word it names
 * carries the digest the lexicon file recorded for it.
 */
export function preludeMatches(prelude, digests) {
  return (prelude ?? []).every(({ name, digest }) => digests.get(norm(name)) === digest);
}
