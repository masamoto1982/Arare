// C-shuffled (work order §5.2): a lexicon whose names carry no meaning. Each
// entry is renamed to a fixed nonsense name and its note is dropped, so a
// subject that reuses an inherited Word can only have read what it does from
// its body. Whether reuse survives this separates reuse by meaning from reuse
// by name.

import { createHash } from 'node:crypto';
import { norm, tokenize } from './source.mjs';

const CONSONANTS = 'BDFGKLMNPRSTVZ';
const VOWELS = 'AEIOU';

/** A pronounceable nonsense name for `name`, fixed by its hash, under `prefix`. */
export function nonsenseName(name, prefix) {
  const bytes = createHash('sha256').update(norm(name)).digest();
  let syllables = '';
  for (let i = 0; i < 3; i += 1) {
    syllables += CONSONANTS[bytes[2 * i] % CONSONANTS.length] + VOWELS[bytes[2 * i + 1] % VOWELS.length];
  }
  return `${prefix}.${syllables}`;
}

/**
 * `lexicon` with every entry not already under `prefix` renamed, references
 * between entries rewritten to the new names and notes dropped. Each renamed
 * entry records the name it had.
 */
export function shuffleLexicon(lexicon, prefix) {
  const renames = new Map();
  const taken = new Set(lexicon.entries.map((e) => norm(e.name)));
  for (const entry of lexicon.entries) {
    if (norm(entry.name).startsWith(`${norm(prefix)}.`)) continue;
    let name = nonsenseName(entry.name, prefix);
    for (let n = 2; taken.has(norm(name)); n += 1) name = `${nonsenseName(entry.name, prefix)}${n}`;
    taken.add(norm(name));
    renames.set(norm(entry.name), name);
  }
  const rewrite = (body) =>
    tokenize(body)
      .map((t) => renames.get(norm(t)) ?? t)
      .join(' ');
  return {
    ...lexicon,
    entries: lexicon.entries.map((entry) => {
      const renamed = renames.get(norm(entry.name));
      return {
        ...entry,
        name: renamed ?? entry.name,
        body: rewrite(entry.body),
        note: '',
        ...(renamed ? { shuffledFrom: entry.name } : {}),
      };
    }),
  };
}
